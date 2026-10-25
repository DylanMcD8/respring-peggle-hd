/* This Source Code Form is subject to the Mozilla Public License,v2.0. */
//! Two exact bounded inner loops. Decoder/state ownership stays original.
use crate::cpu::CpuContext;
pub(crate)const ENTRIES:[u32;2]=[0x28b614,0x28b84a];pub(crate)const EXITS:[u32;2]=[0x28b63c,0x28b866];pub(crate)const LENGTHS:[u32;2]=[40,28];
pub(crate)struct Outcome{pub context:CpuContext,pub frame:[u8;112],pub source:u32,pub source_after:Vec<u8>,pub dest:u32,pub output:Vec<u8>,pub class:usize}
pub(crate)fn word(b:&[u8],n:usize)->u32{u32::from_le_bytes(b[n..n+4].try_into().unwrap())}fn put(b:&mut[u8],n:usize,v:u32){b[n..n+4].copy_from_slice(&v.to_le_bytes())}
pub(crate)fn disjoint(a:u32,n:u32,b:u32,m:u32)->bool{match(a.checked_add(n),b.checked_add(m)){(Some(x),Some(y))=>x<=b||y<=a,_=>false}}
fn flags(a:u32,b:u32)->u32{let v=a.wrapping_sub(b);(v&0x80000000)|if v==0{0x40000000}else{0}|if a>=b{0x20000000}else{0}|if((a^b)&(a^v))>>31!=0{0x10000000}else{0}}
pub(crate)fn calculate(mut c:CpuContext,prior:&[u8],source:&[u8],destination:&[u8],routine:usize)->Option<Outcome>{
 if routine>1||prior.len()!=112||c.cpsr&0x0600fc00!=0||c.regs[13]&3!=0{return None;}let sp=c.regs[13];let guard=sp.checked_sub(256)?;sp.checked_add(512)?;if !disjoint(sp,112,0x1000,0x3ff000){return None;}let mut frame:[u8;112]=prior.try_into().ok()?;
 let(src,dst,class,output,source_after)=if routine==0{
  let mut k=word(prior,0x3c);let mut bits=word(prior,0x40);let available=word(prior,0x34);let ptr=word(prior,0x38);let n=if k>=20{0}else{(20-k+7)/8};if source.len()!=n as usize||!destination.is_empty()||available<n{return None;}
  if n>0&&(!disjoint(ptr,n,guard,768)||!disjoint(ptr,n,0x1000,0x3ff000)){return None;}ptr.checked_add(n)?;
  for &byte in source{c.regs[1]=bits;bits|=(byte as u32)<<k;k+=8;}
  if n>0{put(&mut frame,0x3c,k);put(&mut frame,0x40,bits);put(&mut frame,0x34,available-n);put(&mut frame,0x38,ptr+n);}
  c.regs[0]=k;c.cpsr=(c.cpsr&0x0fffffff)|flags(k,20)|0x20;(ptr,0,n as usize,Vec::new(),source.to_vec())
 }else{
  let n=word(prior,0x20);let src=word(prior,0x18);let dst=word(prior,0x30);if n==0||n>258||source.len()!=n as usize||destination.len()!=n as usize{return None;}src.checked_add(n)?;dst.checked_add(n)?;
  for a in [src,dst]{if !disjoint(a,n,guard,768)||!disjoint(a,n,0x1000,0x3ff000){return None;}}
  let class=if disjoint(src,n,dst,n){0}else if src<dst{1}else if src==dst{2}else{3};let mut out=destination.to_vec();
  // Forward LZ expansion reads bytes already produced when distance<length.
  // memmove or a source snapshot alone would silently corrupt repetitions.
  for i in 0..n as usize{let a=src+i as u32;out[i]=if a>=dst&&a<dst+i as u32{out[(a-dst)as usize]}else{source[i]};}
  let mut after=source.to_vec();for(i,x)in after.iter_mut().enumerate(){let a=src+i as u32;if a>=dst&&a<dst+n{*x=out[(a-dst)as usize];}}
  put(&mut frame,0x18,src+n);put(&mut frame,0x30,dst+n);put(&mut frame,0x20,0);c.regs[0]=0;c.regs[1]=dst+n-1;c.regs[2]=dst+n;c.cpsr=(c.cpsr&0x0fffffff)|0x60000020;(src,dst,class,out,after)
 };
 c.regs[15]=EXITS[routine];Some(Outcome{context:c,frame,source:src,source_after,dest:dst,output,class})
}
