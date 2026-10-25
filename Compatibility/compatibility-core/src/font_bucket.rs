/* This Source Code Form is subject to the Mozilla Public License, v. 2.0. */
//! Bounded ImageFont empty-bucket traversal. No retained font/image state.
//! Stops before the first actual draw; preserves scalar state and instruction quota.
use crate::mem::{Mem,ConstPtr,MutPtr};
pub(crate)const ENTRY:u32=0x2466e6;
const HEADS:u32=0x3c73e8;
#[derive(Default)]pub(crate)struct State{pub enabled:bool,pub disabled:bool,pub calls:u64,pub shadows:[u64;2],pub fast:u64,#[cfg(test)]pub fault:bool}
impl State{
 pub fn shadow(&self,class:usize)->bool{self.shadows[class]<64||(self.calls+1)%4096==0}
 pub fn checked(&mut self,class:usize,ok:bool){self.calls+=1;if !ok{self.disabled=true;event("disabled_after_mismatch",self.calls);}else{self.shadows[class]+=1;if self.shadows[class]==64{event(if class==0{"nonempty_gate_passed"}else{"end_gate_passed"},64);}}}
 pub fn accelerated(&mut self){self.calls+=1;self.fast+=1;if self.fast==1||self.fast%65536==0{event("fast_calls",self.fast);}}
}
fn event(kind:&str,count:u64){crate::diagnostics::event(&format!("{{\"event\":\"font_bucket_native\",\"kind\":\"{kind}\",\"count\":{count}}}"));}
#[derive(Clone,Copy,Debug)]pub(crate)struct Outcome{pub regs:[u32;16],pub cpsr:u32,pub retired:u64,pub class:usize,stack:u32,index:u32,head:u32,write_index:bool,write_head:bool}
fn word(mem:&Mem,a:u32)->Option<u32>{Some(u32::from_le_bytes(mem.get_bytes_fallible(ConstPtr::<u8>::from_bits(a).cast(),4)?.try_into().ok()?))}
impl Outcome{
 pub fn matches(&self,mem:&Mem)->bool{(!self.write_index||word(mem,self.stack+0x1f0)==Some(self.index))&&(!self.write_head||word(mem,self.stack+0xfc)==Some(self.head))}
 pub fn commit(&self,mem:&mut Mem){if self.write_index{mem.write(MutPtr::<u32>::from_bits(self.stack+0x1f0),self.index);}if self.write_head{mem.write(MutPtr::<u32>::from_bits(self.stack+0xfc),self.head);}}
}
pub(crate)fn predict(mem:&Mem,mut regs:[u32;16],cpsr:u32,budget:u64)->Option<Outcome>{
 if regs[15]!=ENTRY||cpsr&0x20==0||cpsr&0x0600fc00!=0||!code_matches(mem){return None;}
 let sp=regs[13];let end=sp.checked_add(0x1f4)?;
 if sp&3!=0||!mem.is_mapped_range(sp+0xfc,0xf8)||!mem.is_mapped_range(HEADS,1024)||(sp+0xfc<HEADS+1024&&HEADS<end)||(sp+0xfc<0x2468b0&&0x245f5c<end){return None;}
 let initial=word(mem,sp+0x1f0)?;if initial>256{return None;}
 let mut index=initial;let mut retired=0;let mut wrote=false;let mut head=0;
 let (flags,class)=loop{
  retired+=3;
  if index==256{regs[0]=256;regs[15]=0x2467dc;break(0x60000000,1);}
  head=word(mem,HEADS+index*4)?;wrote=true;retired+=13;
  regs[0]=0;regs[1]=head;
  if head!=0{regs[15]=0x246710;break((head&0x80000000)|0x20000000,0);}
  index+=1;retired+=5;
 };
 if retired>budget{return None;}
 Some(Outcome{regs,cpsr:(cpsr&0x0fffffff)|flags,retired,class,stack:sp,index,head,write_index:index!=initial,write_head:wrote})
}
// The parent whole-function/image guard must also pass before enabling State.
pub(crate)fn code_matches(mem:&Mem)->bool{CODE.iter().all(|(a,b)|mem.get_bytes_fallible(ConstPtr::<u8>::from_bits(*a).cast(),b.len()as u32).is_some_and(|s|s==*b))}
const CODE:&[(u32,&[u8])]=&[(0x2466e6,&[124, 152, 176, 245, 128, 127, 128, 242, 118, 128, 64, 246, 236, 64, 192, 242, 24, 0, 120, 68, 124, 153, 137, 0, 8, 68, 0, 104, 63, 144, 0, 32, 192, 242, 0, 0, 63, 153, 129, 66, 96, 208]),(0x2467d2,&[255, 231, 124, 152, 1, 48, 124, 144, 132, 231])];
