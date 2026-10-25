/* This Source Code Form is subject to the Mozilla Public License, v. 2.0. */
//! Exact wrapping arithmetic for the audited HD EndPoint vector accessor.
//! This only computes an address; it never reads/writes an endpoint or sorts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Outcome { pub address:u32, pub scaled:u32, pub cpsr:u32 }
#[inline]
pub fn calculate(base:u32,index:u32,cpsr:u32)->Outcome {
    let scaled=index.wrapping_mul(12);
    // Original MOVS/MULS update N/Z and preserve C/V and control bits.
    let nz=(scaled&0x8000_0000)|if scaled==0{0x4000_0000}else{0};
    Outcome{address:base.wrapping_add(scaled),scaled,cpsr:(cpsr&0x3fff_ffff)|nz}
}
/// Original spills two words below SP before reading the vector's base field.
/// Reject aliases and unaligned/wrapping ranges; original fallback handles them.
pub fn eligible(vector:u32,sp:u32)->bool {
    vector&3==0 && sp&3==0 && sp>=8 && vector.checked_add(4).is_some()
        && (vector+4<=sp-8 || vector>=sp)
}
pub fn stack_words(vector:u32,index:u32)->[u8;8] {
    let mut a=[0;8];a[..4].copy_from_slice(&index.to_le_bytes());
    a[4..].copy_from_slice(&vector.to_le_bytes());a
}
#[cfg(test)]mod tests {
    use super::*;
    #[test]fn twelve_byte_stride_and_wrap(){assert_eq!(calculate(0x1000,3,0).address,0x1024);assert_eq!(calculate(0xfffffff8,1,0).address,4);assert_eq!(calculate(1,0xffffffff,0).scaled,0xfffffff4);}
    #[test]fn flags_preserve_cv_control(){for cv in [0,0x10000000,0x20000000,0x30000000]{for i in [0,1,0x20000000,0x40000000,0xffffffff]{let o=calculate(7,i,0xc0000030|cv);assert_eq!(o.cpsr&0x3fffffff,cv|0x30);assert_eq!(o.cpsr>>30,((o.scaled>>31)<<1)|u32::from(o.scaled==0));}}}
    #[test]fn exact_spill_order(){assert_eq!(stack_words(0x12345678,0xabcdef01),[1,239,205,171,120,86,52,18]);}
    #[test]fn range_and_alias_rejections(){for(v,s)in[(0x1001,0x2000),(0x1000,0x2001),(0x1000,4),(0xfffffffc,0x2000),(0x1ff8,0x2000),(0x1ffc,0x2000)]{assert!(!eligible(v,s));}assert!(eligible(0x1ff4,0x2000));assert!(eligible(0x2000,0x2000));}
}
