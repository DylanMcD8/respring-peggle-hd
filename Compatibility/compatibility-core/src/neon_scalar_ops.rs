/* This Source Code Form is subject to the Mozilla Public License, v. 2.0. */
//! One shared operation boundary for the existing interpreter and native leaf
//! kernels. Keeping each operation separate forbids FMA/reassociation and makes
//! NaN, subnormal and saturation behavior identical within the same host build.
#[inline(never)]pub(crate)fn mul_f32(a:u32,b:u32)->u32{(f32::from_bits(a)*f32::from_bits(b)).to_bits()}
#[inline(never)]pub(crate)fn sub_f32(a:u32,b:u32)->u32{(f32::from_bits(a)-f32::from_bits(b)).to_bits()}
#[inline(never)]pub(crate)fn signed_from_f32(a:u32)->u32{
 let f=f32::from_bits(a)as f64;
 (if f.is_nan(){0}else if f>=i32::MAX as f64{i32::MAX}else if f<=i32::MIN as f64{i32::MIN}else{f.trunc()as i32})as u32
}
#[cfg(test)]mod tests{use super::*;
 #[test]fn saturation_and_signed_zero_edges(){for b in[0,0x80000000,0x3f800000,0xbf800000,0x4effffff,0x4f000000,0xcf000000,0x7f800000,0xff800000,0x7fc01234,0x7f801234,1,0x80000001]{assert_eq!(signed_from_f32(b),(f32::from_bits(b)as i32)as u32);}assert_eq!(mul_f32(0x80000000,0x41000000),0x80000000);}
}
