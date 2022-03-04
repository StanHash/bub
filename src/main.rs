/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

pub mod util;
pub mod gbasm;
pub mod xaddr;
pub mod tags;
pub mod anal;

use xaddr::prelude::*;

use std::path::PathBuf;
use structopt::StructOpt;
use anyhow::Result;

#[derive(StructOpt)]
#[structopt(name = "bub")]
struct Opt
{
    #[structopt(name = "rom", parse(from_os_str))]
    input_filename: PathBuf,

    #[structopt(name = "tags", parse(from_os_str))]
    tags_filename: Option<PathBuf>,

    #[structopt(long)]
    big_rom: Option<bool>,

    #[structopt(long)]
    cgb_ram: Option<bool>,

    #[structopt(long)]
    sram_count: Option<usize>,
}

const SRAM_COUNT_LUT: &[usize] = &[
    0,  // $00: no sram
    0,  // $01: unused
    1,  // $02: 8KiB, 1 bank
    4,  // $03: 32KiB, 4 banks
    16, // $04: 128KiB, 16 banks
    8]; // $05: 64KiB, 8 banks

use std::collections::HashMap;

fn default_xaddr_name(xa: XAddr, base: &str) -> String
{
    match xa.addr
    {
        0xA000 ..= 0xAFFF => format!("s{}_{:02X}_{:04X}", base, xa.bank, xa.addr),
        0xFF80 ..= 0xFFFE => format!("h{}{:04X}", base, xa.addr),

        0xC000 ..= 0xDFFF =>
        {
            match xa.bank
            {
                0 => format!("w{}{:04X}", base, xa.addr),
                _ => format!("w{}_{:02X}_{:04X}", base, xa.bank, xa.addr),
            }
        }

        _ =>
        {
            match xa.bank
            {
                0 => format!("{}_{:04X}", base, xa.addr),
                _ => format!("{}_{:02X}_{:04X}", base, xa.bank, xa.addr),
            }
        }
    }
}

fn update_name_map_with_code_refs(info: &anal::AnalInfo, code_blocks: &[(XAddr, usize)], name_map: &mut HashMap<XAddr, String>)
{
    use log::warn;

    for &(xa, len) in code_blocks
    {
        let mut emu = anal::AnalEmu::with_bound(info, xa, len);

        while let Some((xa, Ok(ins))) = emu.next()
        {
            if let Some(addr) = ins.get_jump_target()
            {
                match emu.expand_addr(addr)
                {
                    Some(xa) => { name_map.entry(xa).or_insert(default_xaddr_name(xa, "Code")); }
                    None => warn!("unresolved code xref at {}: {:04X}", xa, addr),
                }
            }
            else if ins.is_addr_operand() || tags::get_tags_at(info.tags, &xa).iter().any(|(_, tag)| if let tags::Tag::OperandAddr = tag { true } else { false })
            {
                let addr = ins.operand;

                match emu.expand_addr(addr)
                {
                    Some(xa) => { name_map.entry(xa).or_insert(default_xaddr_name(xa, "Unk")); }
                    None => warn!("unresolved data xref at {}: {:04X}", xa, addr),
                }
            }
        }
    }
}

fn main() -> Result<()>
{
    use std::fs::File;
    use std::io::Read;
    use std::io::BufReader;

    env_logger::builder().format_timestamp(None).init();

    let opt = Opt::from_args();

    let rom_data =
    {
        let mut file = File::open(opt.input_filename)?;

        let mut rom_data = vec![];
        file.read_to_end(&mut rom_data)?;

        rom_data
    };

    let rom_info = anal::RomInfo
    {
        big_rom: opt.big_rom.unwrap_or(rom_data.len() > 0x8000),
        cgb_ram: opt.cgb_ram.unwrap_or(rom_data[0x143] == 0xC0),
        sram_count: opt.sram_count.unwrap_or(*SRAM_COUNT_LUT.get(rom_data[0x149] as usize).unwrap_or(&0)),
    };

    let tags = match opt.tags_filename
    {
        Some(filename) => tags::parse_tags(&mut BufReader::new(File::open(filename)?))?,
        None => vec![]
    };

    let entry_points =
    {
        use std::collections::BinaryHeap;

        let mut entry_points = BinaryHeap::new();

        for (xa, tag) in &tags
        {
            if let tags::Tag::Code = tag
            {
                entry_points.push(*xa);
            }
        }

        entry_points.into_sorted_vec()
    };

    let anal_info = anal::AnalInfo::new(rom_info, &rom_data, &tags);

    let code_blocks = anal::anal(&anal_info, &entry_points);

    let mut name_map = HashMap::new();

    for (xa, tag) in &tags
    {
        if let tags::Tag::Name(name) = tag
        {
            name_map.entry(*xa).or_insert(name.clone());
        }
    }

    update_name_map_with_code_refs(&anal_info, &code_blocks, &mut name_map);

    let mut last_xa = XAddr::new(0xFFFF, 0xFFFF);
    let mut last_name = String::from("");

    let mut get_local_name = |name: String, update: bool|
    {
        let parts: Vec<_> = name.split('.').collect();

        if parts.len() == 2 && parts[0] == last_name
        {
            format!(".{}", parts[1])
        }
        else
        {
            if update {
                last_name = name.clone(); }
            name
        }
    };

    for (xa, len) in code_blocks
    {
        if last_xa != xa
        {
            println!("\t; end: {}", last_xa);
            println!("\tsection \"rom_{:02X}_{:04X}\"", xa.bank, xa.addr);
        }

        last_xa = xa + len as u16;

        if let Some(name) = name_map.get(&xa)
        {
            let name = get_local_name(name.clone(), true);
            println!("{}: ; {}", name, xa)
        }

        let mut emu = anal::AnalEmu::with_bound(&anal_info, xa, len);

        while let Some((xa, Ok(ins))) = emu.next()
        {
            let fmt = ins.info().fmt;

            let ops = format!("${:X}", ins.operand);
            let ops = if ins.is_addr_operand() || tags::get_tags_at(&tags, &xa).iter().any(|(_, tag)| if let tags::Tag::OperandAddr = tag { true } else { false })
            {
                match emu.expand_addr(ins.operand).map(|xa| name_map.get(&xa)).flatten()
                {
                    Some(name) => get_local_name(name.clone(), false),
                    None => ops
                }
            }
            else
            {
                ops
            };

            let fmt = fmt.replace("%", &ops);

            println!("\t/* {} */ {}", xa, fmt);
        }

        println!("");
    }

    Ok(())
}
