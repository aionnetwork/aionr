/*******************************************************************************
 * Copyright (c) 2015-2018 Parity Technologies (UK) Ltd.
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

#[macro_use]
extern crate hex_literal;

use ethbloom::{Bloom, Input};

#[test]
fn it_works() {
    let bloom: Bloom = "0x00000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000000000900000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000".into();
    let address = hex!("ef2d6d194084c2de36e0dabfce45d046b37d1106");
    let topic = hex!("02c69be41d0b7e40352fc85be1cd65eb03d40ef8427a0ca4596b1ead9a00e9fc");

    let mut my_bloom = Bloom::default();
    assert!(!my_bloom.contains_input(Input::Raw(&address)));
    assert!(!my_bloom.contains_input(Input::Raw(&topic)));

    my_bloom.accrue(Input::Raw(&address));
    assert!(my_bloom.contains_input(Input::Raw(&address)));
    assert!(!my_bloom.contains_input(Input::Raw(&topic)));

    my_bloom.accrue(Input::Raw(&topic));
    assert!(my_bloom.contains_input(Input::Raw(&address)));
    assert!(my_bloom.contains_input(Input::Raw(&topic)));
    assert_eq!(my_bloom, bloom);
}
