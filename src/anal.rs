/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

use log::warn;

use super::gbasm;
use super::tags;
use super::util;
use super::xaddr::prelude::*;

#[derive(Clone, Copy, Debug)]
pub struct RomInfo {
    pub big_rom: bool,
    pub cgb_ram: bool,
    pub sram_count: usize,
}

#[derive(Debug)]
pub struct AnalInfo<'a> {
    pub rom: &'a [u8],
    pub rom_info: RomInfo,
    pub tags: &'a [(XAddr, tags::Tag)],
}

#[derive(Debug)]
pub enum RomSliceError {
    NonRomAddr,
    BankedRomAddr,
    NonBankedHiRomAddr,
    BankTooHigh,
}

impl<'a> AnalInfo<'a> {
    pub fn new(rom_info: RomInfo, rom: &'a [u8], tags: &'a [(XAddr, tags::Tag)]) -> Self {
        assert_eq!(rom.len() % 0x4000, 0);

        Self {
            rom: rom,
            rom_info: rom_info,
            tags: tags,
        }
    }

    pub fn rom_slice(&self, xa: XAddr, len: usize) -> Result<&[u8], RomSliceError> {
        use std::cmp;

        match xa.addr {
            0x0000..=0x3FFF => {
                if xa.bank != 0 {
                    return Err(RomSliceError::BankedRomAddr);
                }

                if self.rom_info.big_rom {
                    let off = xa.addr as usize;
                    let end = cmp::min(off + len, 0x4000);

                    Ok(&self.rom[off..end])
                } else {
                    let off = xa.addr as usize;
                    let end = cmp::min(off + len, self.rom.len());

                    Ok(&self.rom[off..end])
                }
            }

            0x4000..=0x7FFF => {
                if xa.bank == 0 {
                    if self.rom_info.big_rom {
                        return Err(RomSliceError::NonBankedHiRomAddr);
                    }

                    let off = xa.addr as usize;
                    let end = cmp::min(off + len, self.rom.len());

                    Ok(&self.rom[off..end])
                } else {
                    let bnk = 0x4000 * (xa.bank as usize);
                    let off = xa.addr as usize - 0x4000;
                    let end = cmp::min(off + len, 0x4000);

                    if !self.rom_info.big_rom {
                        return Err(RomSliceError::BankedRomAddr);
                    }

                    if bnk + end > self.rom.len() {
                        return Err(RomSliceError::BankTooHigh);
                    }

                    Ok(&self.rom[bnk + off..bnk + end])
                }
            }

            _ => Err(RomSliceError::NonRomAddr),
        }
    }

    pub fn rom_bank_count(&self) -> usize {
        match self.rom_info.big_rom {
            true => (self.rom.len() + 0x3FFF) / 0x4000,
            false => 1,
        }
    }

    pub fn rom_bank_block(&self, bank: usize) -> (XAddr, usize) {
        if self.rom_info.big_rom {
            assert!(bank < self.rom_bank_count());
            (
                XAddr::new(bank as u16, if bank == 0 { 0x0000 } else { 0x4000 }),
                0x4000,
            )
        } else {
            assert_eq!(bank, 0);
            (XAddr::new(0, 0), self.rom.len())
        }
    }

    pub fn rom_bank_blocks(&self) -> Vec<(XAddr, usize)> {
        let bank_count = self.rom_bank_count();
        let mut result = Vec::with_capacity(bank_count);

        for i in 0..bank_count {
            result.push(self.rom_bank_block(i));
        }

        result
    }
}

pub struct AnalEmu<'a> {
    info: &'a AnalInfo<'a>,
    decoder: gbasm::DecodeSliceIter<'a, XAddr>,
    romb: Option<u16>,
    ramb: Option<u16>,
    srmb: Option<u16>,
}

impl<'a> AnalEmu<'a> {
    pub fn with_bound(info: &'a AnalInfo, xa: XAddr, len: usize) -> Self {
        let slice = match info.rom_slice(xa, len) {
            Ok(slice) => slice,
            Err(e) => panic!("{}[{:04X}] {:?}", xa, len, e),
        };

        Self {
            info: info,
            decoder: gbasm::decode_slice(xa, slice),
            romb: if let 0x4000..=0x7FFF = xa.addr {
                Some(xa.bank)
            } else {
                None
            },
            ramb: None,
            srmb: None,
        }
    }

    pub fn new(info: &'a AnalInfo, xa: XAddr) -> Self {
        Self::with_bound(info, xa, 0x8000)
    }

    pub fn expand_addr(&self, addr: u16) -> Option<XAddr> {
        match addr {
            0x4000..=0x7FFF => {
                if self.info.rom_info.big_rom {
                    return self.romb.map(|b| XAddr::new(b, addr));
                }
            }

            0xA000..=0xBFFF => return self.srmb.map(|b| XAddr::new(b, addr)),

            0xD000..=0xDFFF => {
                if self.info.rom_info.cgb_ram {
                    return self.ramb.map(|b| XAddr::new(b, addr));
                }
            }

            _ => {}
        }

        Some(XAddr::new(0, addr))
    }
}

impl<'a> Iterator for AnalEmu<'a> {
    type Item = (XAddr, gbasm::DecodeResult);

    fn next(&mut self) -> Option<(XAddr, gbasm::DecodeResult)> {
        if let Some((xa, ins)) = self.decoder.next() {
            for (_, tag) in tags::get_tags_at(self.info.tags, &xa) {
                match tag {
                    tags::Tag::RomBank(bank) => self.romb = Some(*bank),
                    tags::Tag::RamBank(bank) => self.ramb = Some(*bank),
                    tags::Tag::SrmBank(bank) => self.srmb = Some(*bank),
                    _ => {}
                }
            }

            return Some((xa, ins));
        }

        None
    }
}

