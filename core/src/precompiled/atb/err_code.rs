/*******************************************************************************
 * Copyright (c) 2018-2019 Aion foundation.
 *
 *     This file is part of the aion network project.
 *
 *     The aion network project is free software: you can redistribute it
 *     and/or modify it under the terms of the GNU General Public License
 *     as published by the Free Software Foundation, either version 3 of
 *     the License, or any later version.
 *
 *     The aion network project is distributed in the hope that it will
 *     be useful, but WITHOUT ANY WARRANTY; without even the implied
 *     warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.
 *     See the GNU General Public License for more details.
 *
 *     You should have received a copy of the GNU General Public License
 *     along with the aion network project source files.
 *     If not, see <https://www.gnu.org/licenses/>.
 *
 ******************************************************************************/

#[allow(unused)]
#[derive(Debug, PartialEq)]
pub enum ErrCode {
    NotOwner = 0x1,
    NotNewOwner = 0x2,
    RingLocked = 0x3,
    RingNotLocked = 0x4,
    RingMemberExists = 0x5,
    RingMemberNotExists = 0x6,
    NotRingMember = 0x7,
    NotEnoughSignatures = 0x8,
    InvalidSignatureBounds = 0x9,
    InvalidTransfer = 0xA,
    NotRelayer = 0xB,
    Processed = 0xC,
    UncaughtError = 0x1337,
}
