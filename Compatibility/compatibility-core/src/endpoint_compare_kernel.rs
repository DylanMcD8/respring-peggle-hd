/* This Source Code Form is subject to the Mozilla Public License, v. 2.0. */
//! IEEE binary32 comparison by integer bits, preserving the current guest
//! interpreter's VCMP(E) behavior. No host floating-point operations or sort.
//! The interpreter currently preserves cumulative FP exception flags even for
//! signalling NaNs; the original-function runtime corpus checks this contract.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct Outcome {pub less:u32,pub cpsr:u32,pub fpscr:u32}
pub const EDGES:[u32;20]=[0,0x80000000,0x3f800000,0xbf800000,0x40000000,0xc0000000,0x7f7fffff,0xff7fffff,0x7f800000,0xff800000,0x7fc01234,0xffc05678,0x7f801234,0xff805678,1,0x80000001,0x007fffff,0x807fffff,0x00800000,0x80800000];
pub const CORPUS_CASES:u32=512;
/// No vector/rounding/FZ/DN/exception-enable or unknown FP controls, and no
/// active Thumb IT state. Status/condition/Q bits are preserved separately.
pub fn supported(cpsr:u32,fpscr:u32)->bool {
    cpsr&0x0600fc00==0 && fpscr&!0xf800009f==0
}
#[inline]pub fn compare(left:u32,right:u32,cpsr:u32,fpscr:u32)->Option<Outcome>{
    if !supported(cpsr,fpscr){return None}
    let a=left&0x7fffffff;let b=right&0x7fffffff;
    let flags=if a>0x7f800000 || b>0x7f800000 {0x30000000}
        else if left==right || (a==0 && b==0) {0x60000000}
        else {
            let less=if (left^right)&0x80000000!=0 {left>>31!=0}
                else if left>>31!=0 {left>right}else{left<right};
            if less{0x80000000}else{0x20000000}
        };
    Some(Outcome{less:(flags>>31)&1,cpsr:(cpsr&0x0fffffff)|flags,fpscr:(fpscr&0x0fffffff)|flags})
}
/// Four-byte input span must be aligned, nonwrapping and disjoint from spills.
pub fn readable_span(address:u32,sp:u32)->bool{
    address&3==0 && sp&3==0 && sp>=8 && address.checked_add(4).is_some()
        && (address+4<=sp-8 || address>=sp)
}
pub fn stack_words(left_endpoint:u32,right_endpoint:u32)->[u8;8]{
    let mut a=[0;8];a[..4].copy_from_slice(&right_endpoint.to_le_bytes());a[4..].copy_from_slice(&left_endpoint.to_le_bytes());a
}
pub fn corpus(i:u32)->(u32,u32){
    if i<400 {(EDGES[(i/20)as usize],EDGES[(i%20)as usize])}
    else {let x=i.wrapping_mul(1664525).wrapping_add(1013904223);(x,x.rotate_left(13).wrapping_mul(0x9e3779b9))}
}
#[cfg(test)]mod tests{
    use super::*;
    #[test]fn all_edges_and_payloads_match_independent_numeric_order(){for a in EDGES{for b in EDGES{let x=f32::from_bits(a);let y=f32::from_bits(b);let flags=match x.partial_cmp(&y){Some(std::cmp::Ordering::Less)=>0x80000000,Some(std::cmp::Ordering::Equal)=>0x60000000,Some(std::cmp::Ordering::Greater)=>0x20000000,None=>0x30000000};let p=compare(a,b,0x88000010,0xf800009f).unwrap();assert_eq!(p.less,u32::from(x<y));assert_eq!(p.cpsr,0x08000010|flags);assert_eq!(p.fpscr,0x0800009f|flags);}}}
    #[test]fn signed_zero_equal_and_nan_unordered(){for a in [0,0x80000000]{for b in [0,0x80000000]{assert_eq!(compare(a,b,0,0).unwrap().fpscr,0x60000000);}}for a in [0x7fc00000,0x7f800001,0xff800001]{assert_eq!(compare(a,0,0,0).unwrap().fpscr,0x30000000);assert_eq!(compare(0,a,0,0).unwrap().less,0);}}
    #[test]fn subnormals_are_not_flushed(){assert_eq!(compare(1,0,0,0).unwrap().fpscr,0x20000000);assert_eq!(compare(0x80000001,0,0,0).unwrap().fpscr,0x80000000);assert_eq!(compare(0x007fffff,0x00800000,0,0).unwrap().less,1);}
    #[test]fn unsupported_fp_controls_and_it_state_fall_back(){for bit in 0..32{if (1u32<<bit)&0xf800009f==0{assert_eq!(compare(0,0,0,1u32<<bit),None);}}for bit in [10,11,12,13,14,15,25,26]{assert!(!supported(1<<bit,0));}}
    #[test]fn address_guard_and_spill_layout(){for a in [0x1ff8,0x1ffc,0xfffffffc,0x2001]{assert!(!readable_span(a,0x2000));}assert!(readable_span(0x1ff4,0x2000));assert!(readable_span(0x2000,0x2000));assert_eq!(stack_words(0x12345678,0xabcdef01),[1,239,205,171,120,86,52,18]);}
}
