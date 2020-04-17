/*******************************************************************************
 * Copyright (c) 2018-2020 Aion foundation.
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
use curve25519::{GeP3, GeP1P1, ge25519_from_uniform, ge_scalarmult, sc_reduce, ge_scalarmult_base, sc_muladd};
use sha2::Sha512;
use digest::Digest;
use rustc_hex::ToHex;

static SUITE: [u8; 1] = [0x04];
static ONE: [u8; 1] = [0x01];
static TWO: [u8; 1] = [0x02];
static THREE: [u8; 1] = [0x03];


fn _vrf_expand_sk(sk: &[u8; 64]) -> ([u8; 32], [u8; 32], GeP3) {
	let seed = &sk[0..32];
    let pk = &sk[32..64];
    let hash: [u8; 64] = {
        let mut hash_output: [u8; 64] = [0; 64];
        let mut hasher = Sha512::new();
        hasher.input(seed);
        hasher.result(&mut hash_output);
        hash_output[0] &= 248;
        hash_output[31] &= 63;
        hash_output[31] |= 64;
        hash_output
    };

    // Calculate x_scalar and truncated_hashed_sk_string
    let mut x_scalar: [u8; 32] = [0; 32];
    let mut truncated_hashed_sk_string: [u8; 32] = [0; 32];
    for (dest, src) in (&mut x_scalar).iter_mut().zip(hash[0..32].iter()) {
        *dest = *src;
    }
    for (dest, src) in (&mut truncated_hashed_sk_string).iter_mut().zip(hash[32..64].iter()) {
        *dest = *src;
    }

    println!("x_scalar: {:?}", x_scalar.to_hex());
    println!("truncated_hashed_sk_string: {:?}", truncated_hashed_sk_string.to_hex());

    (x_scalar, truncated_hashed_sk_string, GeP3::from_bytes(pk).unwrap()) // TODO: handle unwrap
}

pub fn vrf_prove(y_point: &GeP3, x_scalar: &[u8; 32], truncated_hashed_sk_string: &[u8; 32], message: &[u8]) -> [u8; 80] {
    let h_string: [u8; 32] = vrf_ietfdraft03_hash_to_curve_elligator2_25519(&y_point, message);
    let h_point: GeP3 = GeP3::from_bytes(&h_string).unwrap(); // TODO: handle unwrap

    let gamma_point: GeP3 = ge_scalarmult(x_scalar, h_point);
    let k_scalar: [u8; 32] = vrf_nonce_generation(truncated_hashed_sk_string, &h_string);
    let kb_point: GeP3 = ge_scalarmult_base(&k_scalar);
    let kh_point: GeP3 = ge_scalarmult(&k_scalar, h_point);

    println!("k_scalar: {:?}", k_scalar.to_hex());
    println!("gamma_point: {:?}", gamma_point.to_bytes().to_hex());
    println!("kb_point: {:?}", kb_point.to_bytes().to_hex());
    println!("kh_point: {:?}", kh_point.to_bytes().to_hex());

    /* c = ECVRF_hash_points(h, gamma, k*B, k*H) 
     * (writes only to the first 16 bytes of c_scalar */
    let c_scalar: [u8; 32] = vrf_ietfdraft03_hash_points(&h_point, &gamma_point, &kb_point, &kh_point);
    println!("c_scalar: {:?}", c_scalar.to_hex());

    let mut pi: [u8; 80] = [0; 80];
    for (dest, src) in (&mut pi[0..32]).iter_mut().zip(gamma_point.to_bytes()[0..32].iter()) {
        *dest = *src;
    }
    for (dest, src) in (&mut pi[32..48]).iter_mut().zip(c_scalar[0..16].iter()) {
        *dest = *src;
    }
    let mut pi_48_80: [u8; 32] = [0; 32];
    sc_muladd(&mut pi_48_80, &c_scalar, x_scalar, &k_scalar);
    for (dest, src) in (&mut pi[48..80]).iter_mut().zip(pi_48_80[0..32].iter()) {
        *dest = *src;
    }
    println!("pi: {:?}", pi.to_hex());

    pi
}

pub fn vrf_verify(pk: &[u8; 32], proof: &[u8; 80], message: &[u8]) -> Result<[u8; 64], ()> {
    let y_point: GeP3 = GeP3::from_bytes(pk).unwrap(); // TODO: handle unwrap
    let (gamma_point, c_scalar, mut s_scalar) = vrf_ietfdraft03_decode_proof(proof);

    sc_reduce(&mut s_scalar);

    let h_string: [u8; 32] = vrf_ietfdraft03_hash_to_curve_elligator2_25519(&y_point, message);
    let h_point: GeP3 = GeP3::from_bytes(&h_string).unwrap(); // TODO: handle unwrap

    /* calculate U = s*B - c*Y */
    let mut tmp_p3_point: GeP3 = ge_scalarmult(&c_scalar, y_point);
    let mut tmp_cached_point = tmp_p3_point.to_cached();
    tmp_p3_point = ge_scalarmult_base(&s_scalar);
    let mut tmp_p1p1_point: GeP1P1 = tmp_p3_point - tmp_cached_point;
    let u_point: GeP3 = tmp_p1p1_point.to_p3();

    /* calculate V = s*H -  c*Gamma */
    tmp_p3_point = ge_scalarmult(&c_scalar, gamma_point);
    tmp_cached_point = tmp_p3_point.to_cached();
    tmp_p3_point = ge_scalarmult(&s_scalar, h_point);
    tmp_p1p1_point = tmp_p3_point - tmp_cached_point;
    let v_point: GeP3 = tmp_p1p1_point.to_p3();

    let cprime: [u8; 32] = vrf_ietfdraft03_hash_points(&h_point, &gamma_point, &u_point, &v_point);

    if cprime == c_scalar {
        Ok(vrf_ietfdraft03_proof_to_hash(proof))
    } else {
        Err(())
    }
}

