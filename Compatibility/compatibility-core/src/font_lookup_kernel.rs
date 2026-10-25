/* This Source Code Form is subject to the Mozilla Public License, v. 2.0. */
//! Native bounded dictionary searches, preserving audited ARM register/stack effects.
//! No cache, font mutation, host FP, drawing, allocation or guest callbacks.
use crate::cpu::CpuContext;
#[derive(Clone,Copy,Debug,PartialEq,Eq)]pub(crate)enum Kind{Glyph,Kerning}
impl Kind{pub(crate)fn stride(self)->usize{if self==Self::Glyph{36}else{8}}pub(crate)fn stack_len(self)->usize{if self==Self::Glyph{40}else{44}}}
pub(crate)struct Outcome{pub(crate)context:CpuContext,pub(crate)stack:[u8;44],pub(crate)length:usize}
fn put(b:&mut[u8],i:usize,v:u32){b[i..i+4].copy_from_slice(&v.to_le_bytes())}
pub(crate)fn spans_disjoint(a:u32,n:u32,b:u32,m:u32)->bool{match(a.checked_add(n),b.checked_add(m)){(Some(ae),Some(be))=>ae<=b||be<=a,_=>false}}
pub(crate)fn eligible(c:CpuContext,kind:Kind,this:u32,base:u32,count:u16)->bool{
 let n=kind.stack_len()as u32;let sp=c.regs[13];
 if c.cpsr&0x0600fc00!=0||sp<n||sp&3!=0||this&3!=0||base&3!=0{return false;}
 spans_disjoint(this,if kind==Kind::Glyph{0x3e}else{0x46},sp-n,n)&&spans_disjoint(base,u32::from(count)*kind.stride()as u32,sp-n,n)
}
pub(crate)fn calculate(mut c:CpuContext,kind:Kind,base:u32,count:u16,table:&[u8],prior_stack:&[u8])->Option<Outcome>{
 let this=c.regs[0];if !eligible(c,kind,this,base,count)||prior_stack.len()!=kind.stack_len()||table.len()!=usize::from(count)*kind.stride(){return None;}
 let mut stack=[0u8;44];let len=prior_stack.len();stack[..len].copy_from_slice(prior_stack);let left=c.regs[1];let right=c.regs[2];
 let key=if kind==Kind::Glyph{left&0xffff}else{((right&0xffff)<<16)|(left&0xffff)};
 put(&mut stack,0,this);if kind==Kind::Glyph{stack[4..6].copy_from_slice(&(key as u16).to_le_bytes());put(&mut stack,20,left);put(&mut stack,24,this);}else{put(&mut stack,4,key);put(&mut stack,20,right);put(&mut stack,24,left);put(&mut stack,28,this);}
 put(&mut stack,len-8,c.regs[7]);put(&mut stack,len-4,c.regs[14]);
 let(mut lo,mut hi)=(0i32,i32::from(count)-1);let mut mid=None;let mut found=None;let mut mismatch=false;
 while lo<=hi{
  let m=((lo+hi)/2)as usize;mid=Some(m);let at=m*kind.stride();let item=if kind==Kind::Glyph{u32::from(u16::from_le_bytes(table[at..at+2].try_into().unwrap()))}else{u32::from_le_bytes(table[at..at+4].try_into().unwrap())};
  if item==key{found=Some(m);break;}
  mismatch=true;if key<item{hi=m as i32-1}else{lo=m as i32+1}
 }
 put(&mut stack,8,hi as u32);put(&mut stack,12,lo as u32);if let Some(m)=mid{put(&mut stack,16,m as u32);}
 let flags;if let Some(m)=found{
  c.regs[0]=base.checked_add((m*kind.stride())as u32)?;c.regs[1]=this;c.regs[2]=base;
  if kind==Kind::Glyph{c.regs[3]=36;flags=if m==0{0x60000000}else{0x20000000};}else{c.regs[3]=if mismatch{base}else{0};flags=if m==0{0x40000000}else{0};}
 }else{
  c.regs[0]=0;c.regs[1]=hi as u32;
  if kind==Kind::Glyph{c.regs[2]=if mid.is_some(){this}else{0};if mid.is_some(){c.regs[3]=base;}}
  else{c.regs[2]=if mid.is_some(){this}else{left&0xffff};c.regs[3]=if mid.is_some(){base}else{0};}
  flags=0x40000000|if (lo as u32)>=(hi as u32){0x20000000}else{0};
 }
 if kind==Kind::Glyph&&mismatch{c.regs[9]=36;}
 put(&mut stack,if kind==Kind::Glyph{28}else{32},c.regs[0]);c.cpsr=(c.cpsr&0x0fffffff)|flags;
 Some(Outcome{context:c,stack,length:len})
}
