/* This Source Code Form is subject to the Mozilla Public License, v. 2.0. */
//! Bounded private static translation of the original integer YCC leaf.
use crate::cpu::CpuContext;
#[derive(Clone)]pub struct Region{pub base:u32,pub bytes:Vec<u8>,pub writable:bool}
pub struct Outcome{pub context:CpuContext,pub regions:Vec<Region>}
struct Machine{c:CpuContext,regions:Vec<Region>}
impl Machine{
 fn load8(&self,a:u32)->Option<u32>{for r in &self.regions{if let Some(i)=a.checked_sub(r.base){if let Some(b)=r.bytes.get(i as usize){return Some(u32::from(*b));}}}None}
 fn load32(&self,a:u32)->Option<u32>{a.checked_add(4)?;for r in &self.regions{if let Some(i)=a.checked_sub(r.base){if let Some(b)=r.bytes.get(i as usize..).and_then(|b|b.get(..4)){return Some(u32::from_le_bytes(b.try_into().unwrap()));}}}None}
 fn store8(&mut self,a:u32,v:u32)->Option<()>{for r in &mut self.regions{if !r.writable{continue;}if let Some(i)=a.checked_sub(r.base){if let Some(b)=r.bytes.get_mut(i as usize){*b=v as u8;return Some(());}}}None}
 fn store32(&mut self,a:u32,v:u32)->Option<()>{a.checked_add(4)?;for r in &mut self.regions{if !r.writable{continue;}if let Some(i)=a.checked_sub(r.base){if let Some(b)=r.bytes.get_mut(i as usize..).and_then(|b|b.get_mut(..4)){b.copy_from_slice(&v.to_le_bytes());return Some(());}}}None}
 fn nz(&mut self,v:u32){self.c.cpsr=(self.c.cpsr&0x3fffffff)|(v&0x80000000)|if v==0{0x40000000}else{0};}
 fn cmp(&mut self,a:u32,b:u32){let v=a.wrapping_sub(b);self.nz(v);self.c.cpsr=(self.c.cpsr&0xcfffffff)|if a>=b{0x20000000}else{0}|if((a^b)&(a^v))>>31!=0{0x10000000}else{0};}
 fn add(&mut self,a:u32,b:u32)->u32{let(v,carry)=a.overflowing_add(b);self.nz(v);self.c.cpsr=(self.c.cpsr&0xcfffffff)|if carry{0x20000000}else{0}|if(!(a^b)&(a^v))>>31!=0{0x10000000}else{0};v}
 fn lsl(&mut self,a:u32,n:u32)->u32{let n=n&255;let v=if n<32{a<<n}else{0};self.nz(v);if n!=0{self.c.cpsr=(self.c.cpsr&!0x20000000)|if n<=32&&(a>>(32-n))&1!=0{0x20000000}else{0};}v}
 fn asr(&mut self,a:u32,n:u32)->u32{let n=n&255;let v=if n==0{a}else{((a as i32)>>n.min(31))as u32};self.nz(v);if n!=0{self.c.cpsr=(self.c.cpsr&!0x20000000)|if(a>>(n.min(32)-1))&1!=0{0x20000000}else{0};}v}
}
pub fn calculate(c:CpuContext,regions:Vec<Region>,pixels:u32,rows:u32)->Option<Outcome>{
 if c.regs[13]<80||c.regs[13]&3!=0||c.cpsr&0x0600fc00!=0||pixels>4096||rows>8||pixels.checked_mul(rows)?.checked_mul(3)?>98304{return None;}
 let mut m=Machine{c,regions};let mut pc=0x2a690c;let limit=64u32.checked_add(rows.checked_mul(8u32.checked_add(pixels.checked_mul(3)?)?)?)?;
 for _ in 0..limit{match pc{
0x2a690c=>{
m.c.regs[13]=m.c.regs[13].wrapping_sub(80u32);
m.c.regs[9]=m.load32(m.c.regs[13].wrapping_add(80u32))?;
m.store32(m.c.regs[13].wrapping_add(76u32),m.c.regs[0])?;
m.store32(m.c.regs[13].wrapping_add(72u32),m.c.regs[1])?;
m.store32(m.c.regs[13].wrapping_add(68u32),m.c.regs[2])?;
m.store32(m.c.regs[13].wrapping_add(64u32),m.c.regs[3])?;
m.store32(m.c.regs[13].wrapping_add(60u32),m.c.regs[9])?;
m.c.regs[0]=m.load32(m.c.regs[13].wrapping_add(76u32))?;
m.c.regs[0]=m.load32(m.c.regs[0].wrapping_add(456u32))?;
m.store32(m.c.regs[13].wrapping_add(56u32),m.c.regs[0])?;
m.c.regs[0]=m.load32(m.c.regs[13].wrapping_add(76u32))?;
m.c.regs[0]=m.load32(m.c.regs[0].wrapping_add(112u32))?;
m.store32(m.c.regs[13].wrapping_add(20u32),m.c.regs[0])?;
m.c.regs[0]=m.load32(m.c.regs[13].wrapping_add(76u32))?;
m.c.regs[0]=m.load32(m.c.regs[0].wrapping_add(324u32))?;
m.store32(m.c.regs[13].wrapping_add(16u32),m.c.regs[0])?;
m.c.regs[0]=m.load32(m.c.regs[13].wrapping_add(56u32))?;
m.c.regs[0]=m.load32(m.c.regs[0].wrapping_add(8u32))?;
m.store32(m.c.regs[13].wrapping_add(12u32),m.c.regs[0])?;
m.c.regs[0]=m.load32(m.c.regs[13].wrapping_add(56u32))?;
m.c.regs[0]=m.load32(m.c.regs[0].wrapping_add(12u32))?;
m.store32(m.c.regs[13].wrapping_add(8u32),m.c.regs[0])?;
m.c.regs[0]=m.load32(m.c.regs[13].wrapping_add(56u32))?;
m.c.regs[0]=m.load32(m.c.regs[0].wrapping_add(16u32))?;
m.store32(m.c.regs[13].wrapping_add(4u32),m.c.regs[0])?;
m.c.regs[0]=m.load32(m.c.regs[13].wrapping_add(56u32))?;
m.c.regs[0]=m.load32(m.c.regs[0].wrapping_add(20u32))?;
m.store32(m.c.regs[13],m.c.regs[0])?;
pc=0x2a694c;
}
0x2a694c=>{
m.c.regs[0]=m.load32(m.c.regs[13].wrapping_add(60u32))?;
m.c.regs[0]=m.c.regs[0].wrapping_add(4294967295u32);
m.store32(m.c.regs[13].wrapping_add(60u32),m.c.regs[0])?;
m.cmp(m.c.regs[0],0u32);
if ((m.c.cpsr>>31)^(m.c.cpsr>>28))&1!=0{pc=0x2a6a2c;continue;}
pc=0x2a6958;
}
0x2a6958=>{
m.c.regs[0]=0u32;m.nz(m.c.regs[0]);
m.c.regs[0]=(m.c.regs[0]&0xffff)|(0u32<<16);
m.c.regs[1]=m.load32(m.c.regs[13].wrapping_add(68u32))?;
m.c.regs[2]=m.load32(m.c.regs[13].wrapping_add(72u32))?;
m.c.regs[2]=m.load32(m.c.regs[2])?;
m.c.regs[1]=m.lsl(m.c.regs[1],2u32);
m.c.regs[1]=m.c.regs[1].wrapping_add(m.c.regs[2]);
m.c.regs[1]=m.load32(m.c.regs[1])?;
m.store32(m.c.regs[13].wrapping_add(36u32),m.c.regs[1])?;
m.c.regs[1]=m.load32(m.c.regs[13].wrapping_add(68u32))?;
m.c.regs[2]=m.load32(m.c.regs[13].wrapping_add(72u32))?;
m.c.regs[2]=m.load32(m.c.regs[2].wrapping_add(4u32))?;
m.c.regs[1]=m.lsl(m.c.regs[1],2u32);
m.c.regs[1]=m.c.regs[1].wrapping_add(m.c.regs[2]);
m.c.regs[1]=m.load32(m.c.regs[1])?;
m.store32(m.c.regs[13].wrapping_add(32u32),m.c.regs[1])?;
m.c.regs[1]=m.load32(m.c.regs[13].wrapping_add(68u32))?;
m.c.regs[2]=m.load32(m.c.regs[13].wrapping_add(72u32))?;
m.c.regs[2]=m.load32(m.c.regs[2].wrapping_add(8u32))?;
m.c.regs[1]=m.lsl(m.c.regs[1],2u32);
m.c.regs[1]=m.c.regs[1].wrapping_add(m.c.regs[2]);
m.c.regs[1]=m.load32(m.c.regs[1])?;
m.store32(m.c.regs[13].wrapping_add(28u32),m.c.regs[1])?;
m.c.regs[1]=m.load32(m.c.regs[13].wrapping_add(68u32))?;
m.c.regs[1]=m.add(m.c.regs[1],1u32);
m.store32(m.c.regs[13].wrapping_add(68u32),m.c.regs[1])?;
m.c.regs[1]=m.load32(m.c.regs[13].wrapping_add(64u32))?;
m.c.regs[2]=m.add(m.c.regs[1],4u32);
m.store32(m.c.regs[13].wrapping_add(64u32),m.c.regs[2])?;
m.c.regs[1]=m.load32(m.c.regs[1])?;
m.store32(m.c.regs[13].wrapping_add(40u32),m.c.regs[1])?;
m.store32(m.c.regs[13].wrapping_add(24u32),m.c.regs[0])?;
pc=0x2a699a;
}
0x2a699a=>{
m.c.regs[0]=m.load32(m.c.regs[13].wrapping_add(24u32))?;
m.c.regs[1]=m.load32(m.c.regs[13].wrapping_add(20u32))?;
m.cmp(m.c.regs[0],m.c.regs[1]);
if m.c.cpsr&0x20000000!=0{pc=0x2a6a2a;continue;}
pc=0x2a69a2;
}
0x2a69a2=>{
m.c.regs[0]=m.load32(m.c.regs[13].wrapping_add(24u32))?;
m.c.regs[1]=m.load32(m.c.regs[13].wrapping_add(36u32))?;
m.c.regs[0]=m.c.regs[0].wrapping_add(m.c.regs[1]);
m.c.regs[0]=m.load8(m.c.regs[0])?;
m.store32(m.c.regs[13].wrapping_add(52u32),m.c.regs[0])?;
m.c.regs[0]=m.load32(m.c.regs[13].wrapping_add(24u32))?;
m.c.regs[1]=m.load32(m.c.regs[13].wrapping_add(32u32))?;
m.c.regs[0]=m.c.regs[0].wrapping_add(m.c.regs[1]);
m.c.regs[0]=m.load8(m.c.regs[0])?;
m.store32(m.c.regs[13].wrapping_add(48u32),m.c.regs[0])?;
m.c.regs[0]=m.load32(m.c.regs[13].wrapping_add(24u32))?;
m.c.regs[1]=m.load32(m.c.regs[13].wrapping_add(28u32))?;
m.c.regs[0]=m.c.regs[0].wrapping_add(m.c.regs[1]);
m.c.regs[0]=m.load8(m.c.regs[0])?;
m.store32(m.c.regs[13].wrapping_add(44u32),m.c.regs[0])?;
m.c.regs[0]=m.load32(m.c.regs[13].wrapping_add(52u32))?;
m.c.regs[1]=m.load32(m.c.regs[13].wrapping_add(44u32))?;
m.c.regs[2]=m.load32(m.c.regs[13].wrapping_add(12u32))?;
m.c.regs[1]=m.lsl(m.c.regs[1],2u32);
m.c.regs[1]=m.c.regs[1].wrapping_add(m.c.regs[2]);
m.c.regs[1]=m.load32(m.c.regs[1])?;
m.c.regs[0]=m.c.regs[0].wrapping_add(m.c.regs[1]);
m.c.regs[1]=m.load32(m.c.regs[13].wrapping_add(16u32))?;
m.c.regs[0]=m.c.regs[0].wrapping_add(m.c.regs[1]);
m.c.regs[0]=m.load8(m.c.regs[0])?;
m.c.regs[1]=m.load32(m.c.regs[13].wrapping_add(40u32))?;
m.store8(m.c.regs[1],m.c.regs[0])?;
m.c.regs[0]=m.load32(m.c.regs[13].wrapping_add(52u32))?;
m.c.regs[1]=m.load32(m.c.regs[13].wrapping_add(48u32))?;
m.c.regs[2]=m.load32(m.c.regs[13])?;
m.c.regs[1]=m.lsl(m.c.regs[1],2u32);
m.c.regs[1]=m.c.regs[1].wrapping_add(m.c.regs[2]);
m.c.regs[1]=m.load32(m.c.regs[1])?;
m.c.regs[2]=m.load32(m.c.regs[13].wrapping_add(44u32))?;
m.c.regs[3]=m.load32(m.c.regs[13].wrapping_add(4u32))?;
m.c.regs[2]=m.lsl(m.c.regs[2],2u32);
m.c.regs[2]=m.c.regs[2].wrapping_add(m.c.regs[3]);
m.c.regs[2]=m.load32(m.c.regs[2])?;
m.c.regs[1]=m.c.regs[1].wrapping_add(m.c.regs[2]);
m.c.regs[2]=16u32;m.nz(m.c.regs[2]);
m.c.regs[2]=(m.c.regs[2]&0xffff)|(0u32<<16);
m.c.regs[1]=m.asr(m.c.regs[1],m.c.regs[2]);
m.c.regs[0]=m.c.regs[0].wrapping_add(m.c.regs[1]);
m.c.regs[1]=m.load32(m.c.regs[13].wrapping_add(16u32))?;
m.c.regs[0]=m.c.regs[0].wrapping_add(m.c.regs[1]);
m.c.regs[0]=m.load8(m.c.regs[0])?;
m.c.regs[1]=m.load32(m.c.regs[13].wrapping_add(40u32))?;
m.store8(m.c.regs[1].wrapping_add(1u32),m.c.regs[0])?;
m.c.regs[0]=m.load32(m.c.regs[13].wrapping_add(52u32))?;
m.c.regs[1]=m.load32(m.c.regs[13].wrapping_add(48u32))?;
m.c.regs[2]=m.load32(m.c.regs[13].wrapping_add(8u32))?;
m.c.regs[1]=m.lsl(m.c.regs[1],2u32);
m.c.regs[1]=m.c.regs[1].wrapping_add(m.c.regs[2]);
m.c.regs[1]=m.load32(m.c.regs[1])?;
m.c.regs[0]=m.c.regs[0].wrapping_add(m.c.regs[1]);
m.c.regs[1]=m.load32(m.c.regs[13].wrapping_add(16u32))?;
m.c.regs[0]=m.c.regs[0].wrapping_add(m.c.regs[1]);
m.c.regs[0]=m.load8(m.c.regs[0])?;
m.c.regs[1]=m.load32(m.c.regs[13].wrapping_add(40u32))?;
m.store8(m.c.regs[1].wrapping_add(2u32),m.c.regs[0])?;
m.c.regs[0]=m.load32(m.c.regs[13].wrapping_add(40u32))?;
m.c.regs[0]=m.add(m.c.regs[0],3u32);
m.store32(m.c.regs[13].wrapping_add(40u32),m.c.regs[0])?;
m.c.regs[0]=m.load32(m.c.regs[13].wrapping_add(24u32))?;
m.c.regs[0]=m.add(m.c.regs[0],1u32);
m.store32(m.c.regs[13].wrapping_add(24u32),m.c.regs[0])?;
pc=0x2a699a;continue;
}
0x2a6a2a=>{
pc=0x2a694c;continue;
}
0x2a6a2c=>{
m.c.regs[13]=m.c.regs[13].wrapping_add(80u32);
return Some(Outcome{context:m.c,regions:m.regions});
}
_=>return None,}}None}
