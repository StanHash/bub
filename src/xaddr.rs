/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

use std::ops::{Add, AddAssign};

#[derive(Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct XAddr
{
    pub bank: u16,
    pub addr: u16,
}

impl XAddr
{
    pub fn new(bank: u16, addr: u16) -> Self
    {
        Self
        {
            bank: bank,
            addr: addr,
        }
    }
}

impl std::fmt::Display for XAddr
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result
    {
        write!(f, "{:02X}:{:04X}", self.bank, self.addr)
    }
}

impl From<XAddr> for u16
{
    fn from(xa: XAddr) -> u16
    {
        xa.addr
    }
}

impl AddAssign<u16> for XAddr
{
    fn add_assign(&mut self, rhs: u16)
    {
        self.addr += rhs;
    }
}

impl Add<u16> for XAddr
{
    type Output = XAddr;

    fn add(self, rhs: u16) -> XAddr
    {
        XAddr::new(self.bank, self.addr + rhs)
    }
}

impl Add<XAddr> for u16
{
    type Output = XAddr;

    fn add(self, rhs: XAddr) -> XAddr
    {
        XAddr::new(rhs.bank, rhs.addr + self)
    }
}

pub mod prelude
{
    pub use super::XAddr;
}
