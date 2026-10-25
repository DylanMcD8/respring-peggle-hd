/* This Source Code Form is subject to the Mozilla Public License, v. 2.0. */
//! Conservative sequential f32 kernel. No FMA, reassociation, or host pointers.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Product { pub values: [f32; 9], pub last_prefix: f32 }
fn ordinary(v:f32)->bool {v==0.0 || v.is_normal()}
pub(crate) fn eligible_addresses(out:u32,a:u32,b:u32)->bool {
    [out,a,b].iter().all(|p|*p>=4096 && p&3==0 && p.checked_add(36).is_some())
        && !(out<a.saturating_add(36) && a<out.saturating_add(36))
        && !(out<b.saturating_add(36) && b<out.saturating_add(36))
}
pub(crate) fn multiply(a:&[f32;9],b:&[f32;9],fpscr:u32)->Option<Product> {
    // Only default rounding/control. Preserve existing condition/sticky bits.
    if fpscr & !0xf000_009f != 0 || !a.iter().chain(b.iter()).all(|v|ordinary(*v)){return None}
    let mut result=Product{values:[0.0;9],last_prefix:0.0};
    for row in 0..3 {for col in 0..3 {
        let mut sum=0.0f32;
        for k in 0..3 {
            let x=a[row*3+k];let y=b[k*3+col];
            let product=x*y; // Deliberate separate f32 operations, never mul_add.
            if !ordinary(product) || (x!=0.0 && y!=0.0 && product==0.0){return None}
            if row==2 && col==2 && k==2 {result.last_prefix=sum}
            sum=sum+product;
            if !ordinary(sum){return None}
        }
        result.values[row*3+col]=sum;
    }}Some(result)
}
/// Deterministic numeric-only corpus shared by host tests and runtime original-code gate.
pub(crate) fn corpus(i:u32)->([f32;9],[f32;9]) {
    let mut a=[0.0;9];let mut b=[0.0;9];
    if i==0 {for j in 0..3{a[j*3+j]=1.0;b[j*3+j]=1.0}return(a,b)}
    if i==1 {return([-0.0;9],[1.0;9])}
    if i==2 {return([16777216.,1.,-16777216.,-0.,0.,-0.,1.,-1.,1.],[1.;9])}
    if i==3 {return([-1.,1.+f32::EPSILON,0.,0.,1.,0.,0.,0.,1.],[1.,1.,1.,1.-f32::EPSILON,1.-f32::EPSILON,1.-f32::EPSILON,1.,1.,1.])}
    let mut seed=i.wrapping_mul(0x9e3779b9);
    for v in a.iter_mut().chain(b.iter_mut()) {seed=seed.wrapping_mul(1664525).wrapping_add(1013904223);*v=((seed>>16) as i16 as f32)/256.0;}
    (a,b)
}
#[cfg(test)] mod tests {
 use super::*;
 #[test]fn identity_and_signed_zero(){let(a,b)=corpus(0);assert_eq!(multiply(&a,&b,0).unwrap().values,a);let(a,b)=corpus(1);assert!(multiply(&a,&b,0).unwrap().values.iter().all(|v|v.to_bits()==0));}
 #[test]fn controls_and_exceptional_fallback(){for mode in [1<<8,1<<16,1<<22,1<<24,1<<25]{assert!(multiply(&[1.;9],&[1.;9],mode).is_none())}for v in [f32::NAN,f32::INFINITY,f32::NEG_INFINITY,f32::from_bits(1)]{assert!(multiply(&[v;9],&[1.;9],0).is_none())}assert!(multiply(&[f32::MAX;9],&[2.;9],0).is_none());assert!(multiply(&[f32::MIN_POSITIVE;9],&[0.125;9],0).is_none());}
 #[test]fn alias_gate(){assert!(eligible_addresses(0x1000,0x2000,0x2000));for o in [0,0x2000,0x2004,0x1fe0,0xfffffff0,0x1001]{assert!(!eligible_addresses(o,0x2000,0x3000))}}
 #[test]fn order_and_sticky_state(){let a=[16777216.,1.,-16777216.,0.,0.,0.,0.,0.,0.];let b=[1.;9];assert_eq!(multiply(&a,&b,0xf000009f).unwrap().values[0].to_bits(),0.0f32.to_bits());}
 #[test]fn no_fused_multiply_add(){let(a,b)=corpus(3);assert_eq!(multiply(&a,&b,0).unwrap().values[0].to_bits(),0);assert_ne!(a[1].mul_add(b[3],-1.0).to_bits(),0)}
 #[test]fn corpus_eligible(){for i in 0..10000{let(a,b)=corpus(i);assert!(multiply(&a,&b,0).is_some(),"{i}")}}
}
