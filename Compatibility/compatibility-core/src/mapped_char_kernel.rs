/* This Source Code Form is subject to the Mozilla Public License, v. 2.0. */
//! Audited ImageFont character mapping, including mixed-width search decisions.
//! Private output only; no cache, callbacks, Unicode conversion, or font writes.
use crate::cpu::CpuContext;
pub(crate) const MAX_COUNT:u32=65536;
pub(crate) const STACK_LEN:u32=40;
pub(crate) struct Outcome{pub(crate)context:CpuContext,pub(crate)stack:[u8;40]}
pub(crate)fn disjoint(a:u32,n:u32,b:u32,m:u32)->bool{match(a.checked_add(n),b.checked_add(m)){(Some(ae),Some(be))=>ae<=b||be<=a,_=>false}}
pub(crate)fn eligible(c:CpuContext,data:u32,table:u32,count:u32)->bool{
 let this=c.regs[0];let sp=c.regs[13];if c.cpsr&0x0600fc00!=0||sp<STACK_LEN||sp&3!=0||this&3!=0||data&3!=0||count>MAX_COUNT||count!=0&&table&3!=0{return false;}
 let spans=[(this,32),(data,96),(sp-STACK_LEN,STACK_LEN),(table,count*4)];
 for(i,&(a,n))in spans.iter().enumerate(){if n==0{continue;}if a.checked_add(n).is_none(){return false;}for&(b,m)in&spans[..i]{if m!=0&&!disjoint(a,n,b,m){return false;}}}true
}
fn word(b:&mut[u8],o:usize,v:u32){b[o..o+4].copy_from_slice(&v.to_le_bytes())}
fn sub_flags(a:u32,b:u32)->u32{let v=a.wrapping_sub(b);(v&0x80000000)|if v==0{0x40000000}else{0}|if a>=b{0x20000000}else{0}|if((a^b)&(a^v))>>31!=0{0x10000000}else{0}}
pub(crate)fn calculate(mut c:CpuContext,data:u32,table_base:u32,count:u32,table:&[u8],prior:&[u8])->Option<Outcome>{
 if !eligible(c,data,table_base,count)||prior.len()!=40||table.len()!=count as usize*4{return None;}
 let input=c.regs[1];let this=c.regs[0];let mut stack:[u8;40]=prior.try_into().ok()?;
 word(&mut stack,0,this);word(&mut stack,20,input);word(&mut stack,24,this);word(&mut stack,32,c.regs[7]);word(&mut stack,36,c.regs[14]);
 let mut flags=0x60000000; // CMP count,0 on the empty path.
 if count==0{c.regs[0]=input;c.regs[1]=0;}else{
  stack[4..6].copy_from_slice(&(input as u16).to_le_bytes());let(mut low,mut high)=(0i32,count as i32-1);let mut last=0usize;let mut had_mismatch=false;let mut hit=None;
  while low<=high{
   let mid=((low+high)/2)as usize;last=mid;let at=mid*4;let key=u16::from_le_bytes(table[at..at+2].try_into().ok()?)as u32;
   if key==input&0xffff{hit=Some(u16::from_le_bytes(table[at+2..at+4].try_into().ok()?)as u32);break;}
   had_mismatch=true;if(input as i32)<key as i32{high=mid as i32-1}else{low=mid as i32+1}
  }
  word(&mut stack,8,high as u32);word(&mut stack,12,low as u32);word(&mut stack,16,last as u32);
  if let Some(value)=hit{
   c.regs[0]=value;c.regs[1]=this;c.regs[2]=table_base;if had_mismatch{c.regs[3]=table_base;}
   // Last flag-setting instruction is LSLS midpoint,#2 after the equal CMP.
   // Bounded midpoint cannot shift a carry out. Equal CMP clears V.
   flags=if last==0{0x40000000}else{0};
  }else{
   c.regs[0]=input;c.regs[1]=high as u32;c.regs[2]=this;c.regs[3]=table_base;flags=sub_flags(low as u32,high as u32);
  }
 }
 word(&mut stack,28,c.regs[0]);c.cpsr=(c.cpsr&0x0fffffff)|flags;Some(Outcome{context:c,stack})
}
