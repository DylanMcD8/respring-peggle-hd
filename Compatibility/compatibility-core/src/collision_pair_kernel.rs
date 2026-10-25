/* This Source Code Form is subject to the Mozilla Public License, v. 2.0. */
//! Exact bounded CollisionPair C2 constructor and IsCollidable leaf predictions.
use crate::cpu::CpuContext;
#[derive(Clone,Copy,Debug,PartialEq,Eq)]pub(crate)struct Object{pub address:u32,pub kind:u32,pub id:u32}
pub(crate)struct PairOutcome{pub context:CpuContext,pub stack:[u8;20],pub pair:[u8;8],pub branch:usize}
fn sub(c:&mut CpuContext,a:u32,b:u32){let r=a.wrapping_sub(b);let v=(a^b)&(a^r)&0x80000000!=0;c.cpsr=(c.cpsr&0x0fffffff)|(r&0x80000000)|if r==0{0x40000000}else{0}|if a>=b{0x20000000}else{0}|if v{0x10000000}else{0};}
/// Canonical order follows object type2 and signed ID, not pointer magnitude.
pub(crate)fn key(left:Object,right:Object)->(u32,u32,usize){if left.kind!=2{(right.address,left.address,0)}else if right.kind!=2{(left.address,right.address,1)}else if (left.id as i32)<(right.id as i32){(left.address,right.address,2)}else{(right.address,left.address,3)}}
pub(crate)fn pair(mut c:CpuContext,left:Object,right:Object,prior:&[u8])->Option<PairOutcome>{
 if c.regs[13]<20||c.regs[13]&3!=0||c.cpsr&0x0600fc00!=0||prior.len()!=20||c.regs[1]!=left.address||c.regs[2]!=right.address{return None;}
 let this=c.regs[0];let mut stack:[u8;20]=prior.try_into().unwrap();for(off,v)in [(12,this),(8,left.address),(4,right.address),(16,this),(0,this)]{stack[off..off+4].copy_from_slice(&v.to_le_bytes());}
 c.regs[1]=left.kind;sub(&mut c,left.kind,2);let(a,b,branch)=key(left,right);
 if left.kind==2{c.regs[0]=right.kind;sub(&mut c,right.kind,2);if right.kind==2{c.regs[0]=left.id;c.regs[1]=right.id;sub(&mut c,left.id,right.id);}}
 // All original branches finish with r1=destination and r0 restored to it.
 c.regs[0]=this;c.regs[1]=this;let mut pair=[0u8;8];pair[..4].copy_from_slice(&a.to_le_bytes());pair[4..].copy_from_slice(&b.to_le_bytes());Some(PairOutcome{context:c,stack,pair,branch})
}
pub(crate)fn collidable(mut c:CpuContext,byte:u8,prior:&[u8])->Option<(CpuContext,[u8;4])>{if c.regs[13]<4||c.regs[13]&3!=0||c.cpsr&0x0600fc00!=0||prior.len()!=4{return None;}let stack=c.regs[0].to_le_bytes();c.regs[0]=u32::from(byte)&1;Some((c,stack))}
