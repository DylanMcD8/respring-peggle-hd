/* This Source Code Form is subject to the Mozilla Public License,v2.0. */
//! Bounded private transcripts for C35. Original owners/callbacks stay original.
use crate::{cpu::CpuContext,mem::{Mem,ConstPtr}};
#[derive(Clone)]pub(crate)struct Bytes{inline:[u8;96],large:Vec<u8>,len:usize}
impl Bytes{fn from_slice(b:&[u8])->Self{let mut s=Self{inline:[0;96],large:Vec::new(),len:b.len()};if b.len()<=96{s.inline[..b.len()].copy_from_slice(b);}else{s.large=b.to_vec();}s}}
impl std::ops::Deref for Bytes{type Target=[u8];fn deref(&self)->&[u8]{if self.len<=96{&self.inline[..self.len]}else{&self.large}}}
impl std::ops::DerefMut for Bytes{fn deref_mut(&mut self)->&mut[u8]{if self.len<=96{&mut self.inline[..self.len]}else{&mut self.large}}}
struct Specs{items:[(u32,u32,bool);12],len:usize}
impl Specs{fn new()->Self{Self{items:[(0,0,false);12],len:0}}fn push(&mut self,v:(u32,u32,bool)){self.items[self.len]=v;self.len+=1;}fn extend<const N:usize>(&mut self,v:[(u32,u32,bool);N]){for x in v{self.push(x);}}}
#[derive(Clone)]pub(crate)struct Region{pub address:u32,pub bytes:Bytes,pub writable:bool}
pub(crate)struct Outcome{pub context:CpuContext,pub regions:Vec<Region>,pub class:usize}
pub(crate)struct Machine{pub c:CpuContext,pub regions:Vec<Region>,saved_pc:u32,saved_lr:u32,pub class:usize}
pub(crate)fn disjoint(a:u32,n:u32,b:u32,m:u32)->bool{match(a.checked_add(n),b.checked_add(m)){(Some(x),Some(y))=>x<=b||y<=a,_=>false}}
impl Machine{
 #[inline(always)]pub(crate)fn read(&self,a:u32,n:u32)->Option<u32>{let end=a.checked_add(n)?;for r in &self.regions{if r.address<=a&&end<=r.address+r.bytes.len()as u32{let mut b=[0u8;4];b[..n as usize].copy_from_slice(&r.bytes[(a-r.address)as usize..(end-r.address)as usize]);return Some(u32::from_le_bytes(b));}}#[cfg(test)]eprintln!("READ_REJECT a={a:x} n={n}");None}
 #[inline(always)]pub(crate)fn write(&mut self,a:u32,n:u32,value:u32)->Option<()>{let end=a.checked_add(n)?;for r in &mut self.regions{if r.writable&&r.address<=a&&end<=r.address+r.bytes.len()as u32{r.bytes[(a-r.address)as usize..(end-r.address)as usize].copy_from_slice(&value.to_le_bytes()[..n as usize]);return Some(());}}#[cfg(test)]eprintln!("WRITE_REJECT a={a:x} n={n}");None}
 pub(crate)fn nz(&mut self,v:u32){self.c.cpsr=(self.c.cpsr&0x3fffffff)|(v&0x80000000)|if v==0{0x40000000}else{0};}
 pub(crate)fn cmp(&mut self,a:u32,b:u32){self.sub_flags(a,b)}
 pub(crate)fn sub_flags(&mut self,a:u32,b:u32){let v=a.wrapping_sub(b);self.c.cpsr=(self.c.cpsr&0x0fffffff)|(v&0x80000000)|if v==0{0x40000000}else{0}|if a>=b{0x20000000}else{0}|if((a^b)&(a^v))>>31!=0{0x10000000}else{0};}
 pub(crate)fn add_flags(&mut self,a:u32,b:u32){let(v,carry)=a.overflowing_add(b);self.c.cpsr=(self.c.cpsr&0x0fffffff)|(v&0x80000000)|if v==0{0x40000000}else{0}|if carry{0x20000000}else{0}|if(!(a^b)&(a^v))>>31!=0{0x10000000}else{0};}
 pub(crate)fn shift(&mut self,a:u32,n:u32,right:bool)->u32{let n=n&255;let(v,carry)=if n==0{(a,self.c.cpsr>>29&1)}else if n<=32{if right{(if n==32{0}else{a>>n},a>>(n-1)&1)}else{(if n==32{0}else{a<<n},a>>(32-n)&1)}}else{(0,0)};self.nz(v);self.c.cpsr=(self.c.cpsr&!0x20000000)|(carry<<29);v}
 pub(crate)fn cond(&self,s:&str)->bool{let(n,z,c,v)=(self.c.cpsr>>31,self.c.cpsr>>30&1,self.c.cpsr>>29&1,self.c.cpsr>>28&1);match s{"eq"=>z==1,"ne"=>z==0,"hs"=>c==1,"hi"=>c==1&&z==0,"ls"=>c==0||z==1,"lo"=>c==0,"ge"=>n==v,"lt"=>n!=v,"gt"=>z==0&&n==v,"le"=>z==1||n!=v,_=>false}}
 pub(crate)fn divide(&mut self,signed:bool)->Option<()>{let(a,b)=(self.c.regs[0],self.c.regs[1]);if b==0{return None;}self.c.regs[0]=if signed{(a as i32).wrapping_div(b as i32)as u32}else{a/b};Some(())}
 pub(crate)fn finish(mut self)->Outcome{self.c.regs[15]=self.saved_pc;self.c.regs[14]=self.saved_lr;Outcome{context:self.c,regions:self.regions,class:self.class}}
}
#[inline(never)]pub(crate)fn fp_mul(a:u32,b:u32)->Option<u32>{fp((f32::from_bits(a)*f32::from_bits(b)).to_bits())}
#[inline(never)]pub(crate)fn fp_div(a:u32,b:u32)->Option<u32>{fp((f32::from_bits(a)/f32::from_bits(b)).to_bits())}
fn fp(a:u32)->Option<u32>{let x=f32::from_bits(a);(x.is_finite()&&(x==0.0||x.is_normal())).then_some(a)}
fn word(m:&Mem,a:u32)->Option<u32>{(a&3==0&&m.is_mapped_range(a,4)).then(||m.read(ConstPtr::<u32>::from_bits(a)))}
pub(crate)fn snapshot(mem:&Mem,c:CpuContext,r:usize)->Option<Machine>{
 if r>=15||c.cpsr&0x0600fc00!=0||c.regs[13]&3!=0{return None;}let sp=c.regs[13];let len=[20,44,28,20,8,8,8,96,28,60,60,76,24,20,16][r];let base=sp.checked_sub(len)?;sp.checked_add(4)?;let mut specs=Specs::new();specs.push((base,len+if r==0{4}else{0},true));let mut class=0;
 match r{
  0=>{let image=c.regs[0];if !mem.is_mapped_range(image,68)||c.fpscr&0x07c00000!=0{return None;}let image_data=mem.bytes_at(ConstPtr::<u8>::from_bits(image),68);let image_word=|offset:usize|u32::from_le_bytes(image_data[offset..offset+4].try_into().unwrap());for offset in [28,32]{fp(image_word(offset))?;}for offset in [60,64]{let n=image_word(offset)as i32;if n<=0||n>16384{return None;}}let y=word(mem,sp)?;if (image|c.regs[3]|y)&3!=0{return None;}specs.extend([(image,68,false),(c.regs[3],4,true),(y,4,true)]);}
  1=>{class=usize::from(c.regs[0]>>24==0);}
  2=>specs.push((c.regs[0],16,false)),
  3=>{let p=c.regs[0];if !mem.is_mapped_range(p,50)||mem.read(ConstPtr::<u8>::from_bits(p+49))&1==0{return None;}specs.push((p,50,false));}
  4..=6=>specs.extend([(c.regs[0],4,false),(c.regs[1],4,false)]),
  7..=13=>{let p=c.regs[0];if !mem.is_mapped_range(p,16){return None;}let header=mem.bytes_at(ConstPtr::<u8>::from_bits(p),16);let width=u32::from_le_bytes(header[0..4].try_into().ok()?);let bytes=u32::from_le_bytes(header[4..8].try_into().ok()?);let color=header[8];let depth=header[9];let channels=header[10];let expected=match color{0=>1,2=>3,4=>2,6=>4,_=>return None};if width==0||width>4096||!matches!(depth,8|16)||channels!=expected||header[11]!=channels*depth||bytes!=width*u32::from(channels)*u32::from(depth/8){return None;}if r==7&&depth==16{return None;}let cap=if r==7&&matches!(color,0|2){width*u32::from(channels+1)*u32::from(depth/8)}else{bytes};class=usize::from(color/2)+usize::from(depth==16)*4+usize::from((if r==7{c.regs[3]}else{c.regs[2]})&128!=0)*8;specs.extend([(p,16,matches!(r,7|8)),(c.regs[1],cap,true)]);}
  14=>{for pair in [c.regs[0],c.regs[1]]{specs.push((pair,8,false));for off in [0,4]{let obj=word(mem,pair+off)?;specs.push((obj,96,false));}}let a=word(mem,c.regs[0])?;let b=word(mem,c.regs[1])?;class=if a!=b{0}else if word(mem,word(mem,c.regs[0]+4)?+16)?!=word(mem,word(mem,c.regs[1]+4)?+16)?{1}else{2};}
  _=>return None
 }
 let mut regions=Vec::<Region>::with_capacity(specs.len);for (a,n,writable)in specs.items.into_iter().take(specs.len){if n==0||!mem.is_mapped_range(a,n)||!disjoint(a,n,0x1000,0x3ff000){return None;}for prior in &regions{if(writable||prior.writable)&&!disjoint(a,n,prior.address,prior.bytes.len()as u32){return None;}}regions.push(Region{address:a,bytes:Bytes::from_slice(mem.bytes_at(ConstPtr::<u8>::from_bits(a),n)),writable});}
 Some(Machine{c,regions,saved_pc:c.regs[15],saved_lr:c.regs[14],class})
}
/// Allocation-free per-blit coordinate contract after mandatory original gates.
/// Same snapshot bounds/aliases/FP domain as the private transcript.
pub(crate)struct Coordinates{pub context:CpuContext,pub frame_address:u32,pub frame:[u8;20],pub x:u32,pub y:u32,pub vx:u32,pub vy:u32}
pub(crate)fn coordinates(mem:&Mem,mut c:CpuContext,guard:(u32,u32))->Option<Coordinates>{
 if c.cpsr&0x0600fc00!=0||c.fpscr&0x07c00000!=0||c.regs[13]&3!=0{return None;}let sp=c.regs[13];let frame_address=sp.checked_sub(20)?;let image=c.regs[0];if image&3!=0||!mem.is_mapped_range(image,68)||!mem.is_mapped_range(frame_address,24){return None;}let y=mem.read(ConstPtr::<u32>::from_bits(sp));let x=c.regs[3];let spans=[(frame_address,24),(image,68),(x,4),(y,4)];for(i,&(a,n))in spans.iter().enumerate(){if a&3!=0||!mem.is_mapped_range(a,n)||!disjoint(a,n,0x1000,0x3ff000)||guard.0!=0&&!disjoint(a,n,guard.0,guard.1.checked_sub(guard.0)?){return None;}for&(b,m)in &spans[..i]{if !disjoint(a,n,b,m){return None;}}}
 let b=mem.bytes_at(ConstPtr::<u8>::from_bits(image),68);let w=|o:usize|u32::from_le_bytes(b[o..o+4].try_into().unwrap());let(dx,dy,sx,sy,ox,oy)=(w(60),w(64),w(28),w(32),w(36),w(40));if dx as i32<=0||dy as i32<=0||dx>16384||dy>16384{return None;}fp(sx)?;fp(sy)?;let vx=fp_mul(fp_div((c.regs[1].wrapping_add(ox)as i32 as f32).to_bits(),(dx as f32).to_bits())?,sx)?;let vy=fp_mul(fp_div((c.regs[2].wrapping_add(oy)as i32 as f32).to_bits(),(dy as f32).to_bits())?,sy)?;let mut frame=[0u8;20];for(o,v)in[(0,y),(4,x),(8,c.regs[2]),(12,c.regs[1]),(16,image)]{frame[o..o+4].copy_from_slice(&v.to_le_bytes());}c.regs[0]=y;c.regs[1]=dy;c.regs[2]=oy;c.regs[9]=y;c.extregs[0]=vy;c.extregs[4]=sy;Some(Coordinates{context:c,frame_address,frame,x,y,vx,vy})
}

/// C36 bounded transcript factory. Callers establish snapshot/domain/alias guards.
pub(crate)fn from_regions(c:CpuContext,regions:Vec<Region>,class:usize)->Machine{Machine{c,regions,saved_pc:c.regs[15],saved_lr:c.regs[14],class}}
pub(crate)fn region(address:u32,bytes:&[u8],writable:bool)->Region{Region{address,bytes:Bytes::from_slice(bytes),writable}}
