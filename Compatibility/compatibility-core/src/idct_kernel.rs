/* This Source Code Form is subject to the Mozilla Public License,v2.0. */
//! Private integer inverse-DCT blocks; JPEG owners and row ordering stay original.
use crate::{cpu::CpuContext,mem::{Mem,ConstPtr,MutPtr}};use crate::native_batch_kernel::{self as k,Machine,Outcome};
pub const META:[(u32,u32,u32);4]=[(0x29b7cc,376,8),(0x29bef4,392,8),(0x29d3bc,228,4),(0x29d820,144,2)];
pub const GUARD_SLOT:u32=0x344068;
fn word(mem:&Mem,a:u32)->Option<u32>{(a&3==0&&mem.is_mapped_range(a,4)).then(||mem.read(ConstPtr::<u32>::from_bits(a)))}
pub fn snapshot(mem:&Mem,c:CpuContext,r:usize,guard:(u32,u32))->Option<Machine>{
 let&(_,frame,dim)=META.get(r)?;let sp=c.regs[13];if sp&3!=0||c.cpsr&0x0600fc00!=0{return None;}let bottom=sp.checked_sub(frame)?;
 let(info,comp,coef,rows)=(c.regs[0],c.regs[1],c.regs[2],c.regs[3]);if(info|comp|rows)&3!=0||coef&1!=0||!mem.is_mapped_range(info,328)||!mem.is_mapped_range(comp,84){return None;}
 let quant=word(mem,comp.checked_add(80)?)?;let range=word(mem,info.checked_add(324)?)?.checked_add(128)?;let canary=word(mem,GUARD_SLOT)?;let col=word(mem,sp)?;if quant&3!=0||col>16384||!mem.is_mapped_range(rows,dim*4){return None;}
 let mut specs=vec![(bottom,frame+4,true),(info,328,false),(comp,84,false),(coef,128,false),(quant,256,false),(range,1024,false),(rows,dim*4,false),(GUARD_SLOT,4,false),(canary,4,false)];
 for y in 0..dim{specs.push((word(mem,rows+y*4)?.checked_add(col)?,dim,true));}
 let mut regions=Vec::with_capacity(specs.len());
 for(a,n,writable)in specs{a.checked_add(n)?;if !mem.is_mapped_range(a,n)||writable&&!k::disjoint(a,n,0x1000,0x3ff000)||guard.0!=0&&!k::disjoint(a,n,guard.0,guard.1.checked_sub(guard.0)?){return None;}for prior in &regions{let prior:&k::Region=prior;if(writable||prior.writable)&&!k::disjoint(a,n,prior.address,prior.bytes.len()as u32){return None;}}regions.push(k::region(a,mem.bytes_at(ConstPtr::<u8>::from_bits(a),n),writable));}
 let count=regions[3].bytes[2..].chunks_exact(2).filter(|b|b[0]!=0||b[1]!=0).count();let class=if count==0{0}else if count<=8{1}else{2};Some(k::from_regions(c,regions,class))
}
pub fn calculate(m:Machine,r:usize)->Option<Outcome>{crate::idct_generated::calculate(m,r)}
pub fn seed(mem:&mut Mem,mut c:CpuContext,base:u32,r:usize,n:u32)->CpuContext{
 let put=|m:&mut Mem,a,v|m.write(MutPtr::<u32>::from_bits(a),v);mem.bytes_at_mut(MutPtr::<u8>::from_bits(base),16384).fill(n.wrapping_mul(73)as u8);
 let(info,comp,coef,quant,range,rows,out,sp)=(base+256,base+1024,base+2048,base+3072,base+4096,base+6144,base+8192,base+15360);c.regs[0]=info;c.regs[1]=comp;c.regs[2]=coef;c.regs[3]=rows;c.regs[13]=sp;c.cpsr=(c.cpsr&0x0fffffff)|((n&15)<<28);put(mem,info+324,range);put(mem,comp+80,quant);put(mem,sp,n%31);
 for i in 0..1024u32{let signed=if i<512{i as i32}else{i as i32-1024};mem.write(MutPtr::<u8>::from_bits(range+128+i),(signed+128).clamp(0,255)as u8);}
 let mut rng=n.wrapping_mul(7919).wrapping_add(0xc3717);for i in 0..64{rng=rng.wrapping_mul(1664525).wrapping_add(1013904223);let v=if i==0||n%4==2||n%4==1&&i==n%63+1||n%4==3&&i%8==0{rng as i16}else{0};mem.write(MutPtr::<u16>::from_bits(coef+i*2),v as u16);put(mem,quant+i*4,if n%5==0{rng}else{rng%255+1});}
 for y in 0..META[r].2{put(mem,rows+y*4,out+y*128);}c
}
