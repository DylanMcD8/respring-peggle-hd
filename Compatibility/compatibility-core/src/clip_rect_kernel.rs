/* This Source Code Form is subject to the Mozilla Public License,v2.0. */
//! Pure integer clip closure; no retained owner, allocator or drawing callback.
use crate::{cpu::CpuContext,mem::{Mem,ConstPtr}};use crate::native_batch_kernel::{self as k,Machine,Outcome};
pub(crate)const ENTRY:u32=0x234394;
pub(crate)const LINKS:[(u32,u32,u32);4]=[(0x2d2bbc,3431140,0x16185),(0x2d39cc,3432040,0x27a71),(0x2e6aac,3451552,0x1556d1),(0x2e6acc,3451560,0x157d79)];
pub(crate)fn linked(mem:&Mem)->bool{LINKS.iter().all(|&(stub,slot,target)|mem.is_mapped_range(stub,16)&&mem.is_mapped_range(slot,4)&&mem.bytes_at(ConstPtr::<u8>::from_bits(stub),12)==[0x04,0xc0,0x9f,0xe5,0x0c,0xc0,0x8f,0xe0,0x00,0xf0,0x9c,0xe5]&&mem.read(ConstPtr::<u32>::from_bits(stub+12)).wrapping_add(stub+12)==slot&&mem.read(ConstPtr::<u32>::from_bits(slot))==target)}
pub(crate)fn snapshot(mem:&Mem,c:CpuContext,guard:(u32,u32))->Option<Machine>{if c.regs[13]&3!=0||c.cpsr&0x0600fc00!=0{return None;}let frame=c.regs[13].checked_sub(136)?;let mut regions=Vec::with_capacity(8);
 for(a,n,writable)in [(frame,136,true),(c.regs[0],16,true),(c.regs[1],16,false),(c.regs[2],16,false)].into_iter().chain(LINKS.map(|(_,slot,_)|(slot,4,false))){
  a.checked_add(n)?;
  if a&3!=0||!mem.is_mapped_range(a,n)||writable&&!k::disjoint(a,n,0x1000,0x3ff000)||guard.0!=0&&!k::disjoint(a,n,guard.0,guard.1.checked_sub(guard.0)?){return None;}
  for r in &regions{let r:&k::Region=r;if(writable||r.writable)&&!k::disjoint(a,n,r.address,r.bytes.len()as u32){return None;}}
  regions.push(k::region(a,mem.bytes_at(ConstPtr::<u8>::from_bits(a),n),writable));
 }Some(k::from_regions(c,regions,0))
}
pub(crate)fn calculate(m:Machine)->Option<Outcome>{let mut p=crate::clip_rect_generated::calculate(m)?;p.class=usize::from(p.regions[1].bytes[8..16].iter().all(|x|*x==0));Some(p)}
pub(crate)fn seed(mem:&mut Mem,mut c:CpuContext,base:u32,n:u32)->CpuContext{use crate::mem::MutPtr;let mut seed=n.wrapping_mul(7919).wrapping_add(0xc36c1);mem.bytes_at_mut(MutPtr::<u8>::from_bits(base),4096).fill(n.wrapping_mul(71)as u8);c.regs[0]=base+128;c.regs[1]=base+256;c.regs[2]=if n%7==0{base+256}else{base+512};c.regs[13]=base+3072;c.cpsr=(c.cpsr&0x0fffffff)|((n&15)<<28);for a in [c.regs[1],c.regs[2]]{for off in [0,4,8,12]{seed=seed.wrapping_mul(1664525).wrapping_add(1013904223);mem.write(MutPtr::<u32>::from_bits(a+off),if n%3==0{seed}else{seed%1024});}}c}
