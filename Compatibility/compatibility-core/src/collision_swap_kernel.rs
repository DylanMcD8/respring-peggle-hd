/* This Source Code Form is subject to the Mozilla Public License, v. 2.0. */
//! Exact side effects of the audited EndPoint swap; no ordering/pair decisions.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]pub struct Outcome{pub left:[u8;12],pub right:[u8;12],pub stack:[u8;20],pub r0:u32,pub r1:u32,pub r2:u32}
pub fn eligible(a:u32,b:u32,sp:u32,cpsr:u32)->bool{
 if a&3!=0||b&3!=0||sp&3!=0||sp<20||cpsr&0x0600fc00!=0{return false;}
 let Some(ae)=a.checked_add(12)else{return false;};let Some(be)=b.checked_add(12)else{return false;};
 (a==b||ae<=b||be<=a)&&(ae<=sp-20||a>=sp)&&(be<=sp-20||b>=sp)
}
pub fn calculate(a:u32,b:u32,left:[u8;12],right:[u8;12])->Outcome{
 let mut l=left;let mut r=right;l[..9].copy_from_slice(&right[..9]);r[..9].copy_from_slice(&left[..9]);
 let mut stack=[0;20];stack[..12].copy_from_slice(&left);stack[12..16].copy_from_slice(&b.to_le_bytes());stack[16..20].copy_from_slice(&a.to_le_bytes());
 Outcome{left:l,right:r,stack,r0:b,r1:u32::from(left[8]),r2:u32::from_le_bytes(right[4..8].try_into().unwrap())}
}
#[cfg(test)]mod tests{use super::*;
 #[test]fn padding_and_dead_stack_exact(){let a=[1u8;12];let b=[2u8;12];let v=calculate(0x1000,0x2000,a,b);assert_eq!(&v.left[..9],&[2;9]);assert_eq!(&v.left[9..],&[1;3]);assert_eq!(&v.right[..9],&[1;9]);assert_eq!(&v.right[9..],&[2;3]);assert_eq!(&v.stack[..12],&a);assert_eq!(v.r0,0x2000);assert_eq!(v.r1,1);assert_eq!(v.r2,0x02020202);}
 #[test]fn same_object_and_alias_rejections(){assert!(eligible(0x1000,0x1000,0x3000,0x10));for(a,b,s,c)in[(0x1000,0x1004,0x3000,0x10),(0x1001,0x2000,0x3000,0x10),(0xfffffffc,0x2000,0x3000,0x10),(0x2fec,0x1000,0x3000,0x10),(0x1000,0x2000,8,0x10),(0x1000,0x2000,0x3000,0x810)]{assert!(!eligible(a,b,s,c));}}
}
