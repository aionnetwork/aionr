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
//! A fixed point number for delta_calc

extern crate num_bigint;
extern crate bigdecimal;
extern crate num_traits;
#[macro_use]
extern crate lazy_static;

#[cfg(test)]
extern crate rand;

mod log_approximator;

use std::fmt;

use num_bigint::{BigUint,ToBigInt};
use bigdecimal::BigDecimal;
use num_traits::{Zero,One,ToPrimitive,Num};

pub use log_approximator::LogApproximator;

const PRECISION: usize = 70;

lazy_static! {
    static ref MAX_PRECISION: FixedPoint = FixedPoint(BigUint::one() << PRECISION);
}

/// FixedPoint error type
#[derive(Debug)]
pub enum FixedPointError {
    /// When there is a negative result
    Negative,
}

impl fmt::Display for FixedPointError {
    fn fmt(&self, f: &mut fmt::Formatter) -> ::std::fmt::Result {
        f.write_str(match *self {
            FixedPointError::Negative => "Should not be negative",
        })
    }
}

/// 70-bit precision fixed point unsigned number
#[derive(PartialEq, PartialOrd, Debug, Clone)]
pub struct FixedPoint(BigUint);

impl FixedPoint {
    /// Creates a fixed-point number with a given BigDecimal.
    ///
    /// Returns Error when the given number is negative
    pub fn parse_from_big_decimal(num: &BigDecimal) -> Result<FixedPoint, FixedPointError> {
        let temp = (num * MAX_PRECISION.0.to_bigint().unwrap())
            .to_bigint()
            .unwrap();
        if temp >= Zero::zero() {
            Ok(FixedPoint(temp.to_biguint().unwrap()))
        } else {
            Err(FixedPointError::Negative)
        }
    }

    /// Creates a fixed-point number with a given number string and a given radix.
    ///
    /// Returns Error when the parsed number is negative
    pub fn from_str_radix(s: &str, radix: u32) -> Result<FixedPoint, FixedPointError> {
        let num = BigDecimal::from_str_radix(s, radix).unwrap();
        Self::parse_from_big_decimal(&num)
    }

    /// Rounds down and converts to BigUint
    pub fn to_big_uint(self) -> BigUint { self.0 >> PRECISION }

    /// Addition. Returns the sum.
    pub fn add(&self, addend: &FixedPoint) -> FixedPoint {
        let res = &self.0 + &addend.0;
        FixedPoint(res)
    }

    /// Subtraction. Returns the diffrence, or Error on negetive
    pub fn subtract(&self, subtractend: &FixedPoint) -> Result<FixedPoint, FixedPointError> {
        if self.0 < subtractend.0 {
            Err(FixedPointError::Negative)
        } else {
            Ok(FixedPoint(&self.0 - &subtractend.0))
        }
    }

    /// multiplication by a BigInt. Returns the product.
    pub fn multiply_uint(&self, multiplicand: BigUint) -> FixedPoint {
        FixedPoint(&self.0 * multiplicand)
    }

    /// Shift right with the given number
    pub fn divide_by_power_of_two(&self, shift: &usize) -> FixedPoint {
        FixedPoint(&self.0 >> *shift)
    }
}

impl From<BigUint> for FixedPoint {
    fn from(value: BigUint) -> FixedPoint { FixedPoint(value) }
}

impl From<FixedPoint> for u64 {
    fn from(value: FixedPoint) -> u64 {
        let temp = value.0 >> PRECISION;
        match temp.to_u64() {
            Some(v) => v,
            // TODO: if None ?
            None => u64::max_value(),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use num_traits::Num;

    #[test]
    fn test_add() {
        let fixed33 = FixedPoint(BigUint::from(33u64));
        let fixed47 = FixedPoint(BigUint::from(47u64));
        assert_eq!(fixed33.add(&fixed47), FixedPoint(BigUint::from(80u64)));
    }

    #[test]
    fn test_parse_from_big_decimal() {
        let n1 = BigDecimal::from_str_radix("0.40546510810816438486", 10).unwrap();
        let fixed_n1 = FixedPoint::parse_from_big_decimal(&n1).unwrap();

        assert_eq!(
            fixed_n1,
            FixedPoint(BigUint::parse_bytes(b"478688709125778178049", 10).unwrap())
        );

        let n2 = BigDecimal::from_str_radix("-0.40546510810816438486", 10).unwrap();
        let fixed_n2 = FixedPoint::parse_from_big_decimal(&n2);

        assert!(fixed_n2.is_err());
    }

    #[test]
    fn test_subtract() {
        let fixed50 = FixedPoint(BigUint::from(50u64));
        let fixed60 = FixedPoint(BigUint::from(60u64));
        let fixed40 = FixedPoint(BigUint::from(40u64));
        assert_eq!(
            fixed50.subtract(&fixed40).unwrap(),
            FixedPoint(BigUint::from(10u64))
        );
        assert!(fixed50.subtract(&fixed60).is_err());
    }

    #[test]
    fn test_multiply_uint() {
        let fixed50 = FixedPoint(BigUint::from(50u64));
        assert_eq!(
            fixed50.multiply_uint(BigUint::from(50u64)),
            FixedPoint(BigUint::from(2500u64))
        )
    }

    #[test]
    fn test_divide_by_power_of_two() {
        let fixed50 = FixedPoint(BigUint::from(50u64));
        assert_eq!(
            fixed50.divide_by_power_of_two(&4),
            FixedPoint(BigUint::from(3u64))
        )
    }

}
