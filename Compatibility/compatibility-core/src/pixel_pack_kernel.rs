/* This Source Code Form is subject to the Mozilla Public License,v2.0. */
//! Exact in-place GetImage16bit packing. No owners, allocation callbacks or cache.
use crate::cpu::CpuContext;
pub(crate)const ENTRIES:[u32;3]=[0x26a5e2,0x26a672,0x26a6e6];
pub(crate)const EXITS:[u32;3]=[0x26a66c,0x26a6e0,0x26a770];
pub(crate)const SLOTS:[usize;3]=[0x60,0x5c,0x58];
pub(crate)const MAX_BYTES:u32=8*1024*1024;
pub(crate)struct Outcome{pub context:CpuContext,pub frame:[u8;224],pub buffer:Vec<u8>,pub start:u32,pub length:u32}
pub(crate)fn word(b:&[u8],o:usize)->u32{u32::from_le_bytes(b[o..o+4].try_into().unwrap())}
fn put(b:&mut[u8],o:usize,v:u32){b[o..o+4].copy_from_slice(&v.to_le_bytes())}
pub(crate)fn disjoint(a:u32,n:u32,b:u32,m:u32)->bool{match(a.checked_add(n),b.checked_add(m)){(Some(ae),Some(be))=>ae<=b||be<=a,_=>false}}
pub(crate)fn packed(p:u32,class:usize)->u16{let x=p.to_le_bytes();let(a,b,c,d)=(x[0]as u16,x[1]as u16,x[2]as u16,x[3]as u16);match class{0=>(a>>4)<<12|(b>>4)<<8|(c>>4)<<4|d>>4,1=>(a>>3)<<11|(b>>2)<<5|c>>3,2=>(a>>3)<<11|(b>>3)<<6|(c>>3)<<1|d>>7,_=>unreachable!()}}
pub(crate)fn calculate(mut c:CpuContext,prior:&[u8],input:&[u8],class:usize)->Option<Outcome>{
 if class>=3||prior.len()!=224||c.cpsr&0x0600fc00!=0||c.regs[13]&3!=0{return None;}
 let sp=c.regs[13];sp.checked_sub(256)?;sp.checked_add(512)?;if !disjoint(sp,224,0x1000,0x3ff000){return None;}
 let count=word(prior,0x64)as i32;let start=word(prior,0x68);let n=(count.max(0)as u32).checked_mul(4)?;
 if n>MAX_BYTES||input.len()!=n as usize||word(prior,SLOTS[class])!=start{return None;}
 if n!=0{let guard=c.regs[13].checked_sub(256)?;if start&3!=0||!disjoint(start,n,guard,768)||!disjoint(start,n,0x1000,0x3ff000){return None;}}
 let mut frame:[u8;224]=prior.try_into().ok()?;let mut buffer=Vec::new();buffer.try_reserve_exact(input.len()).ok()?;buffer.extend_from_slice(input);
 for(j,p)in input.chunks_exact(4).enumerate(){let value=packed(u32::from_le_bytes(p.try_into().ok()?),class);buffer[j*2..j*2+2].copy_from_slice(&value.to_le_bytes());}
 put(&mut frame,0x64,if count>0{u32::MAX}else{(count as u32).wrapping_sub(1)});
 if n!=0{put(&mut frame,0x68,start.checked_add(n)?);let dest=start.checked_add(n/2)?;put(&mut frame,SLOTS[class],dest);c.regs[2]=dest;}
 c.regs[0]=count.min(0)as u32;c.regs[1]=word(&frame,0x64);c.regs[15]=EXITS[class];let flags=if count>=0{0x60000000}else{0xa0000000};c.cpsr=(c.cpsr&0x0fffffff)|flags|0x20;
 Some(Outcome{context:c,frame,buffer,start,length:n})
}
