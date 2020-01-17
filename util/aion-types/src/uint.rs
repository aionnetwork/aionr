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

#[cfg(feature = "serialize")]
use serde::{Serialize, Serializer, Deserialize, Deserializer};

#[cfg(feature = "serialize")]
use ethereum_types_serialize;

use num_bigint::BigUint;

construct_uint!(pub struct U64(1););
construct_uint!(pub struct U128(2););
construct_uint!(pub struct U256(4););
construct_uint!(pub struct U512(8););

impl U256 {
    /// Multiplies two 256-bit integers to produce full 512-bit integer
    /// No overflow possible
    #[cfg(all(asm_available, target_arch = "x86_64"))]
    pub fn full_mul(self, other: U256) -> U512 {
        let self_t: &[u64; 4] = &self.0;
        let other_t: &[u64; 4] = &other.0;
        let mut result: [u64; 8] = unsafe { ::core::mem::uninitialized() };
        unsafe {
            asm!("
                mov $8, %rax
                mulq $12
                mov %rax, $0
                mov %rdx, $1

                mov $8, %rax
                mulq $13
                add %rax, $1
                adc $$0, %rdx
                mov %rdx, $2

                mov $8, %rax
                mulq $14
                add %rax, $2
                adc $$0, %rdx
                mov %rdx, $3

                mov $8, %rax
                mulq $15
                add %rax, $3
                adc $$0, %rdx
                mov %rdx, $4

                mov $9, %rax
                mulq $12
                add %rax, $1
                adc %rdx, $2
                adc $$0, $3
                adc $$0, $4
                xor $5, $5
                adc $$0, $5
                xor $6, $6
                adc $$0, $6
                xor $7, $7
                adc $$0, $7

                mov $9, %rax
                mulq $13
                add %rax, $2
                adc %rdx, $3
                adc $$0, $4
                adc $$0, $5
                adc $$0, $6
                adc $$0, $7

                mov $9, %rax
                mulq $14
                add %rax, $3
                adc %rdx, $4
                adc $$0, $5
                adc $$0, $6
                adc $$0, $7

                mov $9, %rax
                mulq $15
                add %rax, $4
                adc %rdx, $5
                adc $$0, $6
                adc $$0, $7

                mov $10, %rax
                mulq $12
                add %rax, $2
                adc %rdx, $3
                adc $$0, $4
                adc $$0, $5
                adc $$0, $6
                adc $$0, $7

                mov $10, %rax
                mulq $13
                add %rax, $3
                adc %rdx, $4
                adc $$0, $5
                adc $$0, $6
                adc $$0, $7

                mov $10, %rax
                mulq $14
                add %rax, $4
                adc %rdx, $5
                adc $$0, $6
                adc $$0, $7

                mov $10, %rax
                mulq $15
                add %rax, $5
                adc %rdx, $6
                adc $$0, $7

                mov $11, %rax
                mulq $12
                add %rax, $3
                adc %rdx, $4
                adc $$0, $5
                adc $$0, $6
                adc $$0, $7

                mov $11, %rax
                mulq $13
                add %rax, $4
                adc %rdx, $5
                adc $$0, $6
                adc $$0, $7

                mov $11, %rax
                mulq $14
                add %rax, $5
                adc %rdx, $6
                adc $$0, $7

                mov $11, %rax
                mulq $15
                add %rax, $6
                adc %rdx, $7
                "
            : /* $0 */ "={r8}"(result[0]), /* $1 */ "={r9}"(result[1]), /* $2 */ "={r10}"(result[2]),
              /* $3 */ "={r11}"(result[3]), /* $4 */ "={r12}"(result[4]), /* $5 */ "={r13}"(result[5]),
              /* $6 */ "={r14}"(result[6]), /* $7 */ "={r15}"(result[7])

            : /* $8 */ "m"(self_t[0]), /* $9 */ "m"(self_t[1]), /* $10 */  "m"(self_t[2]),
              /* $11 */ "m"(self_t[3]), /* $12 */ "m"(other_t[0]), /* $13 */ "m"(other_t[1]),
              /* $14 */ "m"(other_t[2]), /* $15 */ "m"(other_t[3])
            : "rax", "rdx"
            :
            );
        }
        U512(result)
    }

    /// Multiplies two 256-bit integers to produce full 512-bit integer
    /// No overflow possible
    #[inline(always)]
    #[cfg(not(all(asm_available, target_arch = "x86_64")))]
    pub fn full_mul(self, other: U256) -> U512 { U512(uint_full_mul_reg!(U256, 4, self, other)) }

    pub fn as_f64(self) -> f64 {
        if self == U256::zero() {
            return 0f64;
        }
        let bits = self.bits();
        let exp = (bits as u64 - 2 + 0x3ff) << 52;
        let t1;
        if bits == 53 {
            t1 = self;
        } else if bits > 53 {
            t1 = self >> (bits - 53);
        } else {
            t1 = self << (53 - bits);
        }

        let t2 = t1.as_u64() + exp;

        f64::from_bits(t2)
    }
}

