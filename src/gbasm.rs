/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

use std::ops::AddAssign;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum OperandKind
{
    None,
    Undefined,
    LongOpcode,
    Code,
    CodeRelative,
    Data,
    DataHram,
}

pub const OPCODE_FLAG_JUMP: u8        = 0b00000001;
pub const OPCODE_FLAG_CALL: u8        = 0b00000010;
pub const OPCODE_FLAG_CONDITIONAL: u8 = 0b00000100;
pub const OPCODE_FLAG_WRITE_MEM: u8   = 0b00001000;
pub const OPCODE_FLAG_READ_MEM: u8    = 0b00010000;
pub const OPCODE_FLAG_INVALID: u8     = 0b10000000;

#[derive(Clone, Copy, Debug)]
pub struct OpcodeInfo
{
    pub fmt: &'static str,
    pub operand_len: u8,
    pub operand_kind: OperandKind,
    pub flags: u8,
}

const OPCODE_BITOPS: u8 = 0xCB;
const OPCODE_RST_00: u8 = 0xC7;
const OPCODE_RST_08: u8 = 0xCF;
const OPCODE_RST_10: u8 = 0xD7;
const OPCODE_RST_18: u8 = 0xDF;
const OPCODE_RST_20: u8 = 0xE7;
const OPCODE_RST_28: u8 = 0xEF;
const OPCODE_RST_30: u8 = 0xF7;
const OPCODE_RST_38: u8 = 0xFF;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Instruction
{
    pub opcode: u8,
    pub operand: u16,
}

impl Instruction
{
    pub const fn new() -> Self
    {
        Instruction
        {
            opcode: 0,
            operand: 0
        }
    }

    pub fn info(&self) -> &'static OpcodeInfo
    {
        match self.opcode
        {
            OPCODE_BITOPS => &BITOPS_INFO[self.operand as usize],
            opcode => &OPCODE_INFO[opcode as usize],
        }
    }

    pub fn is_valid(&self) -> bool
    {
        (self.info().flags & OPCODE_FLAG_INVALID) == 0
    }

    pub fn encoded_len(&self) -> usize
    {
        self.info().operand_len as usize + 1
    }

    pub fn get_jump_target(&self) -> Option<u16>
    {
        match self.opcode
        {
            OPCODE_RST_00 => Some(0x0000),
            OPCODE_RST_08 => Some(0x0008),
            OPCODE_RST_10 => Some(0x0010),
            OPCODE_RST_18 => Some(0x0018),
            OPCODE_RST_20 => Some(0x0020),
            OPCODE_RST_28 => Some(0x0028),
            OPCODE_RST_30 => Some(0x0030),
            OPCODE_RST_38 => Some(0x0038),
            _ =>
            {
                if (self.info().flags & OPCODE_FLAG_JUMP) != 0
                && self.info().operand_kind != OperandKind::None {
                    Some(self.operand) }
                else {
                    None }
            }
        }
    }

    pub fn is_addr_operand(&self) -> bool
    {
        return self.info().flags & (OPCODE_FLAG_READ_MEM | OPCODE_FLAG_WRITE_MEM | OPCODE_FLAG_JUMP) != 0
            && self.info().operand_kind != OperandKind::None
            && (self.info().operand_len == 2 || self.info().operand_kind == OperandKind::DataHram || self.info().operand_kind == OperandKind::CodeRelative);
    }
}

#[derive(Clone, Copy, Debug)]
pub enum DecodeError
{
    SliceTooSmall,
    InvalidOpcode,
}

pub type DecodeResult = Result<Instruction, DecodeError>;

pub fn decode(addr: u16, slice: &[u8]) -> DecodeResult
{
    if slice.len() == 0 {
        return Err(DecodeError::SliceTooSmall); }

    let mut result = Instruction::new();

    // read opcode

    result.opcode = slice[0];

    if !result.is_valid() {
        return Err(DecodeError::InvalidOpcode); }

    // read operand

    let len = result.encoded_len();

    if slice.len() < len {
        return Err(DecodeError::SliceTooSmall); }

    for i in 0 .. len-1 {
        result.operand += (slice[1+i] as u16) << i*8; }

    // fix operand if necessary

    match result.info().operand_kind
    {
        OperandKind::CodeRelative =>
            result.operand = ((addr as i32) + 2 + (result.operand as i8) as i32) as u16,

        OperandKind::DataHram =>
            result.operand = 0xFF00 + result.operand,

        _ => {}
    }

    Ok(result)
}

pub struct DecodeSliceIter<'a, T>
    where T: Copy + AddAssign<u16> + Into<u16>
{
    addr: T,
    slice: &'a [u8],
}

impl<'a, T> Iterator for DecodeSliceIter<'a, T>
    where T: Copy + AddAssign<u16> + Into<u16>
{
    type Item = (T, DecodeResult);

    fn next(&mut self) -> Option<(T, DecodeResult)>
    {
        if self.slice.len() == 0 {
            return None; }

        let (addr, ins) = (self.addr, decode(self.addr.into(), self.slice));

        if let Ok(ins) = ins
        {
            self.addr += ins.encoded_len() as u16;
            self.slice = &self.slice[ins.encoded_len() ..];
        }

        Some((addr, ins))
    }
}

pub fn decode_slice<'a, T>(addr: T, slice: &'a [u8]) -> DecodeSliceIter<'a, T>
    where T: Copy + AddAssign<u16> + Into<u16>
{
    DecodeSliceIter
    {
        addr: addr,
        slice: slice,
    }
}

