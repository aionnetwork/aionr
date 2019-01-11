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

static RET_HEADER_LEN: usize = 3;
static HEADER_LEN: usize = 4;
pub static HASH_LEN: usize = 8;

pub fn to_return_header(vers: u8, ret_code: i32) -> Vec<u8> {
    let mut ret_header = Vec::with_capacity(RET_HEADER_LEN);
    ret_header.push(vers);
    ret_header.push(ret_code as u8);
    ret_header.push(0u8);
    ret_header
}

pub fn get_api_msg_hash(request: &Vec<u8>) -> Vec<u8> {
    if request.is_empty() || request[3] == 0 {
        return vec![];
    }
    request.as_slice()[HEADER_LEN..HEADER_LEN + HASH_LEN].to_vec()
}

pub fn to_return_header_with_hash(vers: u8, ret_code: i32, hash: Vec<u8>) -> Vec<u8> {
    if hash.is_empty() || hash.len() != HASH_LEN {
        return vec![];
    }
    let mut ret_header = Vec::with_capacity(HASH_LEN + RET_HEADER_LEN);
    ret_header.push(vers);
    ret_header.push(ret_code as u8);
    ret_header.push(1u8);
    ret_header.extend_from_slice(&hash);
    ret_header
}

pub fn to_return_header_with_hash_and_error(
    vers: u8,
    ret_code: i32,
    hash: &[u8],
    error: &[u8],
) -> Vec<u8>
{
    if hash.len() == 0 || hash.len() != HASH_LEN {
        return vec![];
    }

    let mut ret_header = Vec::with_capacity(HASH_LEN + RET_HEADER_LEN + 1 + error.len());
    ret_header.push(vers);
    ret_header.push(ret_code as u8);
    ret_header.push(1u8);
    ret_header.extend_from_slice(hash);
    ret_header.push(error.len() as u8);
    ret_header.extend_from_slice(error);
    ret_header
}

pub fn to_return_header_with_hash_error_and_result(
    vers: u8,
    ret_code: i32,
    hash: &[u8],
    error: &[u8],
    result: &[u8],
) -> Vec<u8>
{
    if hash.len() == 0 || hash.len() != HASH_LEN {
        return vec![];
    }

    let mut ret_header =
        Vec::with_capacity(HASH_LEN + RET_HEADER_LEN + 1 + error.len() + result.len());
    ret_header.push(vers);
    ret_header.push(ret_code as u8);
    ret_header.push(1u8);
    ret_header.extend_from_slice(hash);
    ret_header.push(error.len() as u8);
    ret_header.extend_from_slice(error);
    ret_header.extend_from_slice(result);

    ret_header
}

pub fn combine_ret_msg(header: Vec<u8>, body: Vec<u8>) -> Vec<u8> {
    vec![header, body]
        .into_iter()
        .flat_map(|x| x.into_iter())
        .collect()
}
