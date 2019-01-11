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

extern crate proc_macro;
extern crate syn;
#[macro_use]
extern crate quote;

mod en;
mod de;

use proc_macro::TokenStream;
use en::{impl_encodable, impl_encodable_wrapper};
use de::{impl_decodable, impl_decodable_wrapper};

#[proc_macro_derive(RlpEncodable)]
pub fn encodable(input: TokenStream) -> TokenStream {
    let s = input.to_string();
    let ast = syn::parse_derive_input(&s).unwrap();
    let gen = impl_encodable(&ast);
    gen.parse().unwrap()
}

#[proc_macro_derive(RlpEncodableWrapper)]
pub fn encodable_wrapper(input: TokenStream) -> TokenStream {
    let s = input.to_string();
    let ast = syn::parse_derive_input(&s).unwrap();
    let gen = impl_encodable_wrapper(&ast);
    gen.parse().unwrap()
}

#[proc_macro_derive(RlpDecodable)]
pub fn decodable(input: TokenStream) -> TokenStream {
    let s = input.to_string();
    let ast = syn::parse_derive_input(&s).unwrap();
    let gen = impl_decodable(&ast);
    gen.parse().unwrap()
}

#[proc_macro_derive(RlpDecodableWrapper)]
pub fn decodable_wrapper(input: TokenStream) -> TokenStream {
    let s = input.to_string();
    let ast = syn::parse_derive_input(&s).unwrap();
    let gen = impl_decodable_wrapper(&ast);
    gen.parse().unwrap()
}