fn scan_head_block(info: &AnalInfo, xa: XAddr, max_len: usize) -> Option<(XAddr, usize)> {
    // returns the range corresponding to the head code block in input block
    // a code block is a sequence of instructions ending at a flow intersection (either a jump or jump target)
    // it is assumed that input block does not contain any jump targets/entry points beyond the very start of it

    let mut offset = 0;

    for (_, ins) in AnalEmu::with_bound(info, xa, max_len) {
        match ins {
            Ok(ins) => {
                offset += ins.encoded_len();

                if (ins.info().flags & gbasm::OPCODE_FLAG_JUMP) != 0 {
                    return Some((xa, offset));
                }
            }

            // this is pretty much the only time we accept bad decode
            Err(_) => return None,
        }
    }

    // we reached the end of the scan range without encountering a jump
    // this would mean that this block flows into the next one

    Some((xa, max_len))
}

fn search_for_code(info: &AnalInfo, parent_blocks: &[(XAddr, usize)]) -> Vec<(XAddr, usize)> {
    let mut result = vec![];

    for &(xstart, max_len) in parent_blocks {
        let mut offset = 0;

        'lop_scan: while offset < max_len {
            let (xa, len) = match scan_head_block(&info, xstart + offset as u16, max_len - offset) {
                Some(code_block) => code_block,
                None => break 'lop_scan,
            };

            result.push((xa, len));

            // scan for unconditional end instruction
            // if we find one, this is an end block
            // which means we shouldn't continue searching past it

            let mut emu = AnalEmu::with_bound(info, xa, len);

            while let Some((_, Ok(ins))) = emu.next() {
                let flags = ins.info().flags;

                if (flags & gbasm::OPCODE_FLAG_JUMP) != 0 {
                    if (flags & (gbasm::OPCODE_FLAG_CALL | gbasm::OPCODE_FLAG_CONDITIONAL)) == 0 {
                        break 'lop_scan;
                    }

                    // check for noreturn function calls

                    if (flags & gbasm::OPCODE_FLAG_CALL) != 0 {
                        if let Some(xa) = ins
                            .get_jump_target()
                            .map(|addr| emu.expand_addr(addr))
                            .flatten()
                        {
                            for (_, tag) in tags::get_tags_at(info.tags, &xa) {
                                if let tags::Tag::NoReturn = tag {
                                    break 'lop_scan;
                                }
                            }
                        }
                    }
                }
            }

            offset += len;
        }
    }

    result
}

fn cut_blocks(info: &AnalInfo, points: &[XAddr]) -> Vec<(XAddr, usize)> {
    use superslice::*;

    let bank_count = info.rom_bank_count();
    let mut result = Vec::with_capacity(points.len());

    for i in 0..bank_count {
        let (bank_xa, bank_len) = info.rom_bank_block(i);

        let (point_beg, point_end) = (
            points.lower_bound(&bank_xa),
            points.upper_bound(&(bank_xa + bank_len as u16)),
        );

        for j in point_beg..point_end {
            let xa = points[j];

            let len = if j + 1 == point_end {
                bank_len - (xa.addr - bank_xa.addr) as usize
            } else {
                (points[j + 1].addr - xa.addr) as usize
            };

            result.push((xa, len));
        }
    }

    result
}

fn scan_xrefs(info: &AnalInfo, code_blocks: &[(XAddr, usize)]) -> Vec<XAddr> {
    let mut result = vec![];

    for &(xa, len) in code_blocks {
        let mut emu = AnalEmu::with_bound(info, xa, len);

        'lop_ins: while let Some((ins_xa, Ok(ins))) = emu.next() {
            for (_, tag) in tags::get_tags_at(info.tags, &ins_xa) {
                if let tags::Tag::DontFollowCall = tag {
                    continue 'lop_ins;
                }
            }

            if let Some(addr) = ins.get_jump_target() {
                match emu.expand_addr(addr) {
                    Some(xa) => result.push(xa),
                    None => (),
                }
            }
        }
    }

    result.sort();
    result.dedup();

    result
}

fn warn_about_differences(prev_points: &[XAddr], new_points: &[XAddr]) {
    let mut i = 0;
    let mut j = 0;

    while i < prev_points.len() && j < new_points.len() {
        if j == new_points.len() || prev_points[i] < new_points[j] {
            warn!("point {0} was removed", prev_points[i]);
            i += 1;
        } else if i == prev_points.len() || new_points[j] < prev_points[i] {
            warn!("point {0} was added", new_points[j]);
            j += 1;
        } else {
            i += 1;
            j += 1;
        }
    }
}

pub fn anal(info: &AnalInfo, entry_points: &[XAddr]) -> Vec<(XAddr, usize)> {
    use log::info;

    let mut points = entry_points.to_vec();
    points.dedup();

    let mut lop_count = 0;

    loop {
        lop_count += 1;
        info!(
            "start analysis cycle #{}: {} analysis point(s)",
            lop_count,
            points.len()
        );

        let cut_blocks = cut_blocks(info, &points);
        let code_blocks = search_for_code(info, &cut_blocks);
        let prev_points = points;

        let code_xrefs = scan_xrefs(&info, &code_blocks);

        info!(
            "analysis cycle #{} ended, finding {} code ranges and {} code xrefs",
            lop_count,
            code_blocks.len(),
            code_xrefs.len()
        );

        points = util::sorted_merge(&entry_points, &code_xrefs);
        points.dedup();

        if points == prev_points {
            info!("no new xrefs found, ending analysis");
            return code_blocks;
        }

        if points.len() < prev_points.len() {
            warn!("found less points than previously");
            warn_about_differences(&prev_points, &points);
            return code_blocks;
        }
    }
}