const fn opi(fmt: &'static str, operand_len: u8, operand_kind: OperandKind, flags: u8) -> OpcodeInfo
{
    OpcodeInfo
    {
        fmt: fmt,
        operand_len: operand_len,
        operand_kind: operand_kind,
        flags: flags,
    }
}

const OPCODE_INFO: [OpcodeInfo; 0x100] =
[
    /* 00 */ opi("nop", 0, OperandKind::None, 0),
    /* 01 */ opi("ld bc, %", 2, OperandKind::Undefined, 0),
    /* 02 */ opi("ld [bc], a", 0, OperandKind::None, OPCODE_FLAG_WRITE_MEM),
    /* 03 */ opi("inc bc", 0, OperandKind::None, 0),
    /* 04 */ opi("inc b", 0, OperandKind::None, 0),
    /* 05 */ opi("dec b", 0, OperandKind::None, 0),
    /* 06 */ opi("ld b, %", 1, OperandKind::Undefined, 0),
    /* 07 */ opi("rlca", 0, OperandKind::None, 0),
    /* 08 */ opi("ld [%], sp", 2, OperandKind::Data, OPCODE_FLAG_WRITE_MEM),
    /* 09 */ opi("add hl, bc", 0, OperandKind::None, 0),
    /* 0A */ opi("ld a, [bc]", 0, OperandKind::None, OPCODE_FLAG_READ_MEM),
    /* 0B */ opi("dec bc", 0, OperandKind::None, 0),
    /* 0C */ opi("inc c", 0, OperandKind::None, 0),
    /* 0D */ opi("dec c", 0, OperandKind::None, 0),
    /* 0E */ opi("ld c, %", 1, OperandKind::Undefined, 0),
    /* 0F */ opi("rrca", 0, OperandKind::None, 0),
    /* 10 */ opi("stop", 1, OperandKind::LongOpcode, 0),
    /* 11 */ opi("ld de, %", 2, OperandKind::Undefined, 0),
    /* 12 */ opi("ld [de], a", 0, OperandKind::None, OPCODE_FLAG_WRITE_MEM),
    /* 13 */ opi("inc de", 0, OperandKind::None, 0),
    /* 14 */ opi("inc d", 0, OperandKind::None, 0),
    /* 15 */ opi("dec d", 0, OperandKind::None, 0),
    /* 16 */ opi("ld d, %", 1, OperandKind::Undefined, 0),
    /* 17 */ opi("rla", 0, OperandKind::None, 0),
    /* 18 */ opi("jr %", 1, OperandKind::CodeRelative, OPCODE_FLAG_JUMP),
    /* 19 */ opi("add hl, de", 0, OperandKind::None, 0),
    /* 1A */ opi("ld a, [de]", 0, OperandKind::None, OPCODE_FLAG_READ_MEM),
    /* 1B */ opi("dec de", 0, OperandKind::None, 0),
    /* 1C */ opi("inc e", 0, OperandKind::None, 0),
    /* 1D */ opi("dec e", 0, OperandKind::None, 0),
    /* 1E */ opi("ld e, %", 1, OperandKind::Undefined, 0),
    /* 1F */ opi("rra", 0, OperandKind::None, 0),
    /* 20 */ opi("jr nz, %", 1, OperandKind::CodeRelative, OPCODE_FLAG_JUMP | OPCODE_FLAG_CONDITIONAL),
    /* 21 */ opi("ld hl, %", 2, OperandKind::Undefined, 0),
    /* 22 */ opi("ld [hli], a", 0, OperandKind::None, OPCODE_FLAG_WRITE_MEM),
    /* 23 */ opi("inc hl", 0, OperandKind::None, 0),
    /* 24 */ opi("inc h", 0, OperandKind::None, 0),
    /* 25 */ opi("dec h", 0, OperandKind::None, 0),
    /* 26 */ opi("ld h, %", 1, OperandKind::Undefined, 0),
    /* 27 */ opi("daa", 0, OperandKind::None, 0),
    /* 28 */ opi("jr z, %", 1, OperandKind::CodeRelative, OPCODE_FLAG_JUMP | OPCODE_FLAG_CONDITIONAL),
    /* 29 */ opi("add hl, hl", 0, OperandKind::None, 0),
    /* 2A */ opi("ld a, [hli]", 0, OperandKind::None, OPCODE_FLAG_READ_MEM),
    /* 2B */ opi("dec hl", 0, OperandKind::None, 0),
    /* 2C */ opi("inc l", 0, OperandKind::None, 0),
    /* 2D */ opi("dec l", 0, OperandKind::None, 0),
    /* 2E */ opi("ld l, %", 1, OperandKind::Undefined, 0),
    /* 2F */ opi("cpl", 0, OperandKind::None, 0),
    /* 30 */ opi("jr nc, %", 1, OperandKind::CodeRelative, OPCODE_FLAG_JUMP | OPCODE_FLAG_CONDITIONAL),
    /* 31 */ opi("ld sp, %", 2, OperandKind::Undefined, 0),
    /* 32 */ opi("ld [hld], a", 0, OperandKind::None, OPCODE_FLAG_WRITE_MEM),
    /* 33 */ opi("inc sp", 0, OperandKind::None, 0),
    /* 34 */ opi("inc [hl]", 0, OperandKind::None, OPCODE_FLAG_WRITE_MEM | OPCODE_FLAG_READ_MEM),
    /* 35 */ opi("dec [hl]", 0, OperandKind::None, OPCODE_FLAG_WRITE_MEM | OPCODE_FLAG_READ_MEM),
    /* 36 */ opi("ld [hl], %", 1, OperandKind::Undefined, OPCODE_FLAG_WRITE_MEM),
    /* 37 */ opi("scf", 0, OperandKind::None, 0),
    /* 38 */ opi("jr c, %", 1, OperandKind::CodeRelative, OPCODE_FLAG_JUMP | OPCODE_FLAG_CONDITIONAL),
    /* 39 */ opi("add hl, sp", 0, OperandKind::None, 0),
    /* 3A */ opi("ld a, [hld]", 0, OperandKind::None, OPCODE_FLAG_READ_MEM),
    /* 3B */ opi("dec sp", 0, OperandKind::None, 0),
    /* 3C */ opi("inc a", 0, OperandKind::None, 0),
    /* 3D */ opi("dec a", 0, OperandKind::None, 0),
    /* 3E */ opi("ld a, %", 1, OperandKind::Undefined, 0),
    /* 3F */ opi("ccf", 0, OperandKind::None, 0),
    /* 40 */ opi("ld b, b", 0, OperandKind::None, 0),
    /* 41 */ opi("ld b, c", 0, OperandKind::None, 0),
    /* 42 */ opi("ld b, d", 0, OperandKind::None, 0),
    /* 43 */ opi("ld b, e", 0, OperandKind::None, 0),
    /* 44 */ opi("ld b, h", 0, OperandKind::None, 0),
    /* 45 */ opi("ld b, l", 0, OperandKind::None, 0),
    /* 46 */ opi("ld b, [hl]", 0, OperandKind::None, OPCODE_FLAG_READ_MEM),
    /* 47 */ opi("ld b, a", 0, OperandKind::None, 0),
    /* 48 */ opi("ld c, b", 0, OperandKind::None, 0),
    /* 49 */ opi("ld c, c", 0, OperandKind::None, 0),
    /* 4A */ opi("ld c, d", 0, OperandKind::None, 0),
    /* 4B */ opi("ld c, e", 0, OperandKind::None, 0),
    /* 4C */ opi("ld c, h", 0, OperandKind::None, 0),
    /* 4D */ opi("ld c, l", 0, OperandKind::None, 0),
    /* 4E */ opi("ld c, [hl]", 0, OperandKind::None, OPCODE_FLAG_READ_MEM),
    /* 4F */ opi("ld c, a", 0, OperandKind::None, 0),
    /* 50 */ opi("ld d, b", 0, OperandKind::None, 0),
    /* 51 */ opi("ld d, c", 0, OperandKind::None, 0),
    /* 52 */ opi("ld d, d", 0, OperandKind::None, 0),
    /* 53 */ opi("ld d, e", 0, OperandKind::None, 0),
    /* 54 */ opi("ld d, h", 0, OperandKind::None, 0),
    /* 55 */ opi("ld d, l", 0, OperandKind::None, 0),
    /* 56 */ opi("ld d, [hl]", 0, OperandKind::None, OPCODE_FLAG_READ_MEM),
    /* 57 */ opi("ld d, a", 0, OperandKind::None, 0),
    /* 58 */ opi("ld e, b", 0, OperandKind::None, 0),
    /* 59 */ opi("ld e, c", 0, OperandKind::None, 0),
    /* 5A */ opi("ld e, d", 0, OperandKind::None, 0),
    /* 5B */ opi("ld e, e", 0, OperandKind::None, 0),
    /* 5C */ opi("ld e, h", 0, OperandKind::None, 0),
    /* 5D */ opi("ld e, l", 0, OperandKind::None, 0),
    /* 5E */ opi("ld e, [hl]", 0, OperandKind::None, OPCODE_FLAG_READ_MEM),
    /* 5F */ opi("ld e, a", 0, OperandKind::None, 0),
    /* 60 */ opi("ld h, b", 0, OperandKind::None, 0),
    /* 61 */ opi("ld h, c", 0, OperandKind::None, 0),
    /* 62 */ opi("ld h, d", 0, OperandKind::None, 0),
    /* 63 */ opi("ld h, e", 0, OperandKind::None, 0),
    /* 64 */ opi("ld h, h", 0, OperandKind::None, 0),
    /* 65 */ opi("ld h, l", 0, OperandKind::None, 0),
    /* 66 */ opi("ld h, [hl]", 0, OperandKind::None, OPCODE_FLAG_READ_MEM),
    /* 67 */ opi("ld h, a", 0, OperandKind::None, 0),
    /* 68 */ opi("ld l, b", 0, OperandKind::None, 0),
    /* 69 */ opi("ld l, c", 0, OperandKind::None, 0),
    /* 6A */ opi("ld l, d", 0, OperandKind::None, 0),
    /* 6B */ opi("ld l, e", 0, OperandKind::None, 0),
    /* 6C */ opi("ld l, h", 0, OperandKind::None, 0),
    /* 6D */ opi("ld l, l", 0, OperandKind::None, 0),
    /* 6E */ opi("ld l, [hl]", 0, OperandKind::None, OPCODE_FLAG_READ_MEM),
    /* 6F */ opi("ld l, a", 0, OperandKind::None, 0),
    /* 70 */ opi("ld [hl], b", 0, OperandKind::None, OPCODE_FLAG_WRITE_MEM),
    /* 71 */ opi("ld [hl], c", 0, OperandKind::None, OPCODE_FLAG_WRITE_MEM),
    /* 72 */ opi("ld [hl], d", 0, OperandKind::None, OPCODE_FLAG_WRITE_MEM),
    /* 73 */ opi("ld [hl], e", 0, OperandKind::None, OPCODE_FLAG_WRITE_MEM),
    /* 74 */ opi("ld [hl], h", 0, OperandKind::None, OPCODE_FLAG_WRITE_MEM),
    /* 75 */ opi("ld [hl], l", 0, OperandKind::None, OPCODE_FLAG_WRITE_MEM),
    /* 76 */ opi("halt", 0, OperandKind::None, 0),
    /* 77 */ opi("ld [hl], a", 0, OperandKind::None, OPCODE_FLAG_WRITE_MEM),
    /* 78 */ opi("ld a, b", 0, OperandKind::None, 0),
    /* 79 */ opi("ld a, c", 0, OperandKind::None, 0),
    /* 7A */ opi("ld a, d", 0, OperandKind::None, 0),
    /* 7B */ opi("ld a, e", 0, OperandKind::None, 0),
    /* 7C */ opi("ld a, h", 0, OperandKind::None, 0),
    /* 7D */ opi("ld a, l", 0, OperandKind::None, 0),
    /* 7E */ opi("ld a, [hl]", 0, OperandKind::None, OPCODE_FLAG_READ_MEM),
    /* 7F */ opi("ld a, a", 0, OperandKind::None, 0),
    /* 80 */ opi("add a, b", 0, OperandKind::None, 0),
    /* 81 */ opi("add a, c", 0, OperandKind::None, 0),
    /* 82 */ opi("add a, d", 0, OperandKind::None, 0),
    /* 83 */ opi("add a, e", 0, OperandKind::None, 0),
    /* 84 */ opi("add a, h", 0, OperandKind::None, 0),
    /* 85 */ opi("add a, l", 0, OperandKind::None, 0),
    /* 86 */ opi("add a, [hl]", 0, OperandKind::None, OPCODE_FLAG_READ_MEM),
    /* 87 */ opi("add a, a", 0, OperandKind::None, 0),
    /* 88 */ opi("adc a, b", 0, OperandKind::None, 0),
    /* 89 */ opi("adc a, c", 0, OperandKind::None, 0),
    /* 8A */ opi("adc a, d", 0, OperandKind::None, 0),
    /* 8B */ opi("adc a, e", 0, OperandKind::None, 0),
    /* 8C */ opi("adc a, h", 0, OperandKind::None, 0),
    /* 8D */ opi("adc a, l", 0, OperandKind::None, 0),
    /* 8E */ opi("adc a, [hl]", 0, OperandKind::None, OPCODE_FLAG_READ_MEM),
    /* 8F */ opi("adc a, a", 0, OperandKind::None, 0),
    /* 90 */ opi("sub a, b", 0, OperandKind::None, 0),
    /* 91 */ opi("sub a, c", 0, OperandKind::None, 0),
    /* 92 */ opi("sub a, d", 0, OperandKind::None, 0),
    /* 93 */ opi("sub a, e", 0, OperandKind::None, 0),
    /* 94 */ opi("sub a, h", 0, OperandKind::None, 0),
    /* 95 */ opi("sub a, l", 0, OperandKind::None, 0),
    /* 96 */ opi("sub a, [hl]", 0, OperandKind::None, OPCODE_FLAG_READ_MEM),
    /* 97 */ opi("sub a, a", 0, OperandKind::None, 0),
    /* 98 */ opi("sbc a, b", 0, OperandKind::None, 0),
    /* 99 */ opi("sbc a, c", 0, OperandKind::None, 0),
    /* 9A */ opi("sbc a, d", 0, OperandKind::None, 0),
    /* 9B */ opi("sbc a, e", 0, OperandKind::None, 0),
    /* 9C */ opi("sbc a, h", 0, OperandKind::None, 0),
    /* 9D */ opi("sbc a, l", 0, OperandKind::None, 0),
    /* 9E */ opi("sbc a, [hl]", 0, OperandKind::None, OPCODE_FLAG_READ_MEM),
    /* 9F */ opi("sbc a, a", 0, OperandKind::None, 0),
    /* A0 */ opi("and a, b", 0, OperandKind::None, 0),
    /* A1 */ opi("and a, c", 0, OperandKind::None, 0),
    /* A2 */ opi("and a, d", 0, OperandKind::None, 0),
    /* A3 */ opi("and a, e", 0, OperandKind::None, 0),
    /* A4 */ opi("and a, h", 0, OperandKind::None, 0),
    /* A5 */ opi("and a, l", 0, OperandKind::None, 0),
    /* A6 */ opi("and a, [hl]", 0, OperandKind::None, OPCODE_FLAG_READ_MEM),
    /* A7 */ opi("and a, a", 0, OperandKind::None, 0),
    /* A8 */ opi("xor a, b", 0, OperandKind::None, 0),
    /* A9 */ opi("xor a, c", 0, OperandKind::None, 0),
    /* AA */ opi("xor a, d", 0, OperandKind::None, 0),
    /* AB */ opi("xor a, e", 0, OperandKind::None, 0),
    /* AC */ opi("xor a, h", 0, OperandKind::None, 0),
    /* AD */ opi("xor a, l", 0, OperandKind::None, 0),
    /* AE */ opi("xor a, [hl]", 0, OperandKind::None, OPCODE_FLAG_READ_MEM),
    /* AF */ opi("xor a, a", 0, OperandKind::None, 0),
    /* B0 */ opi("or a, b", 0, OperandKind::None, 0),
    /* B1 */ opi("or a, c", 0, OperandKind::None, 0),
    /* B2 */ opi("or a, d", 0, OperandKind::None, 0),
    /* B3 */ opi("or a, e", 0, OperandKind::None, 0),
    /* B4 */ opi("or a, h", 0, OperandKind::None, 0),
    /* B5 */ opi("or a, l", 0, OperandKind::None, 0),
    /* B6 */ opi("or a, [hl]", 0, OperandKind::None, OPCODE_FLAG_READ_MEM),
    /* B7 */ opi("or a, a", 0, OperandKind::None, 0),
    /* B8 */ opi("cp a, b", 0, OperandKind::None, 0),
    /* B9 */ opi("cp a, c", 0, OperandKind::None, 0),
    /* BA */ opi("cp a, d", 0, OperandKind::None, 0),
    /* BB */ opi("cp a, e", 0, OperandKind::None, 0),
    /* BC */ opi("cp a, h", 0, OperandKind::None, 0),
    /* BD */ opi("cp a, l", 0, OperandKind::None, 0),
    /* BE */ opi("cp a, [hl]", 0, OperandKind::None, OPCODE_FLAG_READ_MEM),
    /* BF */ opi("cp a, a", 0, OperandKind::None, 0),
    /* C0 */ opi("ret nz", 0, OperandKind::None, OPCODE_FLAG_JUMP | OPCODE_FLAG_CONDITIONAL),
    /* C1 */ opi("pop bc", 0, OperandKind::None, 0),
    /* C2 */ opi("jp nz, %", 2, OperandKind::Code, OPCODE_FLAG_JUMP | OPCODE_FLAG_CONDITIONAL),
    /* C3 */ opi("jp %", 2, OperandKind::Code, OPCODE_FLAG_JUMP),
    /* C4 */ opi("call nz, %", 2, OperandKind::Code, OPCODE_FLAG_JUMP | OPCODE_FLAG_CALL | OPCODE_FLAG_CONDITIONAL),
    /* C5 */ opi("push bc", 0, OperandKind::None, 0),
    /* C6 */ opi("add a, %", 1, OperandKind::Undefined, 0),
    /* C7 */ opi("rst $0", 0, OperandKind::None, OPCODE_FLAG_JUMP | OPCODE_FLAG_CALL),
    /* C8 */ opi("ret z", 0, OperandKind::None, OPCODE_FLAG_JUMP | OPCODE_FLAG_CONDITIONAL),
    /* C9 */ opi("ret", 0, OperandKind::None, OPCODE_FLAG_JUMP),
    /* CA */ opi("jp z, %", 2, OperandKind::Code, OPCODE_FLAG_JUMP | OPCODE_FLAG_CONDITIONAL),
    /* CB */ opi("bitops", 1, OperandKind::None, 0),
    /* CC */ opi("call z, %", 2, OperandKind::Code, OPCODE_FLAG_JUMP | OPCODE_FLAG_CALL | OPCODE_FLAG_CONDITIONAL),
    /* CD */ opi("call %", 2, OperandKind::Code, OPCODE_FLAG_JUMP | OPCODE_FLAG_CALL),
    /* CE */ opi("adc a, %", 1, OperandKind::Undefined, 0),
    /* CF */ opi("rst $8", 0, OperandKind::None, OPCODE_FLAG_JUMP | OPCODE_FLAG_CALL),
    /* D0 */ opi("ret nc", 0, OperandKind::None, OPCODE_FLAG_JUMP | OPCODE_FLAG_CONDITIONAL),
    /* D1 */ opi("pop de", 0, OperandKind::None, 0),
    /* D2 */ opi("jp nc, %", 2, OperandKind::Code, OPCODE_FLAG_JUMP | OPCODE_FLAG_CONDITIONAL),
    /* D3 */ opi("", 0, OperandKind::None, OPCODE_FLAG_INVALID),
    /* D4 */ opi("call nc, %", 2, OperandKind::Code, OPCODE_FLAG_JUMP | OPCODE_FLAG_CALL | OPCODE_FLAG_CONDITIONAL),
    /* D5 */ opi("push de", 0, OperandKind::None, 0),
    /* D6 */ opi("sub a, %", 1, OperandKind::Undefined, 0),
    /* D7 */ opi("rst $10", 0, OperandKind::None, OPCODE_FLAG_JUMP | OPCODE_FLAG_CALL),
    /* D8 */ opi("ret c", 0, OperandKind::None, OPCODE_FLAG_JUMP | OPCODE_FLAG_CONDITIONAL),
    /* D9 */ opi("reti", 0, OperandKind::None, OPCODE_FLAG_JUMP),
    /* DA */ opi("jp c, %", 2, OperandKind::Code, OPCODE_FLAG_JUMP | OPCODE_FLAG_CONDITIONAL),
    /* DB */ opi("", 0, OperandKind::None, OPCODE_FLAG_INVALID),
    /* DC */ opi("call c, %", 2, OperandKind::Code, OPCODE_FLAG_JUMP | OPCODE_FLAG_CALL | OPCODE_FLAG_CONDITIONAL),
    /* DD */ opi("", 2, OperandKind::None, OPCODE_FLAG_INVALID),
    /* DE */ opi("sbc a, %", 1, OperandKind::Undefined, 0),
    /* DF */ opi("rst $18", 0, OperandKind::None, OPCODE_FLAG_JUMP | OPCODE_FLAG_CALL),
    /* E0 */ opi("ldh [%], a", 1, OperandKind::DataHram, OPCODE_FLAG_WRITE_MEM),
    /* E1 */ opi("pop hl", 0, OperandKind::None, 0),
    /* E2 */ opi("ld [$FF00+c], a", 0, OperandKind::None, OPCODE_FLAG_WRITE_MEM),
    /* E3 */ opi("", 0, OperandKind::None, OPCODE_FLAG_INVALID),
    /* E4 */ opi("", 0, OperandKind::None, OPCODE_FLAG_INVALID),
    /* E5 */ opi("push hl", 0, OperandKind::None, 0),
    /* E6 */ opi("and a, %", 1, OperandKind::Undefined, 0),
    /* E7 */ opi("rst $20", 0, OperandKind::None, OPCODE_FLAG_JUMP | OPCODE_FLAG_CALL),
    /* E8 */ opi("add sp, %", 1, OperandKind::Undefined, 0),
    /* E9 */ opi("jp hl", 0, OperandKind::None, OPCODE_FLAG_JUMP),
    /* EA */ opi("ld [%], a", 2, OperandKind::Data, OPCODE_FLAG_WRITE_MEM),
    /* EB */ opi("", 0, OperandKind::None, OPCODE_FLAG_INVALID),
    /* EC */ opi("", 2, OperandKind::None, OPCODE_FLAG_INVALID),
    /* ED */ opi("", 2, OperandKind::None, OPCODE_FLAG_INVALID),
    /* EE */ opi("xor a, %", 1, OperandKind::Undefined, 0),
    /* EF */ opi("rst $28", 0, OperandKind::None, OPCODE_FLAG_JUMP | OPCODE_FLAG_CALL),
    /* F0 */ opi("ldh a, [%]", 1, OperandKind::DataHram, OPCODE_FLAG_READ_MEM),
    /* F1 */ opi("pop af", 0, OperandKind::None, 0),
    /* F2 */ opi("ld a, [$FF00+c]", 0, OperandKind::None, OPCODE_FLAG_READ_MEM),
    /* F3 */ opi("di", 0, OperandKind::None, 0),
    /* F4 */ opi("", 0, OperandKind::None, OPCODE_FLAG_INVALID),
    /* F5 */ opi("push af", 0, OperandKind::None, 0),
    /* F6 */ opi("or a, %", 1, OperandKind::Undefined, 0),
    /* F7 */ opi("rst $30", 0, OperandKind::None, OPCODE_FLAG_JUMP | OPCODE_FLAG_CALL),
    /* F8 */ opi("ld hl, sp+%", 1, OperandKind::Undefined, 0),
    /* F9 */ opi("ld sp, hl", 0, OperandKind::None, 0),
    /* FA */ opi("ld a, [%]", 2, OperandKind::Data, OPCODE_FLAG_READ_MEM),
    /* FB */ opi("ei", 0, OperandKind::None, 0),
    /* FC */ opi("", 2, OperandKind::None, OPCODE_FLAG_INVALID),
    /* FD */ opi("", 2, OperandKind::None, OPCODE_FLAG_INVALID),
    /* FE */ opi("cp a, %", 1, OperandKind::Undefined, 0),
    /* FF */ opi("rst $38", 0, OperandKind::None, OPCODE_FLAG_JUMP | OPCODE_FLAG_CALL),
];

const BITOPS_INFO: [OpcodeInfo; 0x100] =
[
    /* 00 */ opi("rlc b", 1, OperandKind::LongOpcode, 0),
    /* 01 */ opi("rlc c", 1, OperandKind::LongOpcode, 0),
    /* 02 */ opi("rlc d", 1, OperandKind::LongOpcode, 0),
    /* 03 */ opi("rlc e", 1, OperandKind::LongOpcode, 0),
    /* 04 */ opi("rlc h", 1, OperandKind::LongOpcode, 0),
    /* 05 */ opi("rlc l", 1, OperandKind::LongOpcode, 0),
    /* 06 */ opi("rlc [hl]", 1, OperandKind::LongOpcode, OPCODE_FLAG_WRITE_MEM | OPCODE_FLAG_READ_MEM),
    /* 07 */ opi("rlc a", 1, OperandKind::LongOpcode, 0),
    /* 08 */ opi("rrc b", 1, OperandKind::LongOpcode, 0),
    /* 09 */ opi("rrc c", 1, OperandKind::LongOpcode, 0),
    /* 0A */ opi("rrc d", 1, OperandKind::LongOpcode, 0),
    /* 0B */ opi("rrc e", 1, OperandKind::LongOpcode, 0),
    /* 0C */ opi("rrc h", 1, OperandKind::LongOpcode, 0),
    /* 0D */ opi("rrc l", 1, OperandKind::LongOpcode, 0),
    /* 0E */ opi("rrc [hl]", 1, OperandKind::LongOpcode, OPCODE_FLAG_WRITE_MEM | OPCODE_FLAG_READ_MEM),
    /* 0F */ opi("rrc a", 1, OperandKind::LongOpcode, 0),
    /* 10 */ opi("rl b", 1, OperandKind::LongOpcode, 0),
    /* 11 */ opi("rl c", 1, OperandKind::LongOpcode, 0),
    /* 12 */ opi("rl d", 1, OperandKind::LongOpcode, 0),
    /* 13 */ opi("rl e", 1, OperandKind::LongOpcode, 0),
    /* 14 */ opi("rl h", 1, OperandKind::LongOpcode, 0),
    /* 15 */ opi("rl l", 1, OperandKind::LongOpcode, 0),
    /* 16 */ opi("rl [hl]", 1, OperandKind::LongOpcode, OPCODE_FLAG_WRITE_MEM | OPCODE_FLAG_READ_MEM),
    /* 17 */ opi("rl a", 1, OperandKind::LongOpcode, 0),
    /* 18 */ opi("rr b", 1, OperandKind::LongOpcode, 0),
    /* 19 */ opi("rr c", 1, OperandKind::LongOpcode, 0),
    /* 1A */ opi("rr d", 1, OperandKind::LongOpcode, 0),
    /* 1B */ opi("rr e", 1, OperandKind::LongOpcode, 0),
    /* 1C */ opi("rr h", 1, OperandKind::LongOpcode, 0),
    /* 1D */ opi("rr l", 1, OperandKind::LongOpcode, 0),
    /* 1E */ opi("rr [hl]", 1, OperandKind::LongOpcode, OPCODE_FLAG_WRITE_MEM | OPCODE_FLAG_READ_MEM),
    /* 1F */ opi("rr a", 1, OperandKind::LongOpcode, 0),
    /* 20 */ opi("sla b", 1, OperandKind::LongOpcode, 0),
    /* 21 */ opi("sla c", 1, OperandKind::LongOpcode, 0),
    /* 22 */ opi("sla d", 1, OperandKind::LongOpcode, 0),
    /* 23 */ opi("sla e", 1, OperandKind::LongOpcode, 0),
    /* 24 */ opi("sla h", 1, OperandKind::LongOpcode, 0),
    /* 25 */ opi("sla l", 1, OperandKind::LongOpcode, 0),
    /* 26 */ opi("sla [hl]", 1, OperandKind::LongOpcode, OPCODE_FLAG_WRITE_MEM | OPCODE_FLAG_READ_MEM),
    /* 27 */ opi("sla a", 1, OperandKind::LongOpcode, 0),
    /* 28 */ opi("sra b", 1, OperandKind::LongOpcode, 0),
    /* 29 */ opi("sra c", 1, OperandKind::LongOpcode, 0),
    /* 2A */ opi("sra d", 1, OperandKind::LongOpcode, 0),
    /* 2B */ opi("sra e", 1, OperandKind::LongOpcode, 0),
    /* 2C */ opi("sra h", 1, OperandKind::LongOpcode, 0),
    /* 2D */ opi("sra l", 1, OperandKind::LongOpcode, 0),
    /* 2E */ opi("sra [hl]", 1, OperandKind::LongOpcode, OPCODE_FLAG_WRITE_MEM | OPCODE_FLAG_READ_MEM),
    /* 2F */ opi("sra a", 1, OperandKind::LongOpcode, 0),
    /* 30 */ opi("swap b", 1, OperandKind::LongOpcode, 0),
    /* 31 */ opi("swap c", 1, OperandKind::LongOpcode, 0),
    /* 32 */ opi("swap d", 1, OperandKind::LongOpcode, 0),
    /* 33 */ opi("swap e", 1, OperandKind::LongOpcode, 0),
    /* 34 */ opi("swap h", 1, OperandKind::LongOpcode, 0),
    /* 35 */ opi("swap l", 1, OperandKind::LongOpcode, 0),
    /* 36 */ opi("swap [hl]", 1, OperandKind::LongOpcode, OPCODE_FLAG_WRITE_MEM | OPCODE_FLAG_READ_MEM),
    /* 37 */ opi("swap a", 1, OperandKind::LongOpcode, 0),
    /* 38 */ opi("srl b", 1, OperandKind::LongOpcode, 0),
    /* 39 */ opi("srl c", 1, OperandKind::LongOpcode, 0),
    /* 3A */ opi("srl d", 1, OperandKind::LongOpcode, 0),
    /* 3B */ opi("srl e", 1, OperandKind::LongOpcode, 0),
    /* 3C */ opi("srl h", 1, OperandKind::LongOpcode, 0),
    /* 3D */ opi("srl l", 1, OperandKind::LongOpcode, 0),
    /* 3E */ opi("srl [hl]", 1, OperandKind::LongOpcode, OPCODE_FLAG_WRITE_MEM | OPCODE_FLAG_READ_MEM),
    /* 3F */ opi("srl a", 1, OperandKind::LongOpcode, 0),
    /* 40 */ opi("bit 0, b", 1, OperandKind::LongOpcode, 0),
    /* 41 */ opi("bit 0, c", 1, OperandKind::LongOpcode, 0),
    /* 42 */ opi("bit 0, d", 1, OperandKind::LongOpcode, 0),
    /* 43 */ opi("bit 0, e", 1, OperandKind::LongOpcode, 0),
    /* 44 */ opi("bit 0, h", 1, OperandKind::LongOpcode, 0),
    /* 45 */ opi("bit 0, l", 1, OperandKind::LongOpcode, 0),
    /* 46 */ opi("bit 0, [hl]", 1, OperandKind::LongOpcode, OPCODE_FLAG_READ_MEM),
    /* 47 */ opi("bit 0, a", 1, OperandKind::LongOpcode, 0),
    /* 48 */ opi("bit 1, b", 1, OperandKind::LongOpcode, 0),
    /* 49 */ opi("bit 1, c", 1, OperandKind::LongOpcode, 0),
    /* 4A */ opi("bit 1, d", 1, OperandKind::LongOpcode, 0),
    /* 4B */ opi("bit 1, e", 1, OperandKind::LongOpcode, 0),
    /* 4C */ opi("bit 1, h", 1, OperandKind::LongOpcode, 0),
    /* 4D */ opi("bit 1, l", 1, OperandKind::LongOpcode, 0),
    /* 4E */ opi("bit 1, [hl]", 1, OperandKind::LongOpcode, OPCODE_FLAG_READ_MEM),
    /* 4F */ opi("bit 1, a", 1, OperandKind::LongOpcode, 0),
    /* 50 */ opi("bit 2, b", 1, OperandKind::LongOpcode, 0),
    /* 51 */ opi("bit 2, c", 1, OperandKind::LongOpcode, 0),
    /* 52 */ opi("bit 2, d", 1, OperandKind::LongOpcode, 0),
    /* 53 */ opi("bit 2, e", 1, OperandKind::LongOpcode, 0),
    /* 54 */ opi("bit 2, h", 1, OperandKind::LongOpcode, 0),
    /* 55 */ opi("bit 2, l", 1, OperandKind::LongOpcode, 0),
    /* 56 */ opi("bit 2, [hl]", 1, OperandKind::LongOpcode, OPCODE_FLAG_READ_MEM),
    /* 57 */ opi("bit 2, a", 1, OperandKind::LongOpcode, 0),
    /* 58 */ opi("bit 3, b", 1, OperandKind::LongOpcode, 0),
    /* 59 */ opi("bit 3, c", 1, OperandKind::LongOpcode, 0),
    /* 5A */ opi("bit 3, d", 1, OperandKind::LongOpcode, 0),
    /* 5B */ opi("bit 3, e", 1, OperandKind::LongOpcode, 0),
    /* 5C */ opi("bit 3, h", 1, OperandKind::LongOpcode, 0),
    /* 5D */ opi("bit 3, l", 1, OperandKind::LongOpcode, 0),
    /* 5E */ opi("bit 3, [hl]", 1, OperandKind::LongOpcode, OPCODE_FLAG_READ_MEM),
    /* 5F */ opi("bit 3, a", 1, OperandKind::LongOpcode, 0),
    /* 60 */ opi("bit 4, b", 1, OperandKind::LongOpcode, 0),
    /* 61 */ opi("bit 4, c", 1, OperandKind::LongOpcode, 0),
    /* 62 */ opi("bit 4, d", 1, OperandKind::LongOpcode, 0),
    /* 63 */ opi("bit 4, e", 1, OperandKind::LongOpcode, 0),
    /* 64 */ opi("bit 4, h", 1, OperandKind::LongOpcode, 0),
    /* 65 */ opi("bit 4, l", 1, OperandKind::LongOpcode, 0),
    /* 66 */ opi("bit 4, [hl]", 1, OperandKind::LongOpcode, OPCODE_FLAG_READ_MEM),
    /* 67 */ opi("bit 4, a", 1, OperandKind::LongOpcode, 0),
    /* 68 */ opi("bit 5, b", 1, OperandKind::LongOpcode, 0),
    /* 69 */ opi("bit 5, c", 1, OperandKind::LongOpcode, 0),
    /* 6A */ opi("bit 5, d", 1, OperandKind::LongOpcode, 0),
    /* 6B */ opi("bit 5, e", 1, OperandKind::LongOpcode, 0),
    /* 6C */ opi("bit 5, h", 1, OperandKind::LongOpcode, 0),
    /* 6D */ opi("bit 5, l", 1, OperandKind::LongOpcode, 0),
    /* 6E */ opi("bit 5, [hl]", 1, OperandKind::LongOpcode, OPCODE_FLAG_READ_MEM),
    /* 6F */ opi("bit 5, a", 1, OperandKind::LongOpcode, 0),
    /* 70 */ opi("bit 6, b", 1, OperandKind::LongOpcode, 0),
    /* 71 */ opi("bit 6, c", 1, OperandKind::LongOpcode, 0),
    /* 72 */ opi("bit 6, d", 1, OperandKind::LongOpcode, 0),
    /* 73 */ opi("bit 6, e", 1, OperandKind::LongOpcode, 0),
    /* 74 */ opi("bit 6, h", 1, OperandKind::LongOpcode, 0),
    /* 75 */ opi("bit 6, l", 1, OperandKind::LongOpcode, 0),
    /* 76 */ opi("bit 6, [hl]", 1, OperandKind::LongOpcode, OPCODE_FLAG_READ_MEM),
    /* 77 */ opi("bit 6, a", 1, OperandKind::LongOpcode, 0),
    /* 78 */ opi("bit 7, b", 1, OperandKind::LongOpcode, 0),
    /* 79 */ opi("bit 7, c", 1, OperandKind::LongOpcode, 0),
    /* 7A */ opi("bit 7, d", 1, OperandKind::LongOpcode, 0),
    /* 7B */ opi("bit 7, e", 1, OperandKind::LongOpcode, 0),
    /* 7C */ opi("bit 7, h", 1, OperandKind::LongOpcode, 0),
    /* 7D */ opi("bit 7, l", 1, OperandKind::LongOpcode, 0),
    /* 7E */ opi("bit 7, [hl]", 1, OperandKind::LongOpcode, OPCODE_FLAG_READ_MEM),
    /* 7F */ opi("bit 7, a", 1, OperandKind::LongOpcode, 0),
    /* 80 */ opi("res 0, b", 1, OperandKind::LongOpcode, 0),
    /* 81 */ opi("res 0, c", 1, OperandKind::LongOpcode, 0),
    /* 82 */ opi("res 0, d", 1, OperandKind::LongOpcode, 0),
    /* 83 */ opi("res 0, e", 1, OperandKind::LongOpcode, 0),
    /* 84 */ opi("res 0, h", 1, OperandKind::LongOpcode, 0),
    /* 85 */ opi("res 0, l", 1, OperandKind::LongOpcode, 0),
    /* 86 */ opi("res 0, [hl]", 1, OperandKind::LongOpcode, OPCODE_FLAG_WRITE_MEM),
    /* 87 */ opi("res 0, a", 1, OperandKind::LongOpcode, 0),
    /* 88 */ opi("res 1, b", 1, OperandKind::LongOpcode, 0),
    /* 89 */ opi("res 1, c", 1, OperandKind::LongOpcode, 0),
    /* 8A */ opi("res 1, d", 1, OperandKind::LongOpcode, 0),
    /* 8B */ opi("res 1, e", 1, OperandKind::LongOpcode, 0),
    /* 8C */ opi("res 1, h", 1, OperandKind::LongOpcode, 0),
    /* 8D */ opi("res 1, l", 1, OperandKind::LongOpcode, 0),
    /* 8E */ opi("res 1, [hl]", 1, OperandKind::LongOpcode, OPCODE_FLAG_WRITE_MEM),
    /* 8F */ opi("res 1, a", 1, OperandKind::LongOpcode, 0),
    /* 90 */ opi("res 2, b", 1, OperandKind::LongOpcode, 0),
    /* 91 */ opi("res 2, c", 1, OperandKind::LongOpcode, 0),
    /* 92 */ opi("res 2, d", 1, OperandKind::LongOpcode, 0),
    /* 93 */ opi("res 2, e", 1, OperandKind::LongOpcode, 0),
    /* 94 */ opi("res 2, h", 1, OperandKind::LongOpcode, 0),
    /* 95 */ opi("res 2, l", 1, OperandKind::LongOpcode, 0),
    /* 96 */ opi("res 2, [hl]", 1, OperandKind::LongOpcode, OPCODE_FLAG_WRITE_MEM),
    /* 97 */ opi("res 2, a", 1, OperandKind::LongOpcode, 0),
    /* 98 */ opi("res 3, b", 1, OperandKind::LongOpcode, 0),
    /* 99 */ opi("res 3, c", 1, OperandKind::LongOpcode, 0),
    /* 9A */ opi("res 3, d", 1, OperandKind::LongOpcode, 0),
    /* 9B */ opi("res 3, e", 1, OperandKind::LongOpcode, 0),
    /* 9C */ opi("res 3, h", 1, OperandKind::LongOpcode, 0),
    /* 9D */ opi("res 3, l", 1, OperandKind::LongOpcode, 0),
    /* 9E */ opi("res 3, [hl]", 1, OperandKind::LongOpcode, OPCODE_FLAG_WRITE_MEM),
    /* 9F */ opi("res 3, a", 1, OperandKind::LongOpcode, 0),
    /* A0 */ opi("res 4, b", 1, OperandKind::LongOpcode, 0),
    /* A1 */ opi("res 4, c", 1, OperandKind::LongOpcode, 0),
    /* A2 */ opi("res 4, d", 1, OperandKind::LongOpcode, 0),
    /* A3 */ opi("res 4, e", 1, OperandKind::LongOpcode, 0),
    /* A4 */ opi("res 4, h", 1, OperandKind::LongOpcode, 0),
    /* A5 */ opi("res 4, l", 1, OperandKind::LongOpcode, 0),
    /* A6 */ opi("res 4, [hl]", 1, OperandKind::LongOpcode, OPCODE_FLAG_WRITE_MEM),
    /* A7 */ opi("res 4, a", 1, OperandKind::LongOpcode, 0),
    /* A8 */ opi("res 5, b", 1, OperandKind::LongOpcode, 0),
    /* A9 */ opi("res 5, c", 1, OperandKind::LongOpcode, 0),
    /* AA */ opi("res 5, d", 1, OperandKind::LongOpcode, 0),
    /* AB */ opi("res 5, e", 1, OperandKind::LongOpcode, 0),
    /* AC */ opi("res 5, h", 1, OperandKind::LongOpcode, 0),
    /* AD */ opi("res 5, l", 1, OperandKind::LongOpcode, 0),
    /* AE */ opi("res 5, [hl]", 1, OperandKind::LongOpcode, OPCODE_FLAG_WRITE_MEM),
    /* AF */ opi("res 5, a", 1, OperandKind::LongOpcode, 0),
    /* B0 */ opi("res 6, b", 1, OperandKind::LongOpcode, 0),
    /* B1 */ opi("res 6, c", 1, OperandKind::LongOpcode, 0),
    /* B2 */ opi("res 6, d", 1, OperandKind::LongOpcode, 0),
    /* B3 */ opi("res 6, e", 1, OperandKind::LongOpcode, 0),
    /* B4 */ opi("res 6, h", 1, OperandKind::LongOpcode, 0),
    /* B5 */ opi("res 6, l", 1, OperandKind::LongOpcode, 0),
    /* B6 */ opi("res 6, [hl]", 1, OperandKind::LongOpcode, OPCODE_FLAG_WRITE_MEM),
    /* B7 */ opi("res 6, a", 1, OperandKind::LongOpcode, 0),
    /* B8 */ opi("res 7, b", 1, OperandKind::LongOpcode, 0),
    /* B9 */ opi("res 7, c", 1, OperandKind::LongOpcode, 0),
    /* BA */ opi("res 7, d", 1, OperandKind::LongOpcode, 0),
    /* BB */ opi("res 7, e", 1, OperandKind::LongOpcode, 0),
    /* BC */ opi("res 7, h", 1, OperandKind::LongOpcode, 0),
    /* BD */ opi("res 7, l", 1, OperandKind::LongOpcode, 0),
    /* BE */ opi("res 7, [hl]", 1, OperandKind::LongOpcode, OPCODE_FLAG_WRITE_MEM),
    /* BF */ opi("res 7, a", 1, OperandKind::LongOpcode, 0),
    /* C0 */ opi("set 0, b", 1, OperandKind::LongOpcode, 0),
    /* C1 */ opi("set 0, c", 1, OperandKind::LongOpcode, 0),
    /* C2 */ opi("set 0, d", 1, OperandKind::LongOpcode, 0),
    /* C3 */ opi("set 0, e", 1, OperandKind::LongOpcode, 0),
    /* C4 */ opi("set 0, h", 1, OperandKind::LongOpcode, 0),
    /* C5 */ opi("set 0, l", 1, OperandKind::LongOpcode, 0),
    /* C6 */ opi("set 0, [hl]", 1, OperandKind::LongOpcode, OPCODE_FLAG_WRITE_MEM),
    /* C7 */ opi("set 0, a", 1, OperandKind::LongOpcode, 0),
    /* C8 */ opi("set 1, b", 1, OperandKind::LongOpcode, 0),
    /* C9 */ opi("set 1, c", 1, OperandKind::LongOpcode, 0),
    /* CA */ opi("set 1, d", 1, OperandKind::LongOpcode, 0),
    /* CB */ opi("set 1, e", 1, OperandKind::LongOpcode, 0),
    /* CC */ opi("set 1, h", 1, OperandKind::LongOpcode, 0),
    /* CD */ opi("set 1, l", 1, OperandKind::LongOpcode, 0),
    /* CE */ opi("set 1, [hl]", 1, OperandKind::LongOpcode, OPCODE_FLAG_WRITE_MEM),
    /* CF */ opi("set 1, a", 1, OperandKind::LongOpcode, 0),
    /* D0 */ opi("set 2, b", 1, OperandKind::LongOpcode, 0),
    /* D1 */ opi("set 2, c", 1, OperandKind::LongOpcode, 0),
    /* D2 */ opi("set 2, d", 1, OperandKind::LongOpcode, 0),
    /* D3 */ opi("set 2, e", 1, OperandKind::LongOpcode, 0),
    /* D4 */ opi("set 2, h", 1, OperandKind::LongOpcode, 0),
    /* D5 */ opi("set 2, l", 1, OperandKind::LongOpcode, 0),
    /* D6 */ opi("set 2, [hl]", 1, OperandKind::LongOpcode, OPCODE_FLAG_WRITE_MEM),
    /* D7 */ opi("set 2, a", 1, OperandKind::LongOpcode, 0),
    /* D8 */ opi("set 3, b", 1, OperandKind::LongOpcode, 0),
    /* D9 */ opi("set 3, c", 1, OperandKind::LongOpcode, 0),
    /* DA */ opi("set 3, d", 1, OperandKind::LongOpcode, 0),
    /* DB */ opi("set 3, e", 1, OperandKind::LongOpcode, 0),
    /* DC */ opi("set 3, h", 1, OperandKind::LongOpcode, 0),
    /* DD */ opi("set 3, l", 1, OperandKind::LongOpcode, 0),
    /* DE */ opi("set 3, [hl]", 1, OperandKind::LongOpcode, OPCODE_FLAG_WRITE_MEM),
    /* DF */ opi("set 3, a", 1, OperandKind::LongOpcode, 0),
    /* E0 */ opi("set 4, b", 1, OperandKind::LongOpcode, 0),
    /* E1 */ opi("set 4, c", 1, OperandKind::LongOpcode, 0),
    /* E2 */ opi("set 4, d", 1, OperandKind::LongOpcode, 0),
    /* E3 */ opi("set 4, e", 1, OperandKind::LongOpcode, 0),
    /* E4 */ opi("set 4, h", 1, OperandKind::LongOpcode, 0),
    /* E5 */ opi("set 4, l", 1, OperandKind::LongOpcode, 0),
    /* E6 */ opi("set 4, [hl]", 1, OperandKind::LongOpcode, OPCODE_FLAG_WRITE_MEM),
    /* E7 */ opi("set 4, a", 1, OperandKind::LongOpcode, 0),
    /* E8 */ opi("set 5, b", 1, OperandKind::LongOpcode, 0),
    /* E9 */ opi("set 5, c", 1, OperandKind::LongOpcode, 0),
    /* EA */ opi("set 5, d", 1, OperandKind::LongOpcode, 0),
    /* EB */ opi("set 5, e", 1, OperandKind::LongOpcode, 0),
    /* EC */ opi("set 5, h", 1, OperandKind::LongOpcode, 0),
    /* ED */ opi("set 5, l", 1, OperandKind::LongOpcode, 0),
    /* EE */ opi("set 5, [hl]", 1, OperandKind::LongOpcode, OPCODE_FLAG_WRITE_MEM),
    /* EF */ opi("set 5, a", 1, OperandKind::LongOpcode, 0),
    /* F0 */ opi("set 6, b", 1, OperandKind::LongOpcode, 0),
    /* F1 */ opi("set 6, c", 1, OperandKind::LongOpcode, 0),
    /* F2 */ opi("set 6, d", 1, OperandKind::LongOpcode, 0),
    /* F3 */ opi("set 6, e", 1, OperandKind::LongOpcode, 0),
    /* F4 */ opi("set 6, h", 1, OperandKind::LongOpcode, 0),
    /* F5 */ opi("set 6, l", 1, OperandKind::LongOpcode, 0),
    /* F6 */ opi("set 6, [hl]", 1, OperandKind::LongOpcode, OPCODE_FLAG_WRITE_MEM),
    /* F7 */ opi("set 6, a", 1, OperandKind::LongOpcode, 0),
    /* F8 */ opi("set 7, b", 1, OperandKind::LongOpcode, 0),
    /* F9 */ opi("set 7, c", 1, OperandKind::LongOpcode, 0),
    /* FA */ opi("set 7, d", 1, OperandKind::LongOpcode, 0),
    /* FB */ opi("set 7, e", 1, OperandKind::LongOpcode, 0),
    /* FC */ opi("set 7, h", 1, OperandKind::LongOpcode, 0),
    /* FD */ opi("set 7, l", 1, OperandKind::LongOpcode, 0),
    /* FE */ opi("set 7, [hl]", 1, OperandKind::LongOpcode, OPCODE_FLAG_WRITE_MEM),
    /* FF */ opi("set 7, a", 1, OperandKind::LongOpcode, 0),
];
