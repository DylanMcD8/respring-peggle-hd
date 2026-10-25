/* This Source Code Form is subject to the Mozilla Public License, v. 2.0. */
//! Audited AddVertex native leaf: private prediction, exact integer/VFP state.
//! No guest callbacks, GL calls, approximate FP, persistent cache or JIT.
use crate::cpu::CpuContext;
pub(crate)const GLOBAL:u32=0x36b344;pub(crate)const VERTICES:u32=0x36e87c;pub(crate)const COLORS:u32=0x37e57c;pub(crate)const COUNTS:u32=0x3863fc;pub(crate)const CAPACITY:u32=8096;
#[derive(Clone)]pub(crate)struct Region{pub(crate)base:u32,pub(crate)data:Vec<u8>,pub(crate)dirty:Option<(usize,usize)>}
pub(crate)struct Inputs{pub(crate)context:CpuContext,pub(crate)configuration:[u8;8],pub(crate)counters:[u8;8],pub(crate)vertex:[u8;8],pub(crate)colors:Vec<u8>,pub(crate)stack:[u8;24],pub(crate)fifth:u32}
pub(crate)struct Outcome{pub(crate)context:CpuContext,pub(crate)stack:[u8;24],pub(crate)regions:Vec<Region>,pub(crate)class:usize,pub(crate)blocks:u32}
pub(crate)fn color_span(c:CpuContext,configuration:&[u8;8],counters:&[u8;8])->Option<(u32,u32,usize)>{
 let v=u32::from_le_bytes(counters[..4].try_into().unwrap());let colors=u32::from_le_bytes(counters[4..].try_into().unwrap());let old=u32::from_le_bytes(configuration[..4].try_into().unwrap());if v>=CAPACITY||colors>=CAPACITY||colors!=0&&colors!=v{return None;}
 let(path,base,len)=if colors!=0{(0,COLORS+colors*4,4)}else if old==c.regs[2]{(1,COLORS,0)}else if v==0{(2,COLORS,0)}else{(3,COLORS,(v+1)*4)};Some((base,len,path*2+usize::from(configuration[4]&1!=0)))
}
fn disjoint(a:u32,n:u32,b:u32,m:u32)->bool{match(a.checked_add(n),b.checked_add(m)){(Some(ae),Some(be))=>ae<=b||be<=a,_=>false}}
struct Machine{c:CpuContext,frame:u32,entry:u32,fifth:u32,stack:[u8;24],regions:Vec<Region>}
impl Machine{
 #[inline(always)]fn nz(&mut self,v:u32){self.c.cpsr=(self.c.cpsr&0x3fffffff)|(v&0x80000000)|if v==0{0x40000000}else{0};}
 #[inline(always)]fn add(&mut self,a:u32,b:u32)->u32{let r=a.wrapping_add(b);let carry=u64::from(a)+u64::from(b)>u64::from(u32::MAX);let overflow=(!(a^b)&(a^r))&0x80000000!=0;self.c.cpsr=(self.c.cpsr&0x0fffffff)|(r&0x80000000)|if r==0{0x40000000}else{0}|if carry{0x20000000}else{0}|if overflow{0x10000000}else{0};r}
 #[inline(always)]fn sub(&mut self,a:u32,b:u32)->u32{let r=a.wrapping_sub(b);let overflow=((a^b)&(a^r))&0x80000000!=0;self.c.cpsr=(self.c.cpsr&0x0fffffff)|(r&0x80000000)|if r==0{0x40000000}else{0}|if a>=b{0x20000000}else{0}|if overflow{0x10000000}else{0};r}
 #[inline(always)]fn lsl(&mut self,a:u32,n:u32)->u32{if n==0{return a;}let out=a.wrapping_shl(n);self.c.cpsr=(self.c.cpsr&!0x20000000)|if a>>(32-n)&1!=0{0x20000000}else{0};self.nz(out);out}
 #[inline(always)]fn load8(&self,a:u32)->Option<u8>{if let Some(o)=a.checked_sub(self.frame){if let Some(v)=self.stack.get(o as usize){return Some(*v);}}if let Some(o)=a.checked_sub(self.entry){if o<4{return Some(self.fifth.to_le_bytes()[o as usize]);}}for r in &self.regions{if let Some(o)=a.checked_sub(r.base){if let Some(v)=r.data.get(o as usize){return Some(*v);}}}None}
 #[inline(always)]fn load32(&self,a:u32)->Option<u32>{Some(u32::from_le_bytes([self.load8(a)?,self.load8(a.checked_add(1)?)?,self.load8(a.checked_add(2)?)?,self.load8(a.checked_add(3)?)?]))}
 #[inline(always)]fn store8(&mut self,a:u32,v:u8)->Option<()>{if let Some(o)=a.checked_sub(self.frame){if let Some(p)=self.stack.get_mut(o as usize){*p=v;return Some(());}}for r in &mut self.regions{if let Some(o)=a.checked_sub(r.base){if let Some(p)=r.data.get_mut(o as usize){*p=v;let o=o as usize;r.dirty=Some(match r.dirty{Some((b,e))=>(b.min(o),e.max(o+1)),None=>(o,o+1)});return Some(());}}}None}
 #[inline(always)]fn store32(&mut self,a:u32,v:u32)->Option<()>{for(i,b)in v.to_le_bytes().into_iter().enumerate(){self.store8(a.checked_add(i as u32)?,b)?;}Some(())}
 #[inline(always)]fn store16(&mut self,a:u32,v:u16)->Option<()>{for(i,b)in v.to_le_bytes().into_iter().enumerate(){self.store8(a.checked_add(i as u32)?,b)?;}Some(())}
}
pub(crate)fn calculate(input:Inputs)->Option<Outcome>{
 let c=input.context;if c.regs[13]<24||c.regs[13]&3!=0||c.cpsr&0x0600fc00!=0{return None;}let(color_base,color_len,class)=color_span(c,&input.configuration,&input.counters)?;if input.colors.len()!=color_len as usize{return None;}let count=u32::from_le_bytes(input.counters[..4].try_into().unwrap());let mut regions=vec![Region{base:GLOBAL,data:input.configuration.to_vec(),dirty:None},Region{base:COUNTS,data:input.counters.to_vec(),dirty:None},Region{base:VERTICES+count*8,data:input.vertex.to_vec(),dirty:None}];if color_len!=0{regions.push(Region{base:color_base,data:input.colors,dirty:None});}
 for(i,r)in regions.iter().enumerate(){if !disjoint(r.base,r.data.len()as u32,c.regs[13]-24,28){return None;}for s in &regions[i+1..]{if !disjoint(r.base,r.data.len()as u32,s.base,s.data.len()as u32){return None;}}}
 let mut m=Machine{c,frame:c.regs[13]-24,entry:c.regs[13],fifth:input.fifth,stack:input.stack,regions};let mut pc=0x22d5fc;let limit=128+count*8;
 for blocks in 0..limit{match pc{
 0x22d5fc=>{
  // 22d5fc sub sp, #0x18
  m.c.regs[13]=m.c.regs[13].wrapping_sub(24u32);
  // 22d5fe vmov d0, r3, r3
  m.c.extregs[0]=m.c.regs[3];m.c.extregs[1]=m.c.regs[3];
  // 22d602 vmov.f32 s4, s0
  m.c.extregs[4]=m.c.extregs[0];
  // 22d606 vmov d0, r1, r1
  m.c.extregs[0]=m.c.regs[1];m.c.extregs[1]=m.c.regs[1];
  // 22d60a vmov.f32 s8, s0
  m.c.extregs[8]=m.c.extregs[0];
  // 22d60e vmov d0, r0, r0
  m.c.extregs[0]=m.c.regs[0];m.c.extregs[1]=m.c.regs[0];
  // 22d612 vmov.f32 s12, s0
  m.c.extregs[12]=m.c.extregs[0];
  // 22d616 vldr s2, [sp, #24]
  m.c.extregs[2]=m.load32(m.c.regs[13].wrapping_add(24u32))?;
  // 22d61a movw r0, #0x8dda
  m.c.regs[0]=36314u32;
  // 22d61e movt r0, #0x15
  m.c.regs[0]=(m.c.regs[0]&0xffff)|(21u32<<16);
  // 22d622 add r0, pc
  m.c.regs[0]=m.c.regs[0].wrapping_add(0x22d626u32);
  // 22d624 vstr s12, [sp, #20]
  m.store32(m.c.regs[13].wrapping_add(20u32),m.c.extregs[12])?;
  // 22d628 vstr s8, [sp, #16]
  m.store32(m.c.regs[13].wrapping_add(16u32),m.c.extregs[8])?;
  // 22d62c str r2, [sp, #0xc]
  m.store32(m.c.regs[13].wrapping_add(12u32),m.c.regs[2])?;
  // 22d62e vstr s4, [sp, #8]
  m.store32(m.c.regs[13].wrapping_add(8u32),m.c.extregs[4])?;
  // 22d632 vstr s2, [sp, #4]
  m.store32(m.c.regs[13].wrapping_add(4u32),m.c.extregs[2])?;
  // 22d636 ldr r0, [r0]
  m.c.regs[0]=m.load32(m.c.regs[0].wrapping_add(0u32))?;
  // 22d638 cmp r0, #0x0
  m.sub(m.c.regs[0],0u32);
  // 22d63a ble 0x22d664 <__ZN4Sexy9AddVertexEffjff+0x68>
  if m.c.cpsr&0x40000000!=0||((m.c.cpsr>>31)^((m.c.cpsr>>28)&1))!=0{pc=0x22d664;continue;}
  pc=0x22d63c;continue;
 },
 0x22d63c=>{
  // 22d63c movw r0, #0xf34
  m.c.regs[0]=3892u32;
  // 22d640 movt r0, #0x15
  m.c.regs[0]=(m.c.regs[0]&0xffff)|(21u32<<16);
  // 22d644 add r0, pc
  m.c.regs[0]=m.c.regs[0].wrapping_add(0x22d648u32);
  // 22d646 movw r1, #0x8dae
  m.c.regs[1]=36270u32;
  // 22d64a movt r1, #0x15
  m.c.regs[1]=(m.c.regs[1]&0xffff)|(21u32<<16);
  // 22d64e add r1, pc
  m.c.regs[1]=m.c.regs[1].wrapping_add(0x22d652u32);
  // 22d650 ldr r2, [sp, #0xc]
  m.c.regs[2]=m.load32(m.c.regs[13].wrapping_add(12u32))?;
  // 22d652 ldr r3, [r1]
  m.c.regs[3]=m.load32(m.c.regs[1].wrapping_add(0u32))?;
  // 22d654 add.w r9, r3, #0x1
  m.c.regs[9]=m.c.regs[3].wrapping_add(1u32);
  // 22d658 str.w r9, [r1]
  m.store32(m.c.regs[1].wrapping_add(0u32),m.c.regs[9])?;
  // 22d65c lsls r1, r3, #0x2
  m.c.regs[1]=m.lsl(m.c.regs[3],2u32);
  // 22d65e add r0, r1
  m.c.regs[0]=m.c.regs[0].wrapping_add(m.c.regs[1]);
  // 22d660 str r2, [r0]
  m.store32(m.c.regs[0].wrapping_add(0u32),m.c.regs[2])?;
  // 22d662 b 0x22d70c <__ZN4Sexy9AddVertexEffjff+0x110>
  pc=0x22d70c;continue;
 },
 0x22d664=>{
  // 22d664 movw r0, #0xdcd4
  m.c.regs[0]=56532u32;
  // 22d668 movt r0, #0x13
  m.c.regs[0]=(m.c.regs[0]&0xffff)|(19u32<<16);
  // 22d66c add r0, pc
  m.c.regs[0]=m.c.regs[0].wrapping_add(0x22d670u32);
  // 22d66e ldr r1, [sp, #0xc]
  m.c.regs[1]=m.load32(m.c.regs[13].wrapping_add(12u32))?;
  // 22d670 ldr r0, [r0]
  m.c.regs[0]=m.load32(m.c.regs[0].wrapping_add(0u32))?;
  // 22d672 cmp r1, r0
  m.sub(m.c.regs[1],m.c.regs[0]);
  // 22d674 beq 0x22d70a <__ZN4Sexy9AddVertexEffjff+0x10e>
  if m.c.cpsr&0x40000000!=0{pc=0x22d70a;continue;}
  pc=0x22d676;continue;
 },
 0x22d676=>{
  // 22d676 movw r0, #0x8d7a
  m.c.regs[0]=36218u32;
  // 22d67a movt r0, #0x15
  m.c.regs[0]=(m.c.regs[0]&0xffff)|(21u32<<16);
  // 22d67e add r0, pc
  m.c.regs[0]=m.c.regs[0].wrapping_add(0x22d682u32);
  // 22d680 ldr r0, [r0]
  m.c.regs[0]=m.load32(m.c.regs[0].wrapping_add(0u32))?;
  // 22d682 cmp r0, #0x0
  m.sub(m.c.regs[0],0u32);
  // 22d684 ble 0x22d6fa <__ZN4Sexy9AddVertexEffjff+0xfe>
  if m.c.cpsr&0x40000000!=0||((m.c.cpsr>>31)^((m.c.cpsr>>28)&1))!=0{pc=0x22d6fa;continue;}
  pc=0x22d686;continue;
 },
 0x22d686=>{
  // 22d686 movs r0, #0x0
  m.c.regs[0]=0u32;
  m.nz(m.c.regs[0]);
  // 22d688 movt r0, #0x0
  m.c.regs[0]=(m.c.regs[0]&0xffff)|(0u32<<16);
  // 22d68c str r0, [sp]
  m.store32(m.c.regs[13].wrapping_add(0u32),m.c.regs[0])?;
  pc=0x22d68e;continue;
 },
 0x22d68e=>{
  // 22d68e movw r0, #0x8d62
  m.c.regs[0]=36194u32;
  // 22d692 movt r0, #0x15
  m.c.regs[0]=(m.c.regs[0]&0xffff)|(21u32<<16);
  // 22d696 add r0, pc
  m.c.regs[0]=m.c.regs[0].wrapping_add(0x22d69au32);
  // 22d698 ldr r1, [sp]
  m.c.regs[1]=m.load32(m.c.regs[13].wrapping_add(0u32))?;
  // 22d69a ldr r0, [r0]
  m.c.regs[0]=m.load32(m.c.regs[0].wrapping_add(0u32))?;
  // 22d69c cmp r1, r0
  m.sub(m.c.regs[1],m.c.regs[0]);
  // 22d69e bge 0x22d6c6 <__ZN4Sexy9AddVertexEffjff+0xca>
  if ((m.c.cpsr>>31)^((m.c.cpsr>>28)&1))==0{pc=0x22d6c6;continue;}
  pc=0x22d6a0;continue;
 },
 0x22d6a0=>{
  // 22d6a0 movw r0, #0xed0
  m.c.regs[0]=3792u32;
  // 22d6a4 movt r0, #0x15
  m.c.regs[0]=(m.c.regs[0]&0xffff)|(21u32<<16);
  // 22d6a8 add r0, pc
  m.c.regs[0]=m.c.regs[0].wrapping_add(0x22d6acu32);
  // 22d6aa movw r1, #0xdc8e
  m.c.regs[1]=56462u32;
  // 22d6ae movt r1, #0x13
  m.c.regs[1]=(m.c.regs[1]&0xffff)|(19u32<<16);
  // 22d6b2 add r1, pc
  m.c.regs[1]=m.c.regs[1].wrapping_add(0x22d6b6u32);
  // 22d6b4 ldr r1, [r1]
  m.c.regs[1]=m.load32(m.c.regs[1].wrapping_add(0u32))?;
  // 22d6b6 ldr r2, [sp]
  m.c.regs[2]=m.load32(m.c.regs[13].wrapping_add(0u32))?;
  // 22d6b8 lsls r2, r2, #0x2
  m.c.regs[2]=m.lsl(m.c.regs[2],2u32);
  // 22d6ba add r0, r2
  m.c.regs[0]=m.c.regs[0].wrapping_add(m.c.regs[2]);
  // 22d6bc str r1, [r0]
  m.store32(m.c.regs[0].wrapping_add(0u32),m.c.regs[1])?;
  // 22d6be ldr r0, [sp]
  m.c.regs[0]=m.load32(m.c.regs[13].wrapping_add(0u32))?;
  // 22d6c0 adds r0, #0x1
  m.c.regs[0]=m.add(m.c.regs[0],1u32);
  // 22d6c2 str r0, [sp]
  m.store32(m.c.regs[13].wrapping_add(0u32),m.c.regs[0])?;
  // 22d6c4 b 0x22d68e <__ZN4Sexy9AddVertexEffjff+0x92>
  pc=0x22d68e;continue;
 },
 0x22d6c6=>{
  // 22d6c6 movw r0, #0x8d2e
  m.c.regs[0]=36142u32;
  // 22d6ca movt r0, #0x15
  m.c.regs[0]=(m.c.regs[0]&0xffff)|(21u32<<16);
  // 22d6ce add r0, pc
  m.c.regs[0]=m.c.regs[0].wrapping_add(0x22d6d2u32);
  // 22d6d0 movw r1, #0x8d20
  m.c.regs[1]=36128u32;
  // 22d6d4 movt r1, #0x15
  m.c.regs[1]=(m.c.regs[1]&0xffff)|(21u32<<16);
  // 22d6d8 add r1, pc
  m.c.regs[1]=m.c.regs[1].wrapping_add(0x22d6dcu32);
  // 22d6da movw r2, #0xe96
  m.c.regs[2]=3734u32;
  // 22d6de movt r2, #0x15
  m.c.regs[2]=(m.c.regs[2]&0xffff)|(21u32<<16);
  // 22d6e2 add r2, pc
  m.c.regs[2]=m.c.regs[2].wrapping_add(0x22d6e6u32);
  // 22d6e4 ldr r3, [sp, #0xc]
  m.c.regs[3]=m.load32(m.c.regs[13].wrapping_add(12u32))?;
  // 22d6e6 ldr.w r9, [r1]
  m.c.regs[9]=m.load32(m.c.regs[1].wrapping_add(0u32))?;
  // 22d6ea lsl.w r9, r9, #0x2
  m.c.regs[9]=m.c.regs[9].wrapping_shl(2u32);
  // 22d6ee add r2, r9
  m.c.regs[2]=m.c.regs[2].wrapping_add(m.c.regs[9]);
  // 22d6f0 str r3, [r2]
  m.store32(m.c.regs[2].wrapping_add(0u32),m.c.regs[3])?;
  // 22d6f2 ldr r1, [r1]
  m.c.regs[1]=m.load32(m.c.regs[1].wrapping_add(0u32))?;
  // 22d6f4 adds r1, #0x1
  m.c.regs[1]=m.add(m.c.regs[1],1u32);
  // 22d6f6 str r1, [r0]
  m.store32(m.c.regs[0].wrapping_add(0u32),m.c.regs[1])?;
  // 22d6f8 b 0x22d708 <__ZN4Sexy9AddVertexEffjff+0x10c>
  pc=0x22d708;continue;
 },
 0x22d6fa=>{
  // 22d6fa movw r0, #0xdc3e
  m.c.regs[0]=56382u32;
  // 22d6fe movt r0, #0x13
  m.c.regs[0]=(m.c.regs[0]&0xffff)|(19u32<<16);
  // 22d702 add r0, pc
  m.c.regs[0]=m.c.regs[0].wrapping_add(0x22d706u32);
  // 22d704 ldr r1, [sp, #0xc]
  m.c.regs[1]=m.load32(m.c.regs[13].wrapping_add(12u32))?;
  // 22d706 str r1, [r0]
  m.store32(m.c.regs[0].wrapping_add(0u32),m.c.regs[1])?;
  pc=0x22d708;continue;
 },
 0x22d708=>{
  // 22d708 b 0x22d70a <__ZN4Sexy9AddVertexEffjff+0x10e>
  pc=0x22d70a;continue;
 },
 0x22d70a=>{
  // 22d70a b 0x22d70c <__ZN4Sexy9AddVertexEffjff+0x110>
  pc=0x22d70c;continue;
 },
 0x22d70c=>{
  // 22d70c movw r0, #0xdc30
  m.c.regs[0]=56368u32;
  // 22d710 movt r0, #0x13
  m.c.regs[0]=(m.c.regs[0]&0xffff)|(19u32<<16);
  // 22d714 add r0, pc
  m.c.regs[0]=m.c.regs[0].wrapping_add(0x22d718u32);
  // 22d716 movw r1, #0x115a
  m.c.regs[1]=4442u32;
  // 22d71a movt r1, #0x14
  m.c.regs[1]=(m.c.regs[1]&0xffff)|(20u32<<16);
  // 22d71e add r1, pc
  m.c.regs[1]=m.c.regs[1].wrapping_add(0x22d722u32);
  // 22d720 movw r2, #0x8cd0
  m.c.regs[2]=36048u32;
  // 22d724 movt r2, #0x15
  m.c.regs[2]=(m.c.regs[2]&0xffff)|(21u32<<16);
  // 22d728 add r2, pc
  m.c.regs[2]=m.c.regs[2].wrapping_add(0x22d72cu32);
  // 22d72a vldr s0, [sp, #20]
  m.c.extregs[0]=m.load32(m.c.regs[13].wrapping_add(20u32))?;
  // 22d72e vmov.f64 d1, d16
  let pair=[m.c.extregs[32],m.c.extregs[33]];m.c.extregs[2]=pair[0];m.c.extregs[3]=pair[1];
  // 22d732 vmov.f64 d2, d1
  let pair=[m.c.extregs[2],m.c.extregs[3]];m.c.extregs[4]=pair[0];m.c.extregs[5]=pair[1];
  // 22d736 vmov.f32 s4, s0
  m.c.extregs[4]=m.c.extregs[0];
  // 22d73a vmov.f32 d16, #8.000000e+00
  m.c.extregs[32]=0x41000000;m.c.extregs[33]=0x41000000;
  // 22d73e vmov.f64 d3, d16
  let pair=[m.c.extregs[32],m.c.extregs[33]];m.c.extregs[6]=pair[0];m.c.extregs[7]=pair[1];
  // 22d742 vmov.f32 s0, s6
  m.c.extregs[0]=m.c.extregs[6];
  // 22d746 vmov.f64 d3, d1
  let pair=[m.c.extregs[2],m.c.extregs[3]];m.c.extregs[6]=pair[0];m.c.extregs[7]=pair[1];
  // 22d74a vmov.f32 s6, s0
  m.c.extregs[6]=m.c.extregs[0];
  // 22d74e vmul.f32 d16, d2, d3
  let pair=[crate::neon_scalar_ops::mul_f32(m.c.extregs[4],m.c.extregs[6]),crate::neon_scalar_ops::mul_f32(m.c.extregs[5],m.c.extregs[7])];m.c.extregs[32]=pair[0];m.c.extregs[33]=pair[1];
  // 22d752 vmov.f64 d2, d16
  let pair=[m.c.extregs[32],m.c.extregs[33]];m.c.extregs[4]=pair[0];m.c.extregs[5]=pair[1];
  // 22d756 vmov.f32 s0, s4
  m.c.extregs[0]=m.c.extregs[4];
  // 22d75a vmov.f64 d2, d1
  let pair=[m.c.extregs[2],m.c.extregs[3]];m.c.extregs[4]=pair[0];m.c.extregs[5]=pair[1];
  // 22d75e vmov.f32 s4, s0
  m.c.extregs[4]=m.c.extregs[0];
  // 22d762 vcvt.s32.f32 d16, d2
  let pair=[crate::neon_scalar_ops::signed_from_f32(m.c.extregs[4]),crate::neon_scalar_ops::signed_from_f32(m.c.extregs[5])];m.c.extregs[32]=pair[0];m.c.extregs[33]=pair[1];
  // 22d766 vmov.f64 d2, d16
  let pair=[m.c.extregs[32],m.c.extregs[33]];m.c.extregs[4]=pair[0];m.c.extregs[5]=pair[1];
  // 22d76a vmov.f32 s0, s4
  m.c.extregs[0]=m.c.extregs[4];
  // 22d76e vmov r3, s0
  m.c.regs[3]=m.c.extregs[0];
  // 22d772 movw r9, #0x8c7e
  m.c.regs[9]=35966u32;
  // 22d776 movt r9, #0x15
  m.c.regs[9]=(m.c.regs[9]&0xffff)|(21u32<<16);
  // 22d77a add r9, pc
  m.c.regs[9]=m.c.regs[9].wrapping_add(0x22d77eu32);
  // 22d77c ldr.w r9, [r9]
  m.c.regs[9]=m.load32(m.c.regs[9].wrapping_add(0u32))?;
  // 22d780 movw r12, #0x10f0
  m.c.regs[12]=4336u32;
  // 22d784 movt r12, #0x14
  m.c.regs[12]=(m.c.regs[12]&0xffff)|(20u32<<16);
  // 22d788 add r12, pc
  m.c.regs[12]=m.c.regs[12].wrapping_add(0x22d78cu32);
  // 22d78a strh.w r3, [r12, r9, lsl #3]
  m.store16(m.c.regs[12].wrapping_add(m.c.regs[9].wrapping_shl(3)),m.c.regs[3] as u16)?;
  // 22d78e vldr s0, [sp, #16]
  m.c.extregs[0]=m.load32(m.c.regs[13].wrapping_add(16u32))?;
  // 22d792 vmov.f64 d2, d1
  let pair=[m.c.extregs[2],m.c.extregs[3]];m.c.extregs[4]=pair[0];m.c.extregs[5]=pair[1];
  // 22d796 vmov.f32 s4, s0
  m.c.extregs[4]=m.c.extregs[0];
  // 22d79a vmul.f32 d16, d2, d3
  let pair=[crate::neon_scalar_ops::mul_f32(m.c.extregs[4],m.c.extregs[6]),crate::neon_scalar_ops::mul_f32(m.c.extregs[5],m.c.extregs[7])];m.c.extregs[32]=pair[0];m.c.extregs[33]=pair[1];
  // 22d79e vmov.f64 d2, d16
  let pair=[m.c.extregs[32],m.c.extregs[33]];m.c.extregs[4]=pair[0];m.c.extregs[5]=pair[1];
  // 22d7a2 vmov.f32 s0, s4
  m.c.extregs[0]=m.c.extregs[4];
  // 22d7a6 vmov.f32 s2, s0
  m.c.extregs[2]=m.c.extregs[0];
  // 22d7aa vcvt.s32.f32 d16, d1
  let pair=[crate::neon_scalar_ops::signed_from_f32(m.c.extregs[2]),crate::neon_scalar_ops::signed_from_f32(m.c.extregs[3])];m.c.extregs[32]=pair[0];m.c.extregs[33]=pair[1];
  // 22d7ae vmov.f64 d1, d16
  let pair=[m.c.extregs[32],m.c.extregs[33]];m.c.extregs[2]=pair[0];m.c.extregs[3]=pair[1];
  // 22d7b2 vmov.f32 s0, s2
  m.c.extregs[0]=m.c.extregs[2];
  // 22d7b6 vmov r3, s0
  m.c.regs[3]=m.c.extregs[0];
  // 22d7ba ldr r2, [r2]
  m.c.regs[2]=m.load32(m.c.regs[2].wrapping_add(0u32))?;
  // 22d7bc lsls r2, r2, #0x3
  m.c.regs[2]=m.lsl(m.c.regs[2],3u32);
  // 22d7be add r1, r2
  m.c.regs[1]=m.c.regs[1].wrapping_add(m.c.regs[2]);
  // 22d7c0 strh r3, [r1, #0x2]
  m.store16(m.c.regs[1].wrapping_add(2u32),m.c.regs[3] as u16)?;
  // 22d7c2 ldrb r0, [r0]
  m.c.regs[0]=u32::from(m.load8(m.c.regs[0].wrapping_add(0u32))?);
  // 22d7c4 tst.w r0, #0x1
  m.nz(m.c.regs[0]&1u32);
  // 22d7c8 bne 0x22d8b2 <__ZN4Sexy9AddVertexEffjff+0x2b6>
  if m.c.cpsr&0x40000000==0{pc=0x22d8b2;continue;}
  pc=0x22d7ca;continue;
 },
 0x22d7ca=>{
  // 22d7ca movw r0, #0x10a6
  m.c.regs[0]=4262u32;
  // 22d7ce movt r0, #0x14
  m.c.regs[0]=(m.c.regs[0]&0xffff)|(20u32<<16);
  // 22d7d2 add r0, pc
  m.c.regs[0]=m.c.regs[0].wrapping_add(0x22d7d6u32);
  // 22d7d4 movw r1, #0x8c1c
  m.c.regs[1]=35868u32;
  // 22d7d8 movt r1, #0x15
  m.c.regs[1]=(m.c.regs[1]&0xffff)|(21u32<<16);
  // 22d7dc add r1, pc
  m.c.regs[1]=m.c.regs[1].wrapping_add(0x22d7e0u32);
  // 22d7de vldr s0, [sp, #8]
  m.c.extregs[0]=m.load32(m.c.regs[13].wrapping_add(8u32))?;
  // 22d7e2 vmov.f64 d1, d16
  let pair=[m.c.extregs[32],m.c.extregs[33]];m.c.extregs[2]=pair[0];m.c.extregs[3]=pair[1];
  // 22d7e6 vmov.f64 d2, d1
  let pair=[m.c.extregs[2],m.c.extregs[3]];m.c.extregs[4]=pair[0];m.c.extregs[5]=pair[1];
  // 22d7ea vmov.f32 s4, s0
  m.c.extregs[4]=m.c.extregs[0];
  // 22d7ee vmov.i32 d16, #0x45000000
  m.c.extregs[32]=0x45000000;m.c.extregs[33]=0x45000000;
  // 22d7f2 vmov.f64 d3, d16
  let pair=[m.c.extregs[32],m.c.extregs[33]];m.c.extregs[6]=pair[0];m.c.extregs[7]=pair[1];
  // 22d7f6 vmov.f32 s0, s6
  m.c.extregs[0]=m.c.extregs[6];
  // 22d7fa vmov.f64 d3, d1
  let pair=[m.c.extregs[2],m.c.extregs[3]];m.c.extregs[6]=pair[0];m.c.extregs[7]=pair[1];
  // 22d7fe vmov.f32 s6, s0
  m.c.extregs[6]=m.c.extregs[0];
  // 22d802 vmul.f32 d16, d2, d3
  let pair=[crate::neon_scalar_ops::mul_f32(m.c.extregs[4],m.c.extregs[6]),crate::neon_scalar_ops::mul_f32(m.c.extregs[5],m.c.extregs[7])];m.c.extregs[32]=pair[0];m.c.extregs[33]=pair[1];
  // 22d806 vmov.f64 d2, d16
  let pair=[m.c.extregs[32],m.c.extregs[33]];m.c.extregs[4]=pair[0];m.c.extregs[5]=pair[1];
  // 22d80a vmov.f32 s0, s4
  m.c.extregs[0]=m.c.extregs[4];
  // 22d80e vmov.f64 d2, d1
  let pair=[m.c.extregs[2],m.c.extregs[3]];m.c.extregs[4]=pair[0];m.c.extregs[5]=pair[1];
  // 22d812 vmov.f32 s4, s0
  m.c.extregs[4]=m.c.extregs[0];
  // 22d816 vmov.i32 d16, #0x47000000
  m.c.extregs[32]=0x47000000;m.c.extregs[33]=0x47000000;
  // 22d81a vmov.f64 d4, d16
  let pair=[m.c.extregs[32],m.c.extregs[33]];m.c.extregs[8]=pair[0];m.c.extregs[9]=pair[1];
  // 22d81e vmov.f32 s0, s8
  m.c.extregs[0]=m.c.extregs[8];
  // 22d822 vmov.f64 d4, d1
  let pair=[m.c.extregs[2],m.c.extregs[3]];m.c.extregs[8]=pair[0];m.c.extregs[9]=pair[1];
  // 22d826 vmov.f32 s8, s0
  m.c.extregs[8]=m.c.extregs[0];
  // 22d82a vsub.f32 d16, d2, d4
  let pair=[crate::neon_scalar_ops::sub_f32(m.c.extregs[4],m.c.extregs[8]),crate::neon_scalar_ops::sub_f32(m.c.extregs[5],m.c.extregs[9])];m.c.extregs[32]=pair[0];m.c.extregs[33]=pair[1];
  // 22d82e vmov.f64 d2, d16
  let pair=[m.c.extregs[32],m.c.extregs[33]];m.c.extregs[4]=pair[0];m.c.extregs[5]=pair[1];
  // 22d832 vmov.f32 s0, s4
  m.c.extregs[0]=m.c.extregs[4];
  // 22d836 vmov.f64 d2, d1
  let pair=[m.c.extregs[2],m.c.extregs[3]];m.c.extregs[4]=pair[0];m.c.extregs[5]=pair[1];
  // 22d83a vmov.f32 s4, s0
  m.c.extregs[4]=m.c.extregs[0];
  // 22d83e vcvt.s32.f32 d16, d2
  let pair=[crate::neon_scalar_ops::signed_from_f32(m.c.extregs[4]),crate::neon_scalar_ops::signed_from_f32(m.c.extregs[5])];m.c.extregs[32]=pair[0];m.c.extregs[33]=pair[1];
  // 22d842 vmov.f64 d2, d16
  let pair=[m.c.extregs[32],m.c.extregs[33]];m.c.extregs[4]=pair[0];m.c.extregs[5]=pair[1];
  // 22d846 vmov.f32 s0, s4
  m.c.extregs[0]=m.c.extregs[4];
  // 22d84a vmov r2, s0
  m.c.regs[2]=m.c.extregs[0];
  // 22d84e movw r3, #0x8ba2
  m.c.regs[3]=35746u32;
  // 22d852 movt r3, #0x15
  m.c.regs[3]=(m.c.regs[3]&0xffff)|(21u32<<16);
  // 22d856 add r3, pc
  m.c.regs[3]=m.c.regs[3].wrapping_add(0x22d85au32);
  // 22d858 ldr r3, [r3]
  m.c.regs[3]=m.load32(m.c.regs[3].wrapping_add(0u32))?;
  // 22d85a movw r9, #0x1016
  m.c.regs[9]=4118u32;
  // 22d85e movt r9, #0x14
  m.c.regs[9]=(m.c.regs[9]&0xffff)|(20u32<<16);
  // 22d862 add r9, pc
  m.c.regs[9]=m.c.regs[9].wrapping_add(0x22d866u32);
  // 22d864 add.w r3, r9, r3, lsl #3
  m.c.regs[3]=m.c.regs[9].wrapping_add(m.c.regs[3].wrapping_shl(3));
  // 22d868 strh r2, [r3, #0x4]
  m.store16(m.c.regs[3].wrapping_add(4u32),m.c.regs[2] as u16)?;
  // 22d86a vldr s0, [sp, #4]
  m.c.extregs[0]=m.load32(m.c.regs[13].wrapping_add(4u32))?;
  // 22d86e vmov.f64 d2, d1
  let pair=[m.c.extregs[2],m.c.extregs[3]];m.c.extregs[4]=pair[0];m.c.extregs[5]=pair[1];
  // 22d872 vmov.f32 s4, s0
  m.c.extregs[4]=m.c.extregs[0];
  // 22d876 vmul.f32 d16, d2, d3
  let pair=[crate::neon_scalar_ops::mul_f32(m.c.extregs[4],m.c.extregs[6]),crate::neon_scalar_ops::mul_f32(m.c.extregs[5],m.c.extregs[7])];m.c.extregs[32]=pair[0];m.c.extregs[33]=pair[1];
  // 22d87a vmov.f64 d2, d16
  let pair=[m.c.extregs[32],m.c.extregs[33]];m.c.extregs[4]=pair[0];m.c.extregs[5]=pair[1];
  // 22d87e vmov.f32 s0, s4
  m.c.extregs[0]=m.c.extregs[4];
  // 22d882 vmov.f64 d2, d1
  let pair=[m.c.extregs[2],m.c.extregs[3]];m.c.extregs[4]=pair[0];m.c.extregs[5]=pair[1];
  // 22d886 vmov.f32 s4, s0
  m.c.extregs[4]=m.c.extregs[0];
  // 22d88a vsub.f32 d16, d2, d4
  let pair=[crate::neon_scalar_ops::sub_f32(m.c.extregs[4],m.c.extregs[8]),crate::neon_scalar_ops::sub_f32(m.c.extregs[5],m.c.extregs[9])];m.c.extregs[32]=pair[0];m.c.extregs[33]=pair[1];
  // 22d88e vmov.f64 d2, d16
  let pair=[m.c.extregs[32],m.c.extregs[33]];m.c.extregs[4]=pair[0];m.c.extregs[5]=pair[1];
  // 22d892 vmov.f32 s0, s4
  m.c.extregs[0]=m.c.extregs[4];
  // 22d896 vmov.f32 s2, s0
  m.c.extregs[2]=m.c.extregs[0];
  // 22d89a vcvt.s32.f32 d16, d1
  let pair=[crate::neon_scalar_ops::signed_from_f32(m.c.extregs[2]),crate::neon_scalar_ops::signed_from_f32(m.c.extregs[3])];m.c.extregs[32]=pair[0];m.c.extregs[33]=pair[1];
  // 22d89e vmov.f64 d1, d16
  let pair=[m.c.extregs[32],m.c.extregs[33]];m.c.extregs[2]=pair[0];m.c.extregs[3]=pair[1];
  // 22d8a2 vmov.f32 s0, s2
  m.c.extregs[0]=m.c.extregs[2];
  // 22d8a6 vmov r2, s0
  m.c.regs[2]=m.c.extregs[0];
  // 22d8aa ldr r1, [r1]
  m.c.regs[1]=m.load32(m.c.regs[1].wrapping_add(0u32))?;
  // 22d8ac lsls r1, r1, #0x3
  m.c.regs[1]=m.lsl(m.c.regs[1],3u32);
  // 22d8ae add r0, r1
  m.c.regs[0]=m.c.regs[0].wrapping_add(m.c.regs[1]);
  // 22d8b0 strh r2, [r0, #0x6]
  m.store16(m.c.regs[0].wrapping_add(6u32),m.c.regs[2] as u16)?;
  pc=0x22d8b2;continue;
 },
 0x22d8b2=>{
  // 22d8b2 movw r0, #0x8b3e
  m.c.regs[0]=35646u32;
  // 22d8b6 movt r0, #0x15
  m.c.regs[0]=(m.c.regs[0]&0xffff)|(21u32<<16);
  // 22d8ba add r0, pc
  m.c.regs[0]=m.c.regs[0].wrapping_add(0x22d8beu32);
  // 22d8bc ldr r1, [r0]
  m.c.regs[1]=m.load32(m.c.regs[0].wrapping_add(0u32))?;
  // 22d8be adds r1, #0x1
  m.c.regs[1]=m.add(m.c.regs[1],1u32);
  // 22d8c0 str r1, [r0]
  m.store32(m.c.regs[0].wrapping_add(0u32),m.c.regs[1])?;
  // 22d8c2 add sp, #0x18
  m.c.regs[13]=m.c.regs[13].wrapping_add(24u32);
  // 22d8c4 bx lr
  return Some(Outcome{context:m.c,stack:m.stack,regions:m.regions,class,blocks});
 },
 _=>return None,
 }}None
}
