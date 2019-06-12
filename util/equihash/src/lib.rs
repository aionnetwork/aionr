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

extern crate acore_bytes as bytes;
extern crate blake2b;
#[macro_use]
extern crate log;
extern crate rustc_hex as hex;

use std::collections::HashSet;
use bytes::bytes_to_i32s;
use bytes::i32_to_bytes;
use bytes::i32_to_bytes_le;
use blake2b::Blake2b;
use std::ptr;

fn extend_array(input: &[u8], output: &mut [u8], bit_len: i32, byte_pad: i32) {
    let out_width: i32 = (bit_len + 7) / 8 + byte_pad;
    let bit_len_mask: i32 = (1 << bit_len) - 1;
    let mut acc_bits: i32 = 0;
    let mut acc_value: i32 = 0;
    let mut j = 0;
    for i in 0..input.len() {
        acc_value = (acc_value << 8) | ((input[i] as i32) & 0xff);
        acc_bits += 8;

        if acc_bits >= bit_len {
            acc_bits -= bit_len;

            for x in byte_pad..out_width {
                let temp = 8 * (out_width - x - 1);
                output[(j + x) as usize] = ((
                    // Big-endian
                    // it's >>> in java.
                    (acc_value as u32) >> ((acc_bits + temp) % 32)
                ) & (
                    // Apply bit_len_mask across byte boundaries
                    ((bit_len_mask as u32) >> (temp % 32)) & 0xFF
                )) as u8;
            }
            j = j + out_width;
        }
    }
}

pub struct EquihashValidator {
    n: i32,
    k: i32,
    indices_hash_length: usize,
    collision_bit_length: i32,
    solution_width: i32,
}

impl EquihashValidator {
    pub fn new(n: i32, k: i32) -> EquihashValidator {
        let indices_per_hash_output = 512 / n;
        let indices_hash_length = (n + 7) / 8;
        //        let hash_output = indices_per_hash_output * indices_hash_length;
        let collision_bit_length = n / (k + 1);
        let solution_width = (1 << k) * (collision_bit_length + 1) / 8;
        debug!(target: "equihash", "equihash validator - solution_width={}", solution_width);
        debug!(target: "equihash", "equihash validator - collision_bit_length={}", collision_bit_length);
        //        trace!(target: "equihash", "equihash validator - hash_output={}", hash_output);
        debug!(target: "equihash", "equihash validator - indices_hash_length={}", indices_hash_length);
        debug!(target: "equihash", "equihash validator - indices_per_hash_output={}", indices_per_hash_output);
        EquihashValidator {
            n,
            k,
            //            indices_per_hash_output,
            indices_hash_length: indices_hash_length as usize,
            //            hash_output,
            collision_bit_length,
            solution_width,
        }
    }

    pub fn is_valid_solution(&self, solution: &[u8], block_header: &[u8], nonce: &[u8]) -> bool {
        if solution.len() as i32 != self.solution_width {
            error!(target: "equihash", "Invalid solution width: {}", solution.len());
            return false;
        }

        let indices: Vec<i32> = self.get_indices_from_minimal(solution, self.collision_bit_length);
        if self.has_duplicate(&indices) {
            error!(target: "equihash", "Invalid solution - duplicate solution index");
            return false;
        }

        let mut personalization: Vec<u8> = Vec::with_capacity(16);
        personalization.extend_from_slice("AION0PoW".as_bytes());
        personalization.extend_from_slice(&i32_to_bytes_le(self.n));
        personalization.extend_from_slice(&i32_to_bytes_le(self.k));
        let native_hash = self.get_solution_hash(
            &personalization.as_slice(),
            nonce,
            indices.as_slice(),
            block_header,
        );

        let mut hash: Vec<u8> = Vec::with_capacity(self.indices_hash_length);
        for _i in 0..self.indices_hash_length {
            hash.push(0u8);
        }
        self.verify(&indices, 0, hash.as_mut_slice(), self.k, &native_hash)
    }

