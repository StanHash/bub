/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

use std::io::BufRead;
use thiserror::Error;

use super::xaddr::prelude::*;

#[derive(Debug)]
pub enum Tag
{
    Name(String),
    Code,
    NoReturn,
    RomBank(u16),
    RamBank(u16),
    SrmBank(u16),
    OperandAddr,
}

pub fn get_tags_at<'a>(dict: &'a [(XAddr, Tag)], xa: &XAddr) -> &'a [(XAddr, Tag)]
{
    use superslice::*;
    &dict[dict.equal_range_by_key(xa, |xt| xt.0)]
}

#[derive(Error, Debug)]
pub enum ParseTagsError
{
    #[error("IO error")]
    Io(#[from] std::io::Error),

    #[error("Parse Int error")]
    ParseInt(#[from] std::num::ParseIntError),

    #[error("Invalid address field")]
    InvalidAddressField,

    #[error("Missing tag")]
    MissingTag,

    #[error("Missing tag argument")]
    MissingTagArgument,
}

pub fn parse_tags<R>(read: &mut R) -> Result<Vec<(XAddr, Tag)>, ParseTagsError>
    where R: BufRead
{
    let mut result = vec![];

    for line in read.lines()
    {
        let line = line?;
        let line = line.trim();

        if line.is_empty() || line.starts_with(';') {
            continue; }

        let mut split = line.split(char::is_whitespace);

        // parse address

        let xa =
        {
            let opt_str_addr = split.next();
            let str_addr = opt_str_addr.unwrap(); // since trimmed line is not empty, there must be at least one part in the line

            let str_addr_components: Vec<&str> = str_addr.split(':').collect();

            match str_addr_components.len()
            {
                1 => XAddr::new(0, u16::from_str_radix(&str_addr_components[0], 16)?),
                2 => XAddr::new(u16::from_str_radix(&str_addr_components[0], 16)?, u16::from_str_radix(&str_addr_components[1], 16)?),
                _ => return Err(ParseTagsError::InvalidAddressField),
            }
        };

        // parse tag

        let opt_str_tag = split.next();

        if let None = opt_str_tag {
            return Err(ParseTagsError::MissingTag); }

        let tag = match opt_str_tag.unwrap()
        {
            ".code" => Tag::Code,
            ".noreturn" => Tag::NoReturn,

            ".bank" | ".rombank" => Tag::RomBank(match split.next() {
                None => return Err(ParseTagsError::MissingTagArgument),
                Some(str_bank) => str_bank.parse()? }),

            ".rambank" => Tag::RamBank(match split.next() {
                None => return Err(ParseTagsError::MissingTagArgument),
                Some(str_bank) => str_bank.parse()? }),

            ".srambank" => Tag::SrmBank(match split.next() {
                None => return Err(ParseTagsError::MissingTagArgument),
                Some(str_bank) => str_bank.parse()? }),

            ".addr" => Tag::OperandAddr,

            str_tag => Tag::Name(str_tag.to_string()),
        };

        result.push((xa, tag));
    }

    result.sort_by_key(|&(xa, _)| xa);

    Ok(result)
}