impl From<U256> for U512 {
    fn from(value: U256) -> U512 {
        let U256(ref arr) = value;
        let mut ret = [0; 8];
        ret[0] = arr[0];
        ret[1] = arr[1];
        ret[2] = arr[2];
        ret[3] = arr[3];
        U512(ret)
    }
}

impl From<U512> for U256 {
    fn from(value: U512) -> U256 {
        let U512(ref arr) = value;
        if arr[4] | arr[5] | arr[6] | arr[7] != 0 {
            panic!("Overflow");
        }
        let mut ret = [0; 4];
        ret[0] = arr[0];
        ret[1] = arr[1];
        ret[2] = arr[2];
        ret[3] = arr[3];
        U256(ret)
    }
}

impl<'a> From<&'a U256> for U512 {
    fn from(value: &'a U256) -> U512 {
        let U256(ref arr) = *value;
        let mut ret = [0; 8];
        ret[0] = arr[0];
        ret[1] = arr[1];
        ret[2] = arr[2];
        ret[3] = arr[3];
        U512(ret)
    }
}

impl<'a> From<&'a U512> for U256 {
    fn from(value: &'a U512) -> U256 {
        let U512(ref arr) = *value;
        if arr[4] | arr[5] | arr[6] | arr[7] != 0 {
            panic!("Overflow");
        }
        let mut ret = [0; 4];
        ret[0] = arr[0];
        ret[1] = arr[1];
        ret[2] = arr[2];
        ret[3] = arr[3];
        U256(ret)
    }
}

impl From<U256> for U128 {
    fn from(value: U256) -> U128 {
        let U256(ref arr) = value;
        if arr[2] | arr[3] != 0 {
            panic!("Overflow");
        }
        let mut ret = [0; 2];
        ret[0] = arr[0];
        ret[1] = arr[1];
        U128(ret)
    }
}

impl From<U512> for U128 {
    fn from(value: U512) -> U128 {
        let U512(ref arr) = value;
        if arr[2] | arr[3] | arr[4] | arr[5] | arr[6] | arr[7] != 0 {
            panic!("Overflow");
        }
        let mut ret = [0; 2];
        ret[0] = arr[0];
        ret[1] = arr[1];
        U128(ret)
    }
}

impl From<U128> for U512 {
    fn from(value: U128) -> U512 {
        let U128(ref arr) = value;
        let mut ret = [0; 8];
        ret[0] = arr[0];
        ret[1] = arr[1];
        U512(ret)
    }
}

impl From<U128> for U256 {
    fn from(value: U128) -> U256 {
        let U128(ref arr) = value;
        let mut ret = [0; 4];
        ret[0] = arr[0];
        ret[1] = arr[1];
        U256(ret)
    }
}

macro_rules! impl_serde {
    ($name:ident, $len:expr) => {
        #[cfg(feature = "serialize")]
        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where S: Serializer {
                let mut slice = [0u8; 2 + 2 * $len * 8];
                let mut bytes = [0u8; $len * 8];
                self.to_big_endian(&mut bytes);
                ethereum_types_serialize::serialize_uint(&mut slice, &bytes, serializer)
            }
        }

        #[cfg(feature = "serialize")]
        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where D: Deserializer<'de> {
                let mut bytes = [0u8; $len * 8];
                let wrote = ethereum_types_serialize::deserialize_check_len(
                    deserializer,
                    ethereum_types_serialize::ExpectedLen::Between(0, &mut bytes),
                )?;
                Ok(bytes[0..wrote].into())
            }
        }
    };
}

impl_serde!(U64, 1);
impl_serde!(U128, 2);
impl_serde!(U256, 4);
impl_serde!(U512, 8);

impl From<U256> for BigUint {
    fn from(value: U256) -> BigUint {
        let arr: [u8; 32] = value.into();
        BigUint::from_bytes_be(&arr)
    }
}

impl From<BigUint> for U256 {
    fn from(value: BigUint) -> U256 {
        let mut le = value.to_bytes_le();
        // TODO: consider when bytes in BigUint is larger than 32
        // it is enough for our calculation
        le.resize(32, 0);
        U256::from_little_endian(&le)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn test_from_to() {
        let a = U256::from(258u64);
        let b: BigUint = a.into();
        assert_eq!(b, BigUint::from(258u64));
        let c: U256 = b.into();
        assert_eq!(c, U256::from(258u64));
    }

    #[test]
    fn test_as_f64() {
        let a = U256::from("123456789abcdef01234567");
        let a_s = format!("{}", a);
        let f = a.as_f64();
        let f_s = format!("{}", f);
        assert_eq!(a_s[..15], f_s[..15]);
    }
}
