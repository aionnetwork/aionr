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

#![warn(unused_extern_crates)]

extern crate fastvm;
extern crate vm_common;
extern crate aion_types;
extern crate rlp;

use aion_types::U256;
use fastvm::vm::CostType;
use rlp::RlpStream;
use vm_common::CallType;

#[test]
fn encode_calltype() {
    let mut s = RlpStream::new();
    s.append(&CallType::None); // 0
    assert_eq!(s.as_raw(), [0x80]);
    s.append(&CallType::Call); // 1
    assert_eq!(s.as_raw(), [0x80, 1]);
    s.append(&CallType::CallCode); // 2
    assert_eq!(s.as_raw(), [0x80, 1, 2]);
    s.append(&CallType::DelegateCall); // 3
    assert_eq!(s.as_raw(), [0x80, 1, 2, 3]);
    s.append(&CallType::StaticCall); // 4

    assert_eq!(s.as_raw(), [0x80u8, 1, 2, 3, 4]);
}

#[test]
fn overflowing_add() {
    let left: U256 = U256::max_value();

    let res = left.overflowing_add(0.into());
    assert_eq!(res.1, false);
    let res = left.overflowing_add(1.into());
    assert_eq!(res.1, true);
}

#[test]
fn should_calculate_overflow_mul_shr_without_overflow() {
    // given
    let num = 1048576;

    // when
    let (res1, o1) = U256::from(num).overflow_mul_shr(U256::from(num), 20);
    let (res2, o2) = num.overflow_mul_shr(num, 20);

    // then
    assert_eq!(res1, U256::from(num));
    assert!(!o1);
    assert_eq!(res2, num);
    assert!(!o2);
}

#[test]
fn should_calculate_overflow_mul_shr_with_overflow() {
    // given
    let max = u64::max_value();
    let num1 = U256([max, max, max, max]);
    let num2 = usize::max_value();

    // when
    let (res1, o1) = num1.overflow_mul_shr(num1, 256);
    let (res2, o2) = num2.overflow_mul_shr(num2, 64);

    // then
    assert_eq!(res2, num2 - 1);
    assert!(o2);

    assert_eq!(res1, U256::max_value() - U256::one());
    assert!(o1);
}

#[test]
fn should_validate_u256_to_usize_conversion() {
    // given
    let v = U256::from(usize::max_value()) + U256::from(1);

    // when
    let res = usize::from_u256(v);

    // then
    assert!(res.is_err());
}
