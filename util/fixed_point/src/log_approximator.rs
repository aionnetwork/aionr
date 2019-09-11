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
use super::{FixedPoint,MAX_PRECISION};
use num_bigint::BigUint;
use bigdecimal::BigDecimal;
use num_traits::Num;

const HARD_CODE_LNS: [&str; 31] = [
    "0",
    "0.40546510810816438486",
    "0.22314355131420976486",
    "0.11778303565638345574",
    "0.06062462181643483994",
    "0.03077165866675368733",
    "0.01550418653596525448",
    "0.00778214044205494896",
    "0.00389864041565732289",
    "0.00195122013126174934",
    "0.00097608597305545892",
    "0.00048816207950135119",
    "0.00024411082752736271",
    "0.00012206286252567737",
    "0.00006103329368063853",
    "0.00003051711247318638",
    "0.00001525867264836240",
    "0.00000762936542756757",
    "0.00000381468998968589",
    "0.00000190734681382541",
    "0.00000095367386165919",
    "0.00000047683704451632",
    "0.00000023841855067986",
    "0.00000011920928244535",
    "0.00000005960464299903",
    "0.00000002980232194361",
    "0.00000001490116108283",
    "0.00000000745058056917",
    "0.00000000372529029152",
    "0.00000000186264514750",
    "0.00000000093132257418",
];

lazy_static! {
    static ref LN_TABLE: Vec<FixedPoint> = {
        let mut ln_table = Vec::new();
        for s in HARD_CODE_LNS.iter() {
            ln_table.push(
                FixedPoint::parse_from_big_decimal(&BigDecimal::from_str_radix(*s, 10).unwrap())
                    .unwrap(),
            );
        }
        ln_table
    };
    static ref LN2: FixedPoint = FixedPoint::parse_from_big_decimal(
        &BigDecimal::from_str_radix("0.693147180559945309", 10).unwrap()
    )
    .unwrap();
}

pub trait LogApproximator {
    fn ln(input: &BigUint) -> FixedPoint;
    fn ln2() -> FixedPoint { LN2.clone() }
}

impl LogApproximator for FixedPoint {
    fn ln(input: &BigUint) -> FixedPoint {
        // put input in the range [0.1, 1)
        let bit_len = input.bits();

        let mut y = (*LN2).multiply_uint(bit_len.into());

        let mut x = FixedPoint(if super::PRECISION >= bit_len {
            input << super::PRECISION - bit_len
        } else {
            input >> bit_len - super::PRECISION
        });

        // We maintain, as an invariant, that y = log(input) - log(x)

        // Multiply x by factors in the sequence 3/2, 5/4, 9/8...

        assert!(x <= *MAX_PRECISION);

        if x == *MAX_PRECISION {
            return y;
        }

        for left_shift in 1..LN_TABLE.len() {
            let x_prime = x.add(&x.divide_by_power_of_two(&left_shift));

            // if xPrime is less than or equal to one, this is the best factor we can multiplyInteger by
            if x_prime < *MAX_PRECISION {
                x = x_prime;
                y = y
                    .subtruct(&(*LN_TABLE)[left_shift])
                    .expect("FixedPoint sub ln failed");
            } else if x_prime == *MAX_PRECISION {
                y = y
                    .subtruct(&(*LN_TABLE)[left_shift])
                    .expect("FixedPoint sub ln failed");
                return y;
            }
            // otherwise, try the next factor (eg. 1.01 was too big, so try 1.001)
        }

        y = y
            .add(&x)
            .subtruct(&*MAX_PRECISION)
            .expect("FixedPoint sub one failed");

        y
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use num_traits::{ToPrimitive};
    use rand::{thread_rng, Rng};
    use std::cmp::Ordering;

    fn assert_ln(test_num: u64) -> bool {
        let a = FixedPoint::ln(&test_num.into()).0;
        let a_len = a.bits();
        let shift = a_len - 53;
        let a = a >> shift;
        let fix = a.to_u64().unwrap();
        // u64 53 bits
        let f64 = (test_num as f64).ln().to_bits() % (1u64 << 52) + (1u64 << 52);

        match fix.cmp(&f64) {
            Ordering::Greater => fix - f64 == 1,
            Ordering::Less => f64 - fix == 1,
            Ordering::Equal => true,
        }
    }

    #[test]
    fn test_ln_1234567() {
        assert!(assert_ln(1234567u64));
    }

    #[test]
    fn test_ln_tables() {
        let lns = vec![
            "0".to_string(),
            "478688709125778178049".to_string(),
            "263441406898681741314".to_string(),
            "139053664958586388484".to_string(),
            "71572920525644939266".to_string(),
            "36328762377545711618".to_string(),
            "18304112710400286721".to_string(),
            "9187529797136163839".to_string(),
            "4602702206915280897".to_string(),
            "2303594137142748674".to_string(),
            "1152358920889075706".to_string(),
            "576320060611281987".to_string(),
            "288195197505197603".to_string(),
            "144106392698596015".to_string(),
            "72055395104146779".to_string(),
            "36028247274334636".to_string(),
            "18014261071926600".to_string(),
            "9007164895177383".to_string(),
            "4503591037457749".to_string(),
            "2251797666204331".to_string(),
            "1125899369972055".to_string(),
            "562949819203622".to_string(),
            "281474943156232".to_string(),
            "140737479966715".to_string(),
            "70368742080507".to_string(),
            "35184371564548".to_string(),
            "17592185913349".to_string(),
            "8796092989442".to_string(),
            "4398046502908".to_string(),
            "2199023253508".to_string(),
            "1099511627261".to_string(),
        ];

        let ln_table: Vec<String> = LN_TABLE.iter().map(|i| format!("{}", i.0)).collect();
        assert_eq!(lns, ln_table);
    }

    #[test]
    fn test_random_ln() {
        use std::time::Instant;
        let mut rng = thread_rng();

        let t = Instant::now();
        for _ in 0..200000 {
            loop {
                let v: u64 = rng.gen();
                if v != 0 {
                    //                    FixedPoint::ln(&v.into());
                    assert!(assert_ln(v));
                    break;
                }
            }
        }
        println!("use:{:?}", t.elapsed());
    }

    #[test]
    fn test_collisions() {
        use std::collections::HashSet;
        let mut lns = HashSet::new();
        let base = BigUint::parse_bytes(
            b"1111111111111111111111111111111111111111111111111111111111111111111111",
            16,
        )
        .unwrap();
        lns.insert(FixedPoint::ln(&base).0);
        for i in 1u64..(1 << 20) {
            let ln = FixedPoint::ln(&(base.clone() - i));
            if !lns.contains(&ln.0) {
                println!("{} is the same ln result as 2^256", i - 1);
                break;
            }
        }
    }
}