fn vrf_ietfdraft03_hash_to_curve_elligator2_25519(y_point: &GeP3, message: &[u8]) -> [u8; 32] {
	let y_bytes: [u8; 32] = y_point.to_bytes();
    println!("y_bytes {:?}", y_bytes.to_hex());

	let hash: [u8; 64] = {
        let mut hash_output: [u8; 64] = [0; 64];
        let mut hasher = Sha512::new();
        hasher.input(&SUITE);
        hasher.input(&ONE);
        hasher.input(&y_bytes);
        hasher.input(message);
        hasher.result(&mut hash_output);
        hash_output[31] &= 127;
        hash_output
    };

    println!("r_string: {:?}", hash.to_hex());

    let mut r_string: [u8; 32] = [0; 32];
    for (dest, src) in (&mut r_string).iter_mut().zip(hash[0..32].iter()) {
        *dest = *src;
    }

    let result: [u8; 32] = ge25519_from_uniform(r_string);

    println!("H_string: {:?}", result.to_hex());
    result
}

fn vrf_nonce_generation(truncated_hashed_sk_string: &[u8; 32], h_string: &[u8; 32]) -> [u8; 32] {
    let mut hash: [u8; 64] = {
        let mut hash_output: [u8; 64] = [0; 64];
        let mut hasher = Sha512::new();
        hasher.input(truncated_hashed_sk_string);
        hasher.input(h_string);
        hasher.result(&mut hash_output);
        hash_output
    };

    sc_reduce(&mut hash);

    let mut k_scalar: [u8; 32] = [0; 32];
    for (dest, src) in (&mut k_scalar).iter_mut().zip(hash[0..32].iter()) {
        *dest = *src;
    }

    k_scalar
}

fn vrf_ietfdraft03_hash_points(p1: &GeP3, p2: &GeP3, p3: &GeP3, p4: &GeP3) -> [u8; 32] {
    let p1_bytes: [u8; 32] = p1.to_bytes();
    let p2_bytes: [u8; 32] = p2.to_bytes();
    let p3_bytes: [u8; 32] = p3.to_bytes();
    let p4_bytes: [u8; 32] = p4.to_bytes();

    let hash: [u8; 64] = {
        let mut hash_output: [u8; 64] = [0; 64];
        let mut hasher = Sha512::new();
        hasher.input(&SUITE);
        hasher.input(&TWO);
        hasher.input(&p1_bytes);
        hasher.input(&p2_bytes);
        hasher.input(&p3_bytes);
        hasher.input(&p4_bytes);
        hasher.result(&mut hash_output);
        hash_output
    };

    let mut c: [u8; 32] = [0; 32];
    for (dest, src) in (&mut c).iter_mut().zip(hash[0..16].iter()) {
        *dest = *src;
    }

    c
}

fn vrf_ietfdraft03_decode_proof(proof: &[u8; 80]) -> (GeP3, [u8; 32], [u8; 64]) {
    let gamma_point: GeP3 = GeP3::from_bytes(&proof[0..32]).unwrap(); // TODO: handle unwrap
    let mut c_scalar: [u8; 32] = [0; 32];
    let mut s_scalar: [u8; 64] = [0; 64];
    for (dest, src) in (&mut c_scalar[0..16]).iter_mut().zip(proof[32..48].iter()) {
        *dest = *src;
    }
    for (dest, src) in (&mut s_scalar[0..32]).iter_mut().zip(proof[48..80].iter()) {
        *dest = *src;
    }
    (gamma_point, c_scalar, s_scalar)
}

fn vrf_ietfdraft03_proof_to_hash(proof: &[u8; 80]) -> [u8;64] {
    let (mut gamma_point, _, _) = vrf_ietfdraft03_decode_proof(proof);
    gamma_point = gamma_point.multiply_by_cofactor();
    let gamma_bytes: [u8; 32] = gamma_point.to_bytes();

    let hash: [u8; 64] = {
        let mut hash_output: [u8; 64] = [0; 64];
        let mut hasher = Sha512::new();
        hasher.input(&SUITE);
        hasher.input(&THREE);
        hasher.input(&gamma_bytes);
        hasher.result(&mut hash_output);
        hash_output
    };

    hash
}


