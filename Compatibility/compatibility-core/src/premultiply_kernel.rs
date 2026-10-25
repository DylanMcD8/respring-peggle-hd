/* This Source Code Form is subject to the Mozilla Public License,v2.0. */
//! Exact bounded GetImage premultiplication block. Outputs stay private until
//! source, ABI, mapped-span and live-shadow gates have passed.
use crate::cpu::CpuContext;
pub(crate)const ENTRY:u32=0x26a512;pub(crate)const EXIT:u32=0x26a59c;
pub(crate)const FRAME:u32=224;pub(crate)const MAX_BYTES:u32=8*1024*1024;
pub(crate)struct Outcome{pub context:CpuContext,pub frame:[u8;224],pub pixels:Vec<u8>,pub start:u32,pub length:u32}
fn word(b:&[u8],o:usize)->u32{u32::from_le_bytes(b[o..o+4].try_into().unwrap())}
fn put(b:&mut[u8],o:usize,v:u32){b[o..o+4].copy_from_slice(&v.to_le_bytes())}
pub(crate)fn disjoint(a:u32,n:u32,b:u32,m:u32)->bool{match(a.checked_add(n),b.checked_add(m)){(Some(ae),Some(be))=>ae<=b||be<=a,_=>false}}
fn sub_flags(a:u32,b:u32)->u32{let v=a.wrapping_sub(b);(v&0x80000000)|if v==0{0x40000000}else{0}|if a>=b{0x20000000}else{0}|if((a^b)&(a^v))>>31!=0{0x10000000}else{0}}
pub(crate)fn span(c:CpuContext,frame:&[u8])->Option<(u32,u32)>{
 if frame.len()!=224||c.cpsr&0x0600fc00!=0||c.regs[13]&3!=0{return None;}
 let start=word(frame,0x80);let end=word(frame,0x7c);
 if start>=end{return Some((start,0));}
 let n=end.checked_sub(start)?;if start&3!=0||end&3!=0||n>MAX_BYTES{return None;}
 let stack=c.regs[13].checked_sub(256)?;
 if !disjoint(start,n,stack,768)||!disjoint(start,n,0x1000,0x3ff000){return None;}
 Some((start,n))
}
pub(crate)fn calculate(mut c:CpuContext,prior:&[u8],input:&[u8])->Option<Outcome>{
 let(start,n)=span(c,prior)?;if input.len()!=n as usize{return None;}
 let mut frame:[u8;224]=prior.try_into().ok()?;let mut pixels=Vec::new();pixels.try_reserve_exact(input.len()).ok()?;pixels.extend_from_slice(input);
 let end=word(prior,0x7c);c.regs[0]=start;c.regs[1]=end;let mut flags=sub_flags(start,end);
 for p in pixels.chunks_exact_mut(4){let original=u32::from_le_bytes(p.try_into().ok()?);let a=original>>24;let r=((original>>16)&255)*a/255;let g=((original>>8)&255)*a/255;let b=(original&255)*a/255;
  p.copy_from_slice(&((a<<24)|(r<<16)|(g<<8)|b).to_le_bytes());
  put(&mut frame,0x38,255);put(&mut frame,0x78,a);put(&mut frame,0x74,r);put(&mut frame,0x70,g);put(&mut frame,0x6c,b);
 }
 if n!=0{put(&mut frame,0x80,end);c.regs[0]=end;c.regs[1]=end;c.regs[2]=end;c.regs[14]=0x26a57d;flags=0x60000000;}
 c.regs[15]=EXIT;c.cpsr=(c.cpsr&0x0fffffff)|flags|0x20;
 Some(Outcome{context:c,frame,pixels,start,length:n})
}
