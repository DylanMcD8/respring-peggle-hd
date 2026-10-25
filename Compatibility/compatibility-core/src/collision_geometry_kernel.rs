/* This Source Code Form is subject to the Mozilla Public License, v. 2.0. */
//! Exact private predictions for two audited collision rectangle leaves.
//! Original-function full-state gates remain mandatory before native activation.
use crate::{cpu::CpuContext,endpoint_compare_kernel as fp};
#[derive(Clone,Copy,Debug)]pub enum Kind{Objects,Values}
impl Kind{pub fn stack_len(self)->usize{match self{Self::Objects=>16,Self::Values=>28}}}
pub struct Outcome{pub context:CpuContext,pub stack:Vec<u8>,pub last_comparison:usize}
fn nz(c:&mut CpuContext,v:u32){c.cpsr=(c.cpsr&0x3fffffff)|(v&0x80000000)|if v==0{0x40000000}else{0};}
fn store(stack:&mut[u8],offset:usize,v:u32){stack[offset..offset+4].copy_from_slice(&v.to_le_bytes());}
fn compare(c:&mut CpuContext,a:u32,b:u32)->Option<()>{let p=fp::compare(a,b,c.cpsr,c.fpscr)?;c.cpsr=p.cpsr;c.fpscr=p.fpscr;Some(())}
fn hi(c:&CpuContext)->bool{c.cpsr&0x20000000!=0&&c.cpsr&0x40000000==0}
fn lt(c:&CpuContext)->bool{(c.cpsr>>31)&1!=(c.cpsr>>28)&1}
/// left/right order is minX,minY,maxX,maxY. For Values, right is ignored;
/// exact incoming r1/r2/r3 plus the fifth argument form the query rectangle.
pub fn calculate(mut c:CpuContext,kind:Kind,left:[u32;4],right:[u32;4],fifth:u32,prior:&[u8])->Option<Outcome>{
 let length=kind.stack_len();if c.regs[13]<length as u32||c.regs[13]&3!=0||!fp::supported(c.cpsr,c.fpscr)||prior.len()!=length{return None;}
 let this=c.regs[0];let other=c.regs[1];let mut stack=prior.to_vec();
 let rect=match kind{
  Kind::Objects=>{c.regs[2]=0;nz(&mut c,0);store(&mut stack,12,this);store(&mut stack,8,other);right},
  Kind::Values=>{
   c.extregs[0]=c.regs[3];c.extregs[1]=c.regs[3];c.extregs[4]=c.extregs[0];
   c.extregs[0]=c.regs[2];c.extregs[1]=c.regs[2];c.extregs[8]=c.extregs[0];
   c.extregs[0]=c.regs[1];c.extregs[1]=c.regs[1];c.extregs[12]=c.extregs[0];c.extregs[2]=fifth;
   let r=[c.regs[1],c.regs[2],c.regs[3],fifth];c.regs[1]=0;nz(&mut c,0);
   for(off,v)in [(24,this),(20,r[0]),(16,r[1]),(12,r[2]),(8,r[3])]{store(&mut stack,off,v);}r
  }
 };
 c.regs[0]=this;
 match kind{Kind::Objects=>{c.extregs[0]=left[0];c.regs[1]=other;c.extregs[4]=rect[2];},Kind::Values=>{c.extregs[4]=left[0];c.extregs[8]=rect[2];}}
 compare(&mut c,left[0],rect[2])?;store(&mut stack,4,this);store(&mut stack,0,0);
 let mut last=0;
 if !hi(&c){
  c.regs[0]=0;nz(&mut c,0);c.regs[1]=this;c.extregs[0]=left[2];
  if matches!(kind,Kind::Objects){c.regs[2]=other;}
  c.extregs[4]=rect[0];compare(&mut c,left[2],rect[0])?;store(&mut stack,0,0);last=1;
  if !lt(&c){
   c.regs[0]=0;nz(&mut c,0);c.regs[1]=this;c.extregs[0]=left[1];
   if matches!(kind,Kind::Objects){c.regs[2]=other;}
   c.extregs[4]=rect[3];compare(&mut c,left[1],rect[3])?;store(&mut stack,0,0);last=2;
   if !hi(&c){
    c.regs[0]=this;c.extregs[0]=left[3];
    if matches!(kind,Kind::Objects){c.regs[1]=other;}
    c.extregs[4]=rect[1];compare(&mut c,left[3],rect[1])?;
    c.regs[1]=u32::from(!lt(&c));store(&mut stack,0,c.regs[1]);last=3;
   }
  }
 }
 c.regs[0]=u32::from_le_bytes(stack[..4].try_into().unwrap())&1;
 Some(Outcome{context:c,stack,last_comparison:last})
}
