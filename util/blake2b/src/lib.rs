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

use std::io;
use aion_types::H256;

pub const BLOCK_BYTES: usize = 128;
pub const KEY_BYTES: usize = 64;
pub const OUT_BYTES: usize = 64;

static IV: [u64; 8] = [
    0x6a09e667f3bcc908,
    0xbb67ae8584caa73b,
    0x3c6ef372fe94f82b,
    0xa54ff53a5f1d36f1,
    0x510e527fade682d1,
    0x9b05688c2b3e6c1f,
    0x1f83d9abfb41bd6b,
    0x5be0cd19137e2179,
];

static SIGMA: [[u8; 16]; 12] = [
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
    [14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3],
    [11, 8, 12, 0, 5, 2, 15, 13, 10, 14, 3, 6, 7, 1, 9, 4],
    [7, 9, 3, 1, 13, 12, 11, 14, 2, 6, 5, 10, 4, 0, 15, 8],
    [9, 0, 5, 7, 2, 4, 10, 15, 14, 1, 11, 12, 6, 8, 3, 13],
    [2, 12, 6, 10, 0, 11, 8, 3, 4, 13, 7, 5, 15, 14, 1, 9],
    [12, 5, 1, 15, 14, 13, 4, 10, 0, 7, 6, 3, 9, 2, 8, 11],
    [13, 11, 7, 14, 12, 1, 3, 9, 5, 0, 15, 4, 8, 6, 2, 10],
    [6, 15, 14, 9, 11, 3, 0, 8, 12, 2, 13, 7, 1, 4, 10, 5],
    [10, 2, 8, 4, 7, 6, 1, 5, 15, 11, 9, 14, 3, 12, 13, 0],
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
    [14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3],
];

/// Get the BLAKE2B (i.e. BLAKE2B) hash of the empty bytes string.
pub const BLAKE2B_EMPTY: H256 = H256([
    0x0e, 0x57, 0x51, 0xc0, 0x26, 0xe5, 0x43, 0xb2, 0xe8, 0xab, 0x2e, 0xb0, 0x60, 0x99, 0xda, 0xa1,
    0xd1, 0xe5, 0xdf, 0x47, 0x77, 0x8f, 0x77, 0x87, 0xfa, 0xab, 0x45, 0xcd, 0xf1, 0x2f, 0xe3, 0xa8,
]);

/// The BLAKE2B of the RLP encoding of empty data.
///
pub const BLAKE2B_NULL_RLP: H256 = H256([
    0x45, 0xb0, 0xcf, 0xc2, 0x20, 0xce, 0xec, 0x5b, 0x7c, 0x1c, 0x62, 0xc4, 0xd4, 0x19, 0x3d, 0x38,
    0xe4, 0xeb, 0xa4, 0x8e, 0x88, 0x15, 0x72, 0x9c, 0xe7, 0x5f, 0x9c, 0x0a, 0xb0, 0xe4, 0xc1, 0xc0,
]);

/// The BLAKE2B of the RLP encoding of empty list.
///
pub const BLAKE2B_EMPTY_LIST_RLP: H256 = H256([
    0xda, 0x22, 0x3b, 0x09, 0x96, 0x7c, 0x5b, 0xd2, 0x11, 0x07, 0x43, 0x30, 0x7e, 0x0a, 0xf6, 0xd3,
    0x9f, 0x61, 0x72, 0x0a, 0xa7, 0x21, 0x8a, 0x64, 0x0a, 0x08, 0xee, 0xd1, 0x2d, 0xd5, 0x75, 0xc7,
]);

pub struct Blake2b {
    h: [u64; 8],
    t: [u64; 2],
    f: [u64; 2],
    buf: [u8; 2 * BLOCK_BYTES],
    buf_len: usize,
}

impl Copy for Blake2b {}

impl Clone for Blake2b {
    fn clone(&self) -> Blake2b { *self }
}

impl Blake2b {
    pub fn new(size: usize) -> Blake2b {
        assert!(size > 0 && size <= OUT_BYTES);

        let param = encode_params(size as u8, 0);
        let mut state = IV;

        for i in 0..state.len() {
            state[i] ^= load64(&param[i * 8..]);
        }

        Blake2b {
            h: state,
            t: [0, 0],
            f: [0, 0],
            buf: [0u8; 2 * BLOCK_BYTES],
            buf_len: 0,
        }
    }

    /// specify key which is used in equihash.
    pub fn new_with_key(size: usize, key: &[u8]) -> Blake2b {
        assert!(size > 0 && size <= OUT_BYTES);
        assert!(key.len() > 0 && key.len() <= KEY_BYTES);

        let param = encode_params(size as u8, key.len() as u8);
        let mut state = IV;

        for i in 0..state.len() {
            state[i] ^= load64(&param[i * 8..]);
        }

        let mut b = Blake2b {
            h: state,
            t: [0, 0],
            f: [0, 0],
            buf: [0u8; 2 * BLOCK_BYTES],
            buf_len: 0,
        };

        let mut block = [0u8; BLOCK_BYTES];
        for i in 0..key.len() {
            block[i] = key[i];
        }
        b.update(block.as_ref());
        b
    }

    // support initialize blake2b with the given encoded parameters.
    // add this to support personalization in equihash.
    pub fn with_params(param: &[u8; 64]) -> Blake2b {
        let size: usize = param[0] as usize;
        assert!(size > 0 && size <= OUT_BYTES);

        let mut state = IV;

        for i in 0..state.len() {
            state[i] ^= load64(&param[i * 8..]);
        }

        Blake2b {
            h: state,
            t: [0, 0],
            f: [0, 0],
            buf: [0u8; 2 * BLOCK_BYTES],
            buf_len: 0,
        }
    }

