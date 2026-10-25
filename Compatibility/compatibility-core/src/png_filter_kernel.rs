/* This Source Code Form is subject to the Mozilla Public License, v. 2.0. */
//! Static native blocks for the audited PNG row reconstruction only.
//! Generated/reviewable translation; no guest instruction decoding, JIT, FP,
//! callbacks or Environment mutation. All output is private until validation.
use crate::cpu::CpuContext;
pub(crate)struct Outcome{pub(crate)context:CpuContext,pub(crate)stack:[u8;176],pub(crate)row:Vec<u8>,pub(crate)blocks:u32}
struct Machine<'a>{c:CpuContext,frame:u32,info:u32,row_address:u32,previous_address:u32,header:&'a[u8],row:Vec<u8>,previous:&'a[u8],stack:[u8;176]}
impl Machine<'_>{
 #[inline(always)]fn sw(&self,o:usize)->u32{u32::from_le_bytes(self.stack[o..o+4].try_into().unwrap())}
 #[inline(always)]fn ss(&mut self,o:usize,v:u32){self.stack[o..o+4].copy_from_slice(&v.to_le_bytes())}
 #[inline(always)]fn nz(&mut self,v:u32){self.c.cpsr=(self.c.cpsr&0x3fffffff)|(v&0x80000000)|if v==0{0x40000000}else{0};}
 #[inline(always)]fn add(&mut self,a:u32,b:u32)->u32{let r=a.wrapping_add(b);let carry=u64::from(a)+u64::from(b)>u64::from(u32::MAX);let overflow=(!(a^b)&(a^r))&0x80000000!=0;self.c.cpsr=(self.c.cpsr&0x0fffffff)|(r&0x80000000)|if r==0{0x40000000}else{0}|if carry{0x20000000}else{0}|if overflow{0x10000000}else{0};r}
 #[inline(always)]fn sub(&mut self,a:u32,b:u32)->u32{let r=a.wrapping_sub(b);let overflow=((a^b)&(a^r))&0x80000000!=0;self.c.cpsr=(self.c.cpsr&0x0fffffff)|(r&0x80000000)|if r==0{0x40000000}else{0}|if a>=b{0x20000000}else{0}|if overflow{0x10000000}else{0};r}
 #[inline(always)]fn asr(&mut self,a:u32,b:u32)->u32{let n=b&255;let(r,carry)=if n==0{(a,self.c.cpsr&0x20000000!=0)}else if n>=32{(((a as i32)>>31)as u32,a>>31!=0)}else{(((a as i32)>>n)as u32,(a>>(n-1))&1!=0)};self.c.cpsr=(self.c.cpsr&!0x20000000)|if carry{0x20000000}else{0};self.nz(r);r}
 #[inline(always)]fn load8(&self,a:u32)->Option<u8>{if let Some(o)=a.checked_sub(self.row_address){if let Some(v)=self.row.get(o as usize){return Some(*v);}}if let Some(o)=a.checked_sub(self.previous_address){if let Some(v)=self.previous.get(o as usize){return Some(*v);}}if let Some(o)=a.checked_sub(self.info){if let Some(v)=self.header.get(o as usize){return Some(*v);}}None}
 #[inline(always)]fn load32(&self,a:u32)->Option<u32>{Some(u32::from_le_bytes([self.load8(a)?,self.load8(a.checked_add(1)?)?,self.load8(a.checked_add(2)?)?,self.load8(a.checked_add(3)?)?]))}
 #[inline(always)]fn store8(&mut self,a:u32,v:u8)->Option<()>{let o=a.checked_sub(self.row_address)? as usize;*self.row.get_mut(o)?=v;Some(())}
}
fn disjoint(a:u32,n:u32,b:u32,m:u32)->bool{match(a.checked_add(n),b.checked_add(m)){(Some(ae),Some(be))=>ae<=b||be<=a,_=>false}}
pub(crate)fn calculate(c:CpuContext,filter:u32,header:&[u8],row:Vec<u8>,previous:&[u8],prior_stack:&[u8])->Option<Outcome>{
 if filter>4||header.len()!=12||prior_stack.len()!=176||row.len()!=previous.len()||row.len()>65536||c.regs[13]<176||c.cpsr&0x0600fc00!=0{return None;}
 let n=u32::from_le_bytes(header[4..8].try_into().unwrap())as usize;let bpp=(usize::from(header[11])+7)>>3;
 if n!=row.len()||header[11]==0||header[11]>64||filter!=0&&n<bpp{return None;}
 if c.regs[13]&3!=0||c.regs[1]&3!=0{return None;}
 let spans=[(c.regs[1],12),(c.regs[2],n as u32),(c.regs[3],n as u32),(c.regs[13]-176,180)];
 for i in 0..spans.len(){for j in i+1..spans.len(){if !disjoint(spans[i].0,spans[i].1,spans[j].0,spans[j].1){return None;}}}
 let frame=c.regs[13]-176;let mut stack=[0;176];stack.copy_from_slice(prior_stack);let mut m=Machine{c,frame,info:c.regs[1],row_address:c.regs[2],previous_address:c.regs[3],header,row,previous,stack};
 m.ss(168,c.regs[7]);m.ss(172,c.regs[14]);m.c.regs[7]=c.regs[13]-8;m.c.regs[9]=filter;
 m.ss(164,c.regs[0]);m.ss(160,c.regs[1]);m.ss(156,c.regs[2]);m.ss(152,c.regs[3]);m.ss(148,filter);m.c.regs[0]=filter;m.sub(filter,4);m.ss(28,filter);m.c.regs[1]=filter;
 let mut pc=match filter{0=>0x28fb14,1=>0x28f858,2=>0x28f8ac,3=>0x28f8f0,4=>0x28f9ae,_=>return None};let limit=128+n as u32*32;
 for blocks in 0..limit{match pc{
 0x28fb14=>{m.c.regs[7]=c.regs[7];return Some(Outcome{context:m.c,stack:m.stack,row:m.row,blocks});},
 0x28f858=>{
  // 28f858 ldr r0, [sp, #0xa0]
  m.c.regs[0]=m.sw(160);
  // 28f85a ldr r0, [r0, #0x4]
  m.c.regs[0]=m.load32(m.c.regs[0].wrapping_add(4))?;
  // 28f85c str r0, [sp, #0x8c]
  m.ss(140,m.c.regs[0]);
  // 28f85e ldr r0, [sp, #0xa0]
  m.c.regs[0]=m.sw(160);
  // 28f860 ldrb r0, [r0, #0xb]
  m.c.regs[0]=u32::from(m.load8(m.c.regs[0].wrapping_add(11))?);
  // 28f862 adds r0, #0x7
  m.c.regs[0]=m.add(m.c.regs[0],7u32);
  // 28f864 movs r1, #0x3
  m.c.regs[1]=3u32;
  m.nz(m.c.regs[1]);
  // 28f866 movt r1, #0x0
  m.c.regs[1]=(m.c.regs[1]&0xffff)|(0u32<<16);
  // 28f86a asrs r0, r1
  m.c.regs[0]=m.asr(m.c.regs[0],m.c.regs[1]);
  // 28f86c str r0, [sp, #0x88]
  m.ss(136,m.c.regs[0]);
  // 28f86e ldr r0, [sp, #0x9c]
  m.c.regs[0]=m.sw(156);
  // 28f870 ldr r1, [sp, #0x88]
  m.c.regs[1]=m.sw(136);
  // 28f872 add r0, r1
  m.c.regs[0]=m.c.regs[0].wrapping_add(m.c.regs[1]);
  // 28f874 str r0, [sp, #0x84]
  m.ss(132,m.c.regs[0]);
  // 28f876 ldr r0, [sp, #0x9c]
  m.c.regs[0]=m.sw(156);
  // 28f878 str r0, [sp, #0x80]
  m.ss(128,m.c.regs[0]);
  // 28f87a ldr r0, [sp, #0x88]
  m.c.regs[0]=m.sw(136);
  // 28f87c str r0, [sp, #0x90]
  m.ss(144,m.c.regs[0]);
  pc=0x28f87e;continue;
 },
 0x28f87e=>{
  // 28f87e ldr r0, [sp, #0x90]
  m.c.regs[0]=m.sw(144);
  // 28f880 ldr r1, [sp, #0x8c]
  m.c.regs[1]=m.sw(140);
  // 28f882 cmp r0, r1
  m.sub(m.c.regs[0],m.c.regs[1]);
  // 28f884 bhs 0x28f8aa <_png_read_filter_row+0x82>
  if m.c.cpsr&0x20000000!=0{pc=0x28f8aa;continue;}
  pc=0x28f886;continue;
 },
 0x28f886=>{
  // 28f886 ldr r0, [sp, #0x84]
  m.c.regs[0]=m.sw(132);
  // 28f888 ldrb r0, [r0]
  m.c.regs[0]=u32::from(m.load8(m.c.regs[0].wrapping_add(0))?);
  // 28f88a ldr r1, [sp, #0x80]
  m.c.regs[1]=m.sw(128);
  // 28f88c adds r2, r1, #0x1
  m.c.regs[2]=m.add(m.c.regs[1],1u32);
  // 28f88e str r2, [sp, #0x80]
  m.ss(128,m.c.regs[2]);
  // 28f890 ldrb r1, [r1]
  m.c.regs[1]=u32::from(m.load8(m.c.regs[1].wrapping_add(0))?);
  // 28f892 add r0, r1
  m.c.regs[0]=m.c.regs[0].wrapping_add(m.c.regs[1]);
  // 28f894 and r0, r0, #0xff
  m.c.regs[0]=m.c.regs[0]&255u32;
  // 28f898 ldr r1, [sp, #0x84]
  m.c.regs[1]=m.sw(132);
  // 28f89a strb r0, [r1]
  m.store8(m.c.regs[1].wrapping_add(0),m.c.regs[0] as u8)?;
  // 28f89c ldr r0, [sp, #0x84]
  m.c.regs[0]=m.sw(132);
  // 28f89e adds r0, #0x1
  m.c.regs[0]=m.add(m.c.regs[0],1u32);
  // 28f8a0 str r0, [sp, #0x84]
  m.ss(132,m.c.regs[0]);
  // 28f8a2 ldr r0, [sp, #0x90]
  m.c.regs[0]=m.sw(144);
  // 28f8a4 adds r0, #0x1
  m.c.regs[0]=m.add(m.c.regs[0],1u32);
  // 28f8a6 str r0, [sp, #0x90]
  m.ss(144,m.c.regs[0]);
  // 28f8a8 b 0x28f87e <_png_read_filter_row+0x56>
  pc=0x28f87e;continue;
 },
 0x28f8aa=>{
  // 28f8aa b 0x28fb14 <_png_read_filter_row+0x2ec>
  pc=0x28fb14;continue;
 },
 0x28f8ac=>{
  // 28f8ac movs r0, #0x0
  m.c.regs[0]=0u32;
  m.nz(m.c.regs[0]);
  // 28f8ae movt r0, #0x0
  m.c.regs[0]=(m.c.regs[0]&0xffff)|(0u32<<16);
  // 28f8b2 ldr r1, [sp, #0xa0]
  m.c.regs[1]=m.sw(160);
  // 28f8b4 ldr r1, [r1, #0x4]
  m.c.regs[1]=m.load32(m.c.regs[1].wrapping_add(4))?;
  // 28f8b6 str r1, [sp, #0x78]
  m.ss(120,m.c.regs[1]);
  // 28f8b8 ldr r1, [sp, #0x9c]
  m.c.regs[1]=m.sw(156);
  // 28f8ba str r1, [sp, #0x74]
  m.ss(116,m.c.regs[1]);
  // 28f8bc ldr r1, [sp, #0x98]
  m.c.regs[1]=m.sw(152);
  // 28f8be str r1, [sp, #0x70]
  m.ss(112,m.c.regs[1]);
  // 28f8c0 str r0, [sp, #0x7c]
  m.ss(124,m.c.regs[0]);
  pc=0x28f8c2;continue;
 },
 0x28f8c2=>{
  // 28f8c2 ldr r0, [sp, #0x7c]
  m.c.regs[0]=m.sw(124);
  // 28f8c4 ldr r1, [sp, #0x78]
  m.c.regs[1]=m.sw(120);
  // 28f8c6 cmp r0, r1
  m.sub(m.c.regs[0],m.c.regs[1]);
  // 28f8c8 bhs 0x28f8ee <_png_read_filter_row+0xc6>
  if m.c.cpsr&0x20000000!=0{pc=0x28f8ee;continue;}
  pc=0x28f8ca;continue;
 },
 0x28f8ca=>{
  // 28f8ca ldr r0, [sp, #0x74]
  m.c.regs[0]=m.sw(116);
  // 28f8cc ldrb r0, [r0]
  m.c.regs[0]=u32::from(m.load8(m.c.regs[0].wrapping_add(0))?);
  // 28f8ce ldr r1, [sp, #0x70]
  m.c.regs[1]=m.sw(112);
  // 28f8d0 adds r2, r1, #0x1
  m.c.regs[2]=m.add(m.c.regs[1],1u32);
  // 28f8d2 str r2, [sp, #0x70]
  m.ss(112,m.c.regs[2]);
  // 28f8d4 ldrb r1, [r1]
  m.c.regs[1]=u32::from(m.load8(m.c.regs[1].wrapping_add(0))?);
  // 28f8d6 add r0, r1
  m.c.regs[0]=m.c.regs[0].wrapping_add(m.c.regs[1]);
  // 28f8d8 and r0, r0, #0xff
  m.c.regs[0]=m.c.regs[0]&255u32;
  // 28f8dc ldr r1, [sp, #0x74]
  m.c.regs[1]=m.sw(116);
  // 28f8de strb r0, [r1]
  m.store8(m.c.regs[1].wrapping_add(0),m.c.regs[0] as u8)?;
  // 28f8e0 ldr r0, [sp, #0x74]
  m.c.regs[0]=m.sw(116);
  // 28f8e2 adds r0, #0x1
  m.c.regs[0]=m.add(m.c.regs[0],1u32);
  // 28f8e4 str r0, [sp, #0x74]
  m.ss(116,m.c.regs[0]);
  // 28f8e6 ldr r0, [sp, #0x7c]
  m.c.regs[0]=m.sw(124);
  // 28f8e8 adds r0, #0x1
  m.c.regs[0]=m.add(m.c.regs[0],1u32);
  // 28f8ea str r0, [sp, #0x7c]
  m.ss(124,m.c.regs[0]);
  // 28f8ec b 0x28f8c2 <_png_read_filter_row+0x9a>
  pc=0x28f8c2;continue;
 },
 0x28f8ee=>{
  // 28f8ee b 0x28fb14 <_png_read_filter_row+0x2ec>
  pc=0x28fb14;continue;
 },
 0x28f8f0=>{
  // 28f8f0 movs r0, #0x0
  m.c.regs[0]=0u32;
  m.nz(m.c.regs[0]);
  // 28f8f2 movt r0, #0x0
  m.c.regs[0]=(m.c.regs[0]&0xffff)|(0u32<<16);
  // 28f8f6 ldr r1, [sp, #0x9c]
  m.c.regs[1]=m.sw(156);
  // 28f8f8 str r1, [sp, #0x68]
  m.ss(104,m.c.regs[1]);
  // 28f8fa ldr r1, [sp, #0x98]
  m.c.regs[1]=m.sw(152);
  // 28f8fc str r1, [sp, #0x64]
  m.ss(100,m.c.regs[1]);
  // 28f8fe ldr r1, [sp, #0x9c]
  m.c.regs[1]=m.sw(156);
  // 28f900 str r1, [sp, #0x60]
  m.ss(96,m.c.regs[1]);
  // 28f902 ldr r1, [sp, #0xa0]
  m.c.regs[1]=m.sw(160);
  // 28f904 ldrb r1, [r1, #0xb]
  m.c.regs[1]=u32::from(m.load8(m.c.regs[1].wrapping_add(11))?);
  // 28f906 adds r1, #0x7
  m.c.regs[1]=m.add(m.c.regs[1],7u32);
  // 28f908 movs r2, #0x3
  m.c.regs[2]=3u32;
  m.nz(m.c.regs[2]);
  // 28f90a movt r2, #0x0
  m.c.regs[2]=(m.c.regs[2]&0xffff)|(0u32<<16);
  // 28f90e asrs r1, r2
  m.c.regs[1]=m.asr(m.c.regs[1],m.c.regs[2]);
  // 28f910 str r1, [sp, #0x5c]
  m.ss(92,m.c.regs[1]);
  // 28f912 ldr r1, [sp, #0xa0]
  m.c.regs[1]=m.sw(160);
  // 28f914 ldr r1, [r1, #0x4]
  m.c.regs[1]=m.load32(m.c.regs[1].wrapping_add(4))?;
  // 28f916 ldr r2, [sp, #0x5c]
  m.c.regs[2]=m.sw(92);
  // 28f918 subs r1, r1, r2
  m.c.regs[1]=m.sub(m.c.regs[1],m.c.regs[2]);
  // 28f91a str r1, [sp, #0x58]
  m.ss(88,m.c.regs[1]);
  // 28f91c str r0, [sp, #0x6c]
  m.ss(108,m.c.regs[0]);
  pc=0x28f91e;continue;
 },
 0x28f91e=>{
  // 28f91e ldr r0, [sp, #0x6c]
  m.c.regs[0]=m.sw(108);
  // 28f920 ldr r1, [sp, #0x5c]
  m.c.regs[1]=m.sw(92);
  // 28f922 cmp r0, r1
  m.sub(m.c.regs[0],m.c.regs[1]);
  // 28f924 bhs 0x28f95a <_png_read_filter_row+0x132>
  if m.c.cpsr&0x20000000!=0{pc=0x28f95a;continue;}
  pc=0x28f926;continue;
 },
 0x28f926=>{
  // 28f926 movs r1, #0x2
  m.c.regs[1]=2u32;
  m.nz(m.c.regs[1]);
  // 28f928 movt r1, #0x0
  m.c.regs[1]=(m.c.regs[1]&0xffff)|(0u32<<16);
  // 28f92c ldr r0, [sp, #0x68]
  m.c.regs[0]=m.sw(104);
  // 28f92e ldrb r0, [r0]
  m.c.regs[0]=u32::from(m.load8(m.c.regs[0].wrapping_add(0))?);
  // 28f930 ldr r2, [sp, #0x64]
  m.c.regs[2]=m.sw(100);
  // 28f932 adds r3, r2, #0x1
  m.c.regs[3]=m.add(m.c.regs[2],1u32);
  // 28f934 str r3, [sp, #0x64]
  m.ss(100,m.c.regs[3]);
  // 28f936 ldrb r2, [r2]
  m.c.regs[2]=u32::from(m.load8(m.c.regs[2].wrapping_add(0))?);
  // 28f938 str r0, [sp, #0x18]
  m.ss(24,m.c.regs[0]);
  // 28f93a mov r0, r2
  m.c.regs[0]=m.c.regs[2];
  // 28f93c blx 0x2f7bec <dyld_stub_binder+0x2f7bec>
  m.c.regs[0]/=2; // audited host __divsi3(x,2), no FP/flags/other register effects
  // 28f940 ldr r1, [sp, #0x18]
  m.c.regs[1]=m.sw(24);
  // 28f942 add r0, r1
  m.c.regs[0]=m.c.regs[0].wrapping_add(m.c.regs[1]);
  // 28f944 and r0, r0, #0xff
  m.c.regs[0]=m.c.regs[0]&255u32;
  // 28f948 ldr r1, [sp, #0x68]
  m.c.regs[1]=m.sw(104);
  // 28f94a strb r0, [r1]
  m.store8(m.c.regs[1].wrapping_add(0),m.c.regs[0] as u8)?;
  // 28f94c ldr r0, [sp, #0x68]
  m.c.regs[0]=m.sw(104);
  // 28f94e adds r0, #0x1
  m.c.regs[0]=m.add(m.c.regs[0],1u32);
  // 28f950 str r0, [sp, #0x68]
  m.ss(104,m.c.regs[0]);
  // 28f952 ldr r0, [sp, #0x6c]
  m.c.regs[0]=m.sw(108);
  // 28f954 adds r0, #0x1
  m.c.regs[0]=m.add(m.c.regs[0],1u32);
  // 28f956 str r0, [sp, #0x6c]
  m.ss(108,m.c.regs[0]);
  // 28f958 b 0x28f91e <_png_read_filter_row+0xf6>
  pc=0x28f91e;continue;
 },
 0x28f95a=>{
  // 28f95a movs r0, #0x0
  m.c.regs[0]=0u32;
  m.nz(m.c.regs[0]);
  // 28f95c movt r0, #0x0
  m.c.regs[0]=(m.c.regs[0]&0xffff)|(0u32<<16);
  // 28f960 str r0, [sp, #0x6c]
  m.ss(108,m.c.regs[0]);
  pc=0x28f962;continue;
 },
 0x28f962=>{
  // 28f962 ldr r0, [sp, #0x6c]
  m.c.regs[0]=m.sw(108);
  // 28f964 ldr r1, [sp, #0x58]
  m.c.regs[1]=m.sw(88);
  // 28f966 cmp r0, r1
  m.sub(m.c.regs[0],m.c.regs[1]);
  // 28f968 bhs 0x28f9ac <_png_read_filter_row+0x184>
  if m.c.cpsr&0x20000000!=0{pc=0x28f9ac;continue;}
  pc=0x28f96a;continue;
 },
 0x28f96a=>{
  // 28f96a movs r1, #0x2
  m.c.regs[1]=2u32;
  m.nz(m.c.regs[1]);
  // 28f96c movt r1, #0x0
  m.c.regs[1]=(m.c.regs[1]&0xffff)|(0u32<<16);
  // 28f970 ldr r0, [sp, #0x68]
  m.c.regs[0]=m.sw(104);
  // 28f972 ldrb r0, [r0]
  m.c.regs[0]=u32::from(m.load8(m.c.regs[0].wrapping_add(0))?);
  // 28f974 ldr r2, [sp, #0x64]
  m.c.regs[2]=m.sw(100);
  // 28f976 adds r3, r2, #0x1
  m.c.regs[3]=m.add(m.c.regs[2],1u32);
  // 28f978 str r3, [sp, #0x64]
  m.ss(100,m.c.regs[3]);
  // 28f97a ldrb r2, [r2]
  m.c.regs[2]=u32::from(m.load8(m.c.regs[2].wrapping_add(0))?);
  // 28f97c ldr r3, [sp, #0x60]
  m.c.regs[3]=m.sw(96);
  // 28f97e add.w r9, r3, #0x1
  m.c.regs[9]=m.c.regs[3].wrapping_add(1u32);
  // 28f982 str.w r9, [sp, #0x60]
  m.ss(96,m.c.regs[9]);
  // 28f986 ldrb r3, [r3]
  m.c.regs[3]=u32::from(m.load8(m.c.regs[3].wrapping_add(0))?);
  // 28f988 add r2, r3
  m.c.regs[2]=m.c.regs[2].wrapping_add(m.c.regs[3]);
  // 28f98a str r0, [sp, #0x14]
  m.ss(20,m.c.regs[0]);
  // 28f98c mov r0, r2
  m.c.regs[0]=m.c.regs[2];
  // 28f98e blx 0x2f7bec <dyld_stub_binder+0x2f7bec>
  m.c.regs[0]/=2; // audited host __divsi3(x,2), no FP/flags/other register effects
  // 28f992 ldr r1, [sp, #0x14]
  m.c.regs[1]=m.sw(20);
  // 28f994 add r0, r1
  m.c.regs[0]=m.c.regs[0].wrapping_add(m.c.regs[1]);
  // 28f996 and r0, r0, #0xff
  m.c.regs[0]=m.c.regs[0]&255u32;
  // 28f99a ldr r1, [sp, #0x68]
  m.c.regs[1]=m.sw(104);
  // 28f99c strb r0, [r1]
  m.store8(m.c.regs[1].wrapping_add(0),m.c.regs[0] as u8)?;
  // 28f99e ldr r0, [sp, #0x68]
  m.c.regs[0]=m.sw(104);
  // 28f9a0 adds r0, #0x1
  m.c.regs[0]=m.add(m.c.regs[0],1u32);
  // 28f9a2 str r0, [sp, #0x68]
  m.ss(104,m.c.regs[0]);
  // 28f9a4 ldr r0, [sp, #0x6c]
  m.c.regs[0]=m.sw(108);
  // 28f9a6 adds r0, #0x1
  m.c.regs[0]=m.add(m.c.regs[0],1u32);
  // 28f9a8 str r0, [sp, #0x6c]
  m.ss(108,m.c.regs[0]);
  // 28f9aa b 0x28f962 <_png_read_filter_row+0x13a>
  pc=0x28f962;continue;
 },
 0x28f9ac=>{
  // 28f9ac b 0x28fb14 <_png_read_filter_row+0x2ec>
  pc=0x28fb14;continue;
 },
 0x28f9ae=>{
  // 28f9ae movs r0, #0x0
  m.c.regs[0]=0u32;
  m.nz(m.c.regs[0]);
  // 28f9b0 movt r0, #0x0
  m.c.regs[0]=(m.c.regs[0]&0xffff)|(0u32<<16);
  // 28f9b4 ldr r1, [sp, #0x9c]
  m.c.regs[1]=m.sw(156);
  // 28f9b6 str r1, [sp, #0x50]
  m.ss(80,m.c.regs[1]);
  // 28f9b8 ldr r1, [sp, #0x98]
  m.c.regs[1]=m.sw(152);
  // 28f9ba str r1, [sp, #0x4c]
  m.ss(76,m.c.regs[1]);
  // 28f9bc ldr r1, [sp, #0x9c]
  m.c.regs[1]=m.sw(156);
  // 28f9be str r1, [sp, #0x48]
  m.ss(72,m.c.regs[1]);
  // 28f9c0 ldr r1, [sp, #0x98]
  m.c.regs[1]=m.sw(152);
  // 28f9c2 str r1, [sp, #0x44]
  m.ss(68,m.c.regs[1]);
  // 28f9c4 ldr r1, [sp, #0xa0]
  m.c.regs[1]=m.sw(160);
  // 28f9c6 ldrb r1, [r1, #0xb]
  m.c.regs[1]=u32::from(m.load8(m.c.regs[1].wrapping_add(11))?);
  // 28f9c8 adds r1, #0x7
  m.c.regs[1]=m.add(m.c.regs[1],7u32);
  // 28f9ca movs r2, #0x3
  m.c.regs[2]=3u32;
  m.nz(m.c.regs[2]);
  // 28f9cc movt r2, #0x0
  m.c.regs[2]=(m.c.regs[2]&0xffff)|(0u32<<16);
  // 28f9d0 asrs r1, r2
  m.c.regs[1]=m.asr(m.c.regs[1],m.c.regs[2]);
  // 28f9d2 str r1, [sp, #0x40]
  m.ss(64,m.c.regs[1]);
  // 28f9d4 ldr r1, [sp, #0xa0]
  m.c.regs[1]=m.sw(160);
  // 28f9d6 ldr r1, [r1, #0x4]
  m.c.regs[1]=m.load32(m.c.regs[1].wrapping_add(4))?;
  // 28f9d8 ldr r2, [sp, #0x40]
  m.c.regs[2]=m.sw(64);
  // 28f9da subs r1, r1, r2
  m.c.regs[1]=m.sub(m.c.regs[1],m.c.regs[2]);
  // 28f9dc str r1, [sp, #0x3c]
  m.ss(60,m.c.regs[1]);
  // 28f9de str r0, [sp, #0x54]
  m.ss(84,m.c.regs[0]);
  pc=0x28f9e0;continue;
 },
 0x28f9e0=>{
  // 28f9e0 ldr r0, [sp, #0x54]
  m.c.regs[0]=m.sw(84);
  // 28f9e2 ldr r1, [sp, #0x40]
  m.c.regs[1]=m.sw(64);
  // 28f9e4 cmp r0, r1
  m.sub(m.c.regs[0],m.c.regs[1]);
  // 28f9e6 bhs 0x28fa0c <_png_read_filter_row+0x1e4>
  if m.c.cpsr&0x20000000!=0{pc=0x28fa0c;continue;}
  pc=0x28f9e8;continue;
 },
 0x28f9e8=>{
  // 28f9e8 ldr r0, [sp, #0x50]
  m.c.regs[0]=m.sw(80);
  // 28f9ea ldrb r0, [r0]
  m.c.regs[0]=u32::from(m.load8(m.c.regs[0].wrapping_add(0))?);
  // 28f9ec ldr r1, [sp, #0x4c]
  m.c.regs[1]=m.sw(76);
  // 28f9ee adds r2, r1, #0x1
  m.c.regs[2]=m.add(m.c.regs[1],1u32);
  // 28f9f0 str r2, [sp, #0x4c]
  m.ss(76,m.c.regs[2]);
  // 28f9f2 ldrb r1, [r1]
  m.c.regs[1]=u32::from(m.load8(m.c.regs[1].wrapping_add(0))?);
  // 28f9f4 add r0, r1
  m.c.regs[0]=m.c.regs[0].wrapping_add(m.c.regs[1]);
  // 28f9f6 and r0, r0, #0xff
  m.c.regs[0]=m.c.regs[0]&255u32;
  // 28f9fa ldr r1, [sp, #0x50]
  m.c.regs[1]=m.sw(80);
  // 28f9fc strb r0, [r1]
  m.store8(m.c.regs[1].wrapping_add(0),m.c.regs[0] as u8)?;
  // 28f9fe ldr r0, [sp, #0x50]
  m.c.regs[0]=m.sw(80);
  // 28fa00 adds r0, #0x1
  m.c.regs[0]=m.add(m.c.regs[0],1u32);
  // 28fa02 str r0, [sp, #0x50]
  m.ss(80,m.c.regs[0]);
  // 28fa04 ldr r0, [sp, #0x54]
  m.c.regs[0]=m.sw(84);
  // 28fa06 adds r0, #0x1
  m.c.regs[0]=m.add(m.c.regs[0],1u32);
  // 28fa08 str r0, [sp, #0x54]
  m.ss(84,m.c.regs[0]);
  // 28fa0a b 0x28f9e0 <_png_read_filter_row+0x1b8>
  pc=0x28f9e0;continue;
 },
 0x28fa0c=>{
  // 28fa0c movs r0, #0x0
  m.c.regs[0]=0u32;
  m.nz(m.c.regs[0]);
  // 28fa0e movt r0, #0x0
  m.c.regs[0]=(m.c.regs[0]&0xffff)|(0u32<<16);
  // 28fa12 str r0, [sp, #0x54]
  m.ss(84,m.c.regs[0]);
  pc=0x28fa14;continue;
 },
 0x28fa14=>{
  // 28fa14 ldr r0, [sp, #0x54]
  m.c.regs[0]=m.sw(84);
  // 28fa16 ldr r1, [sp, #0x3c]
  m.c.regs[1]=m.sw(60);
  // 28fa18 cmp r0, r1
  m.sub(m.c.regs[0],m.c.regs[1]);
  // 28fa1a bhs 0x28faf8 <_png_read_filter_row+0x2d0>
  if m.c.cpsr&0x20000000!=0{pc=0x28faf8;continue;}
  pc=0x28fa1c;continue;
 },
 0x28fa1c=>{
  // 28fa1c ldr r0, [sp, #0x48]
  m.c.regs[0]=m.sw(72);
  // 28fa1e adds r1, r0, #0x1
  m.c.regs[1]=m.add(m.c.regs[0],1u32);
  // 28fa20 str r1, [sp, #0x48]
  m.ss(72,m.c.regs[1]);
  // 28fa22 ldrb r0, [r0]
  m.c.regs[0]=u32::from(m.load8(m.c.regs[0].wrapping_add(0))?);
  // 28fa24 str r0, [sp, #0x38]
  m.ss(56,m.c.regs[0]);
  // 28fa26 ldr r0, [sp, #0x4c]
  m.c.regs[0]=m.sw(76);
  // 28fa28 adds r1, r0, #0x1
  m.c.regs[1]=m.add(m.c.regs[0],1u32);
  // 28fa2a str r1, [sp, #0x4c]
  m.ss(76,m.c.regs[1]);
  // 28fa2c ldrb r0, [r0]
  m.c.regs[0]=u32::from(m.load8(m.c.regs[0].wrapping_add(0))?);
  // 28fa2e str r0, [sp, #0x34]
  m.ss(52,m.c.regs[0]);
  // 28fa30 ldr r0, [sp, #0x44]
  m.c.regs[0]=m.sw(68);
  // 28fa32 adds r1, r0, #0x1
  m.c.regs[1]=m.add(m.c.regs[0],1u32);
  // 28fa34 str r1, [sp, #0x44]
  m.ss(68,m.c.regs[1]);
  // 28fa36 ldrb r0, [r0]
  m.c.regs[0]=u32::from(m.load8(m.c.regs[0].wrapping_add(0))?);
  // 28fa38 str r0, [sp, #0x30]
  m.ss(48,m.c.regs[0]);
  // 28fa3a ldr r0, [sp, #0x34]
  m.c.regs[0]=m.sw(52);
  // 28fa3c ldr r1, [sp, #0x30]
  m.c.regs[1]=m.sw(48);
  // 28fa3e subs r0, r0, r1
  m.c.regs[0]=m.sub(m.c.regs[0],m.c.regs[1]);
  // 28fa40 str r0, [sp, #0x20]
  m.ss(32,m.c.regs[0]);
  // 28fa42 ldr r0, [sp, #0x38]
  m.c.regs[0]=m.sw(56);
  // 28fa44 ldr r1, [sp, #0x30]
  m.c.regs[1]=m.sw(48);
  // 28fa46 subs r0, r0, r1
  m.c.regs[0]=m.sub(m.c.regs[0],m.c.regs[1]);
  // 28fa48 str r0, [sp, #0x24]
  m.ss(36,m.c.regs[0]);
  // 28fa4a ldr r0, [sp, #0x20]
  m.c.regs[0]=m.sw(32);
  // 28fa4c cmp r0, #0x0
  m.sub(m.c.regs[0],0u32);
  // 28fa4e bge 0x28fa5e <_png_read_filter_row+0x236>
  if ((m.c.cpsr>>31)^((m.c.cpsr>>28)&1))==0{pc=0x28fa5e;continue;}
  pc=0x28fa50;continue;
 },
 0x28fa50=>{
  // 28fa50 movs r0, #0x0
  m.c.regs[0]=0u32;
  m.nz(m.c.regs[0]);
  // 28fa52 movt r0, #0x0
  m.c.regs[0]=(m.c.regs[0]&0xffff)|(0u32<<16);
  // 28fa56 ldr r1, [sp, #0x20]
  m.c.regs[1]=m.sw(32);
  // 28fa58 subs r0, r0, r1
  m.c.regs[0]=m.sub(m.c.regs[0],m.c.regs[1]);
  // 28fa5a str r0, [sp, #0x10]
  m.ss(16,m.c.regs[0]);
  // 28fa5c b 0x28fa62 <_png_read_filter_row+0x23a>
  pc=0x28fa62;continue;
 },
 0x28fa5e=>{
  // 28fa5e ldr r0, [sp, #0x20]
  m.c.regs[0]=m.sw(32);
  // 28fa60 str r0, [sp, #0x10]
  m.ss(16,m.c.regs[0]);
  pc=0x28fa62;continue;
 },
 0x28fa62=>{
  // 28fa62 ldr r0, [sp, #0x10]
  m.c.regs[0]=m.sw(16);
  // 28fa64 str r0, [sp, #0x2c]
  m.ss(44,m.c.regs[0]);
  // 28fa66 ldr r0, [sp, #0x24]
  m.c.regs[0]=m.sw(36);
  // 28fa68 cmp r0, #0x0
  m.sub(m.c.regs[0],0u32);
  // 28fa6a bge 0x28fa7a <_png_read_filter_row+0x252>
  if ((m.c.cpsr>>31)^((m.c.cpsr>>28)&1))==0{pc=0x28fa7a;continue;}
  pc=0x28fa6c;continue;
 },
 0x28fa6c=>{
  // 28fa6c movs r0, #0x0
  m.c.regs[0]=0u32;
  m.nz(m.c.regs[0]);
  // 28fa6e movt r0, #0x0
  m.c.regs[0]=(m.c.regs[0]&0xffff)|(0u32<<16);
  // 28fa72 ldr r1, [sp, #0x24]
  m.c.regs[1]=m.sw(36);
  // 28fa74 subs r0, r0, r1
  m.c.regs[0]=m.sub(m.c.regs[0],m.c.regs[1]);
  // 28fa76 str r0, [sp, #0xc]
  m.ss(12,m.c.regs[0]);
  // 28fa78 b 0x28fa7e <_png_read_filter_row+0x256>
  pc=0x28fa7e;continue;
 },
 0x28fa7a=>{
  // 28fa7a ldr r0, [sp, #0x24]
  m.c.regs[0]=m.sw(36);
  // 28fa7c str r0, [sp, #0xc]
  m.ss(12,m.c.regs[0]);
  pc=0x28fa7e;continue;
 },
 0x28fa7e=>{
  // 28fa7e ldr r0, [sp, #0xc]
  m.c.regs[0]=m.sw(12);
  // 28fa80 str r0, [sp, #0x28]
  m.ss(40,m.c.regs[0]);
  // 28fa82 ldr r0, [sp, #0x20]
  m.c.regs[0]=m.sw(32);
  // 28fa84 ldr r1, [sp, #0x24]
  m.c.regs[1]=m.sw(36);
  // 28fa86 add r0, r1
  m.c.regs[0]=m.c.regs[0].wrapping_add(m.c.regs[1]);
  // 28fa88 cmp r0, #0x0
  m.sub(m.c.regs[0],0u32);
  // 28fa8a bge 0x28fa9e <_png_read_filter_row+0x276>
  if ((m.c.cpsr>>31)^((m.c.cpsr>>28)&1))==0{pc=0x28fa9e;continue;}
  pc=0x28fa8c;continue;
 },
 0x28fa8c=>{
  // 28fa8c movs r0, #0x0
  m.c.regs[0]=0u32;
  m.nz(m.c.regs[0]);
  // 28fa8e movt r0, #0x0
  m.c.regs[0]=(m.c.regs[0]&0xffff)|(0u32<<16);
  // 28fa92 ldr r1, [sp, #0x20]
  m.c.regs[1]=m.sw(32);
  // 28fa94 ldr r2, [sp, #0x24]
  m.c.regs[2]=m.sw(36);
  // 28fa96 add r1, r2
  m.c.regs[1]=m.c.regs[1].wrapping_add(m.c.regs[2]);
  // 28fa98 subs r0, r0, r1
  m.c.regs[0]=m.sub(m.c.regs[0],m.c.regs[1]);
  // 28fa9a str r0, [sp, #0x8]
  m.ss(8,m.c.regs[0]);
  // 28fa9c b 0x28faa6 <_png_read_filter_row+0x27e>
  pc=0x28faa6;continue;
 },
 0x28fa9e=>{
  // 28fa9e ldr r0, [sp, #0x20]
  m.c.regs[0]=m.sw(32);
  // 28faa0 ldr r1, [sp, #0x24]
  m.c.regs[1]=m.sw(36);
  // 28faa2 add r0, r1
  m.c.regs[0]=m.c.regs[0].wrapping_add(m.c.regs[1]);
  // 28faa4 str r0, [sp, #0x8]
  m.ss(8,m.c.regs[0]);
  pc=0x28faa6;continue;
 },
 0x28faa6=>{
  // 28faa6 ldr r0, [sp, #0x8]
  m.c.regs[0]=m.sw(8);
  // 28faa8 str r0, [sp, #0x24]
  m.ss(36,m.c.regs[0]);
  // 28faaa ldr r0, [sp, #0x2c]
  m.c.regs[0]=m.sw(44);
  // 28faac ldr r1, [sp, #0x28]
  m.c.regs[1]=m.sw(40);
  // 28faae cmp r0, r1
  m.sub(m.c.regs[0],m.c.regs[1]);
  // 28fab0 bgt 0x28fac0 <_png_read_filter_row+0x298>
  if m.c.cpsr&0x40000000==0&&((m.c.cpsr>>31)^((m.c.cpsr>>28)&1))==0{pc=0x28fac0;continue;}
  pc=0x28fab2;continue;
 },
 0x28fab2=>{
  // 28fab2 ldr r0, [sp, #0x2c]
  m.c.regs[0]=m.sw(44);
  // 28fab4 ldr r1, [sp, #0x24]
  m.c.regs[1]=m.sw(36);
  // 28fab6 cmp r0, r1
  m.sub(m.c.regs[0],m.c.regs[1]);
  // 28fab8 bgt 0x28fac0 <_png_read_filter_row+0x298>
  if m.c.cpsr&0x40000000==0&&((m.c.cpsr>>31)^((m.c.cpsr>>28)&1))==0{pc=0x28fac0;continue;}
  pc=0x28faba;continue;
 },
 0x28faba=>{
  // 28faba ldr r0, [sp, #0x38]
  m.c.regs[0]=m.sw(56);
  // 28fabc str r0, [sp, #0x4]
  m.ss(4,m.c.regs[0]);
  // 28fabe b 0x28fad6 <_png_read_filter_row+0x2ae>
  pc=0x28fad6;continue;
 },
 0x28fac0=>{
  // 28fac0 ldr r0, [sp, #0x28]
  m.c.regs[0]=m.sw(40);
  // 28fac2 ldr r1, [sp, #0x24]
  m.c.regs[1]=m.sw(36);
  // 28fac4 cmp r0, r1
  m.sub(m.c.regs[0],m.c.regs[1]);
  // 28fac6 bgt 0x28face <_png_read_filter_row+0x2a6>
  if m.c.cpsr&0x40000000==0&&((m.c.cpsr>>31)^((m.c.cpsr>>28)&1))==0{pc=0x28face;continue;}
  pc=0x28fac8;continue;
 },
 0x28fac8=>{
  // 28fac8 ldr r0, [sp, #0x34]
  m.c.regs[0]=m.sw(52);
  // 28faca str r0, [sp]
  m.ss(0,m.c.regs[0]);
  // 28facc b 0x28fad2 <_png_read_filter_row+0x2aa>
  pc=0x28fad2;continue;
 },
 0x28face=>{
  // 28face ldr r0, [sp, #0x30]
  m.c.regs[0]=m.sw(48);
  // 28fad0 str r0, [sp]
  m.ss(0,m.c.regs[0]);
  pc=0x28fad2;continue;
 },
 0x28fad2=>{
  // 28fad2 ldr r0, [sp]
  m.c.regs[0]=m.sw(0);
  // 28fad4 str r0, [sp, #0x4]
  m.ss(4,m.c.regs[0]);
  pc=0x28fad6;continue;
 },
 0x28fad6=>{
  // 28fad6 ldr r0, [sp, #0x4]
  m.c.regs[0]=m.sw(4);
  // 28fad8 str r0, [sp, #0x20]
  m.ss(32,m.c.regs[0]);
  // 28fada ldr r0, [sp, #0x50]
  m.c.regs[0]=m.sw(80);
  // 28fadc ldrb r0, [r0]
  m.c.regs[0]=u32::from(m.load8(m.c.regs[0].wrapping_add(0))?);
  // 28fade ldr r1, [sp, #0x20]
  m.c.regs[1]=m.sw(32);
  // 28fae0 add r0, r1
  m.c.regs[0]=m.c.regs[0].wrapping_add(m.c.regs[1]);
  // 28fae2 and r0, r0, #0xff
  m.c.regs[0]=m.c.regs[0]&255u32;
  // 28fae6 ldr r1, [sp, #0x50]
  m.c.regs[1]=m.sw(80);
  // 28fae8 strb r0, [r1]
  m.store8(m.c.regs[1].wrapping_add(0),m.c.regs[0] as u8)?;
  // 28faea ldr r0, [sp, #0x50]
  m.c.regs[0]=m.sw(80);
  // 28faec adds r0, #0x1
  m.c.regs[0]=m.add(m.c.regs[0],1u32);
  // 28faee str r0, [sp, #0x50]
  m.ss(80,m.c.regs[0]);
  // 28faf0 ldr r0, [sp, #0x54]
  m.c.regs[0]=m.sw(84);
  // 28faf2 adds r0, #0x1
  m.c.regs[0]=m.add(m.c.regs[0],1u32);
  // 28faf4 str r0, [sp, #0x54]
  m.ss(84,m.c.regs[0]);
  // 28faf6 b 0x28fa14 <_png_read_filter_row+0x1ec>
  pc=0x28fa14;continue;
 },
 0x28faf8=>{
  // 28faf8 b 0x28fb14 <_png_read_filter_row+0x2ec>
  pc=0x28fb14;continue;
 },
 _=>return None,
 }}None
}
