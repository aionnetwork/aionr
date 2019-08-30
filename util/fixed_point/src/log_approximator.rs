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
use super::FixedPoint;
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
                FixedPoint::parse_from_big_decimal(BigDecimal::from_str_radix(s, 10).unwrap())
                    .unwrap(),
            );
        }
        ln_table
    };
    static ref LN2: FixedPoint = FixedPoint::parse_from_big_decimal(
        BigDecimal::from_str_radix("0.693147180559945309", 10).unwrap()
    )
    .unwrap();
}

//pub fn init_log_approximator(){
//    for s in HARD_CODE_LNS.iter(){
//        LN_TABLE.push(FixedPoint::parse_from_big_decimal(BigDecimal::from_str_radix(s, 10).unwrap()).unwrap());
//    }
//    LN2 = FixedPoint::parse_from_big_decimal(BigDecimal::from_str_radix("0.693147180559945309", 10).unwrap()).unwrap();
//}

pub fn log(input: BigUint) -> FixedPoint {
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

    let mut left_shift = 1usize;

    while left_shift < (*LN_TABLE).len() && x < FixedPoint::one() {
        let x_prime = x.add(x.divide_by_power_of_two(left_shift));

        // if xPrime is less than or equal to one, this is the best factor we can multiplyInteger by
        if x_prime <= FixedPoint::one() {
            x = x_prime;
            y = y
                .subtruct((*LN_TABLE)[left_shift].clone())
                .expect("FixedPoint sub ln failed");
        }
        // otherwise, try the next factor (eg. 1.01 was too big, so try 1.001)
        else {
            left_shift += 1;
        }
    }

    y = y
        .add(x)
        .subtruct(FixedPoint::one())
        .expect("FixedPoint sub one failed");

    y
}