    pub fn has_duplicate(&self, indices: &Vec<i32>) -> bool {
        let mut set: HashSet<i32> = HashSet::with_capacity(512);
        for index in indices {
            if !set.insert(*index) {
                return true;
            }
        }
        false
    }

    pub fn get_indices_from_minimal(&self, minimal: &[u8], c_bit_len: i32) -> Vec<i32> {
        let len_indices = 8 * 4 * minimal.len() / (c_bit_len as usize + 1);
        let byte_pad = 4 - ((c_bit_len + 1) + 7) / 8;

        let mut arr: Vec<u8> = Vec::with_capacity(len_indices);
        for _i in 0..len_indices {
            arr.push(0u8);
        }
        extend_array(minimal, arr.as_mut_slice(), c_bit_len + 1, byte_pad);

        let ret_len = arr.len() / 4;
        let mut ret: Vec<i32> = Vec::with_capacity(ret_len);
        for _i in 0..ret_len {
            ret.push(0);
        }
        bytes_to_i32s(arr.as_slice(), ret.as_mut_slice(), true);
        ret
    }

    pub fn get_solution_hash(
        &self,
        personalization: &[u8],
        nonce: &[u8],
        indices: &[i32],
        header: &[u8],
    ) -> Vec<[u8; 27]>
    {
        let hashesperblake: i32 = 2;

        let mut param = [0u8; 64];
        param[0] = 54;
        param[2] = 1;
        param[3] = 1;
        param[48..64].copy_from_slice(&personalization[..16]);

        let mut out: Vec<[u8; 27]> = Vec::new();
        let indice_len = indices.len();
        for i in 0..indice_len {
            let mut blake2b = Blake2b::with_params(&param);
            blake2b.update(header);
            blake2b.update(nonce);
            let leb: i32 = (indices[i] / hashesperblake).to_le();
            blake2b.update(&i32_to_bytes(leb));
            let mut blakehash = [0u8; 54];
            blake2b.finalize(&mut blakehash);

            unsafe {
                let mut index_hash: [u8; 27] = [0u8; 27];

                let s = ((indices[i] % hashesperblake) * (self.n + 7) / 8) as usize;
                ptr::copy_nonoverlapping(
                    blakehash[s..].as_ptr(),
                    index_hash.as_mut_ptr(),
                    ((self.n + 7) / 8) as usize,
                );
                out.push(index_hash);
            }
        }

        out
    }

    fn verify(
        &self,
        indices: &Vec<i32>,
        index: i32,
        hash: &mut [u8],
        round: i32,
        hashes: &Vec<[u8; 27]>,
    ) -> bool
    {
        if round == 0 {
            return true;
        }

        let index1 = index + (1 << ((round - 1) % 32));
        if indices[index as usize] >= indices[index1 as usize] {
            error!(target: "equihash", "Solution validation failed - indices out of order");
            return false;
        }

        let mut hash0 = hashes[index as usize];
        let mut hash1 = hashes[index1 as usize];
        let verify0 = self.verify(&indices, index, &mut hash0, round - 1, &hashes);
        if !verify0 {
            error!(target: "equihash", "Solution validation failed - unable to verify left subtree");
            return false;
        }

        let verify1 = self.verify(&indices, index1, &mut hash1, round - 1, &hashes);
        if !verify1 {
            error!(target: "equihash", "Solution validation failed - unable to verify right subtree");
            return false;
        }

        for i in 0..(self.indices_hash_length) {
            hash[i] = hash0[i] ^ hash1[i];
        }

        let mut bits = self.n;
        if round < self.k {
            bits = self.collision_bit_length;
        }
        for i in 0..((bits / 8) as usize) {
            if hash[i] != 0 {
                error!(target: "equihash", "Solution validation failed - Non-zero XOR");
                return false;
            }
        }

        // check remainder bits
        if (bits % 8) > 0 && (hash[(bits / 8) as usize] >> (8 - (bits % 8))) != 0 {
            error!(target: "equihash", "Solution validation failed - Non-zero XOR");
            return false;
        }

        true
    }
}