    pub fn hash_256(input_bytes: &[u8]) -> [u8; 32] {
        let mut h = Blake2b::new(32);
        let mut out = [0u8; 32];
        h.update(&input_bytes);
        h.finalize(&mut out[..32]);
        out
    }

    pub fn update(&mut self, m: &[u8]) {
        let mut m = m;

        while m.len() > 0 {
            let left = self.buf_len;
            let fill = 2 * BLOCK_BYTES - left;

            if m.len() > fill {
                for i in 0..fill {
                    self.buf[left + i] = m[i];
                }
                self.buf_len += fill;
                m = &m[fill..];
                self.increment_counter(BLOCK_BYTES as u64);
                self.compress();
                for i in 0..BLOCK_BYTES {
                    self.buf[i] = self.buf[i + BLOCK_BYTES];
                }
                self.buf_len -= BLOCK_BYTES;
            } else {
                for i in 0..m.len() {
                    self.buf[left + i] = m[i];
                }
                self.buf_len += m.len();
                m = &m[m.len()..];
            }
        }
    }

    pub fn finalize(&mut self, out: &mut [u8]) {
        let mut buf = [0u8; OUT_BYTES];
        if self.buf_len > BLOCK_BYTES {
            self.increment_counter(BLOCK_BYTES as u64);
            self.compress();
            for i in 0..BLOCK_BYTES {
                self.buf[i] = self.buf[i + BLOCK_BYTES];
            }
            self.buf_len -= BLOCK_BYTES;
        }
        let n = self.buf_len as u64;
        self.increment_counter(n);
        self.f[0] = !0;
        for i in self.buf_len..self.buf.len() {
            self.buf[i] = 0;
        }
        self.compress();
        for i in 0..self.h.len() {
            store64(&mut buf[i * 8..], self.h[i]);
        }
        for i in 0..::std::cmp::min(out.len(), OUT_BYTES) {
            out[i] = buf[i];
        }
    }

    fn increment_counter(&mut self, inc: u64) {
        self.t[0] += inc;
        self.t[1] += if self.t[0] < inc { 1 } else { 0 };
    }

    fn compress(&mut self) {
        let mut m = [0u64; 16];
        let mut v = [0u64; 16];
        let block = self.buf.as_ref();

        assert!(block.len() >= BLOCK_BYTES);

        for i in 0..m.len() {
            m[i] = load64(&block[i * 8..]);
        }

        for i in 0..8 {
            v[i] = self.h[i];
        }

        v[8] = IV[0];
        v[9] = IV[1];
        v[10] = IV[2];
        v[11] = IV[3];
        v[12] = self.t[0] ^ IV[4];
        v[13] = self.t[1] ^ IV[5];
        v[14] = self.f[0] ^ IV[6];
        v[15] = self.f[1] ^ IV[7];

        macro_rules! g(
            ($r: expr, $i: expr, $a: expr, $b: expr, $c: expr, $d: expr) => ({
                $a = $a.wrapping_add($b).wrapping_add(m[SIGMA[$r][2*$i+0] as usize]);
                $d = ($d ^ $a).rotate_right(32);
                $c = $c.wrapping_add($d);
                $b = ($b ^ $c).rotate_right(24);
                $a = $a.wrapping_add($b).wrapping_add(m[SIGMA[$r][2*$i+1] as usize]);
                $d = ($d ^ $a).rotate_right(16);
                $c = $c.wrapping_add($d);
                $b = ($b ^ $c).rotate_right(63);
            });
        );

        macro_rules! round(
            ($r: expr) => ({
                g!($r, 0, v[ 0], v[ 4], v[ 8], v[12]);
                g!($r, 1, v[ 1], v[ 5], v[ 9], v[13]);
                g!($r, 2, v[ 2], v[ 6], v[10], v[14]);
                g!($r, 3, v[ 3], v[ 7], v[11], v[15]);
                g!($r, 4, v[ 0], v[ 5], v[10], v[15]);
                g!($r, 5, v[ 1], v[ 6], v[11], v[12]);
                g!($r, 6, v[ 2], v[ 7], v[ 8], v[13]);
                g!($r, 7, v[ 3], v[ 4], v[ 9], v[14]);
            });
        );

        for i in 0..12 {
            round!(i);
        }

        for i in 0..8 {
            self.h[i] = self.h[i] ^ v[i] ^ v[i + 8];
        }
    }
}

fn encode_params(size: u8, keylen: u8) -> [u8; 64] {
    let mut param = [0u8; 64];
    param[0] = size as u8;
    param[1] = keylen as u8;
    param[2] = 1; // fanout
    param[3] = 1; // depth
    param
}

fn load64(b: &[u8]) -> u64 {
    let mut v = 0u64;
    for i in 0..8 {
        v |= (b[i] as u64) << (8 * i);
    }
    v
}

fn store64(b: &mut [u8], v: u64) {
    let mut w = v;
    for i in 0..8 {
        b[i] = w as u8;
        w >>= 8;
    }
}

pub fn blake2b<T: AsRef<[u8]>>(s: T) -> H256 {
    let mut result = [0u8; 32];
    let input = s.as_ref();
    let mut blake2b = Blake2b::new(32);
    blake2b.update(&input);
    blake2b.finalize(&mut result);
    H256(result)
}

pub fn blake2b_buffer(r: &mut dyn io::BufRead) -> Result<H256, io::Error> {
    let mut output = [0u8; 32];
    let mut input = [0u8; 1024];
    let mut blake2b = Blake2b::new(32);

    // read file
    loop {
        let some = r.read(&mut input)?;
        if some == 0 {
            break;
        }
        blake2b.update(&input[0..some]);
    }

    blake2b.finalize(&mut output);
    Ok(output.into())
}