#[cfg(test)]
mod tests {
    use curve25519::{GeP3};
	use ed25519::{keypair};
    use vrf::{_vrf_expand_sk, vrf_prove, vrf_verify};
    use rustc_hex::ToHex;

	#[test]
	fn generate_keypair() {
		let seed: [u8; 32] = [0x9d,0x61,0xb1,0x9d,0xef,0xfd,0x5a,0x60,0xba,0x84,0x4a,0xf4,0x92,0xec,0x2c,0xc4,0x44,0x49,0xc5,0x69,0x7b,0x32,0x69,0x19,0x70,0x3b,0xac,0x03,0x1c,0xae,0x7f,0x60];
		let (_sk, pk): ([u8; 64], [u8; 32]) = keypair(seed.as_ref());
		assert_eq!(pk, [0xd7,0x5a,0x98,0x01,0x82,0xb1,0x0a,0xb7,0xd5,0x4b,0xfe,0xd3,0xc9,0x64,0x07,0x3a,0x0e,0xe1,0x72,0xf3,0xda,0xa6,0x23,0x25,0xaf,0x02,0x1a,0x68,0xf7,0x07,0x51,0x1a]);
	}

	#[test]
	fn generate_proof() {
		let seed: [u8; 32] = [0xc5,0xaa,0x8d,0xf4,0x3f,0x9f,0x83,0x7b,0xed,0xb7,0x44,0x2f,0x31,0xdc,0xb7,0xb1,0x66,0xd3,0x85,0x35,0x07,0x6f,0x09,0x4b,0x85,0xce,0x3a,0x2e,0x0b,0x44,0x58,0xf7];
		let expected_pk: [u8; 32] = [0xfc,0x51,0xcd,0x8e,0x62,0x18,0xa1,0xa3,0x8d,0xa4,0x7e,0xd0,0x02,0x30,0xf0,0x58,0x08,0x16,0xed,0x13,0xba,0x33,0x03,0xac,0x5d,0xeb,0x91,0x15,0x48,0x90,0x80,0x25];
        let expected_proof: [u8; 80] = [0xdf,0xa2,0xcb,0xa3,0x4b,0x61,0x1c,0xc8,0xc8,0x33,0xa6,0xea,0x83,0xb8,0xeb,0x1b,0xb5,0xe2,0xef,0x2d,0xd1,0xb0,0xc4,0x81,0xbc,0x42,0xff,0x36,0xae,0x78,0x47,0xf6,0xab,0x52,0xb9,0x76,0xcf,0xd5,0xde,0xf1,0x72,0xfa,0x41,0x2d,0xef,0xde,0x27,0x0c,0x8b,0x8b,0xdf,0xba,0xae,0x1c,0x7e,0xce,0x17,0xd9,0x83,0x3b,0x1b,0xcf,0x31,0x06,0x4f,0xff,0x78,0xef,0x49,0x3f,0x82,0x00,0x55,0xb5,0x61,0xec,0xe4,0x5e,0x10,0x09];
        let expected_output: [u8; 64] = [0x20,0x31,0x83,0x7f,0x58,0x2c,0xd1,0x7a,0x9a,0xf9,0xe0,0xc7,0xef,0x5a,0x65,0x40,0xe3,0x45,0x3e,0xd8,0x94,0xb6,0x2c,0x29,0x36,0x86,0xca,0x3c,0x1e,0x31,0x9d,0xde,0x9d,0x0a,0xa4,0x89,0xa4,0xb5,0x9a,0x95,0x94,0xfc,0x23,0x28,0xbc,0x3d,0xef,0xf3,0xc8,0xa0,0x92,0x9a,0x36,0x9a,0x72,0xb1,0x18,0x0a,0x59,0x6e,0x01,0x6b,0x5d,0xed];

        let (sk, pk): ([u8; 64], [u8; 32]) = keypair(seed.as_ref());
		assert_eq!(pk, expected_pk);
        let (x_scalar, truncated_hashed_sk_string, y_point): ([u8; 32], [u8; 32], GeP3) = _vrf_expand_sk(&sk);
        let message: &[u8] = &[0xaf, 0x82];
        let mut proof: [u8; 80] = vrf_prove(&y_point, &x_scalar, &truncated_hashed_sk_string, message);
        assert_eq!(proof.to_hex(), expected_proof.to_hex());
        let verify_result = vrf_verify(&expected_pk, &proof, message);
        assert!(verify_result.is_ok());
        assert_eq!(verify_result.unwrap().to_hex(), expected_output.to_hex());

        proof[0] ^= 0x01;
        assert!(vrf_verify(&expected_pk, &proof, message).is_err());

        proof[0] ^= 0x01;
        proof[32] ^= 0x01;
        assert!(vrf_verify(&expected_pk, &proof, message).is_err());

        proof[32] ^= 0x01;
        proof[48] ^= 0x01;
        assert!(vrf_verify(&expected_pk, &proof, message).is_err());

        proof[48] ^= 0x01;
        proof[79] ^= 0x80;
        assert!(vrf_verify(&expected_pk, &proof, message).is_err());

        proof[79] ^= 0x80;
        assert!(vrf_verify(&expected_pk, &proof, message).is_ok());
	}

}