/* This Source Code Form is subject to the Mozilla Public License,v2.0. */
//! Private bounded whole inflate_fast transcript. No allocator/stream callbacks.
use crate::{cpu::CpuContext,mem::{Mem,ConstPtr}};
use crate::native_batch_kernel::{self as k,Machine,Outcome};
pub(crate)const ENTRY:u32=0x28b594;
pub(crate)const MASK_SLOT:u32=0x345334;
pub(crate)const MASK:u32=0x36c64c;
fn word(mem:&Mem,a:u32)->Option<u32>{(a&3==0&&mem.is_mapped_range(a,4)).then(||mem.read(ConstPtr::<u32>::from_bits(a)))}
/// Avoid paying snapshot/dispatch cost for short or immediately terminating calls.
/// This is a profitability guard, not permission to skip semantic checks.
pub(crate)fn worthwhile(mem:&Mem,c:CpuContext)->bool{
 let Some(s)=word(mem,c.regs[13])else{return false;};let Some(z)=word(mem,c.regs[13].wrapping_add(4))else{return false;};
 let Some(n)=word(mem,z.wrapping_add(4))else{return false;};if n<128{return false;}
 let(Some(read),Some(write),Some(end))=(word(mem,s.wrapping_add(48)),word(mem,s.wrapping_add(52)),word(mem,s.wrapping_add(44)))else{return false;};
 let free=if write<read{read.checked_sub(write).and_then(|n|n.checked_sub(1))}else{end.checked_sub(write)};if free.is_none_or(|n|n<2048){return false;}
 let(Some(mut bits),Some(mut hold),Some(mut input))=(word(mem,s.wrapping_add(28)),word(mem,s.wrapping_add(32)),word(mem,z))else{return false;};
 if bits>31||!(1..=10).contains(&c.regs[0]){return false;}
 for _ in 0..3{if bits>=20{break;}if !mem.is_mapped_range(input,1){return false;}hold|=u32::from(mem.read(ConstPtr::<u8>::from_bits(input)))<<bits;bits+=8;let Some(next)=input.checked_add(1)else{return false;};input=next;}
 let Some(entry)=c.regs[2].checked_add((hold&((1<<c.regs[0])-1))*8)else{return false;};
 if !mem.is_mapped_range(entry,8){return false;}let op=mem.read(ConstPtr::<u8>::from_bits(entry));if op==0{return true;}if op&16==0{return false;}
 let used=u32::from(mem.read(ConstPtr::<u8>::from_bits(entry+1)));let extra=u32::from(op&15);if used+extra>bits{return false;}hold>>=used+extra;bits-=used+extra;
 for _ in 0..3{if bits>=15{break;}if !mem.is_mapped_range(input,1){return false;}hold|=u32::from(mem.read(ConstPtr::<u8>::from_bits(input)))<<bits;bits+=8;let Some(next)=input.checked_add(1)else{return false;};input=next;}
 if !(1..=9).contains(&c.regs[1]){return false;}let Some(distance)=c.regs[3].checked_add((hold&((1<<c.regs[1])-1))*8)else{return false;};mem.is_mapped_range(distance,1)&&mem.read(ConstPtr::<u8>::from_bits(distance))&16!=0
}
pub(crate)fn snapshot(mem:&Mem,c:CpuContext,guard:(u32,u32))->Option<Machine>{
 if c.cpsr&0x0600fc00!=0||c.regs[13]&3!=0||!(1..=10).contains(&c.regs[0])||!(1..=9).contains(&c.regs[1]){return None;}
 let sp=c.regs[13];let frame=sp.checked_sub(104)?;let state=word(mem,sp)?;let z=word(mem,sp.checked_add(4)?)?;
 if !mem.is_mapped_range(state,56)||!mem.is_mapped_range(z,56)||word(mem,MASK_SLOT)?!=MASK{return None;}
 let input=word(mem,z)?;let available=word(mem,z+4)?;
 let start=word(mem,state+40)?;let end=word(mem,state+44)?;let length=end.checked_sub(start)?;
 let read=word(mem,state+48)?;let write=word(mem,state+52)?;
 if !(10..=1048576).contains(&available)||length==0||length>32768||read<start||read>end||write<start||write>=end||word(mem,state+28)?>31{return None;}
 let tl=c.regs[2];let td=c.regs[3];let tables=tl.min(td);let table_end=tl.max(td).checked_add(11520)?;let table_len=table_end.checked_sub(tables)?;
 if table_len>32768||(tl|td)&3!=0{return None;}
 let specs=[(frame,112,true),(state,56,true),(z,56,true),(start,length,true),(input,available.min(65536),false),(tables,table_len,false),(MASK_SLOT,4,false),(MASK,68,false)];
 let mut regions=Vec::with_capacity(specs.len());
 for (a,n,writable)in specs{
  a.checked_add(n)?;
  if !mem.is_mapped_range(a,n)||writable&&!k::disjoint(a,n,0x1000,0x3ff000)||guard.0!=0&&!k::disjoint(a,n,guard.0,guard.1.checked_sub(guard.0)?){return None;}
  for prior in &regions{let prior:&k::Region=prior;if(writable||prior.writable)&&!k::disjoint(a,n,prior.address,prior.bytes.len()as u32){return None;}}
  regions.push(k::region(a,mem.bytes_at(ConstPtr::<u8>::from_bits(a),n),writable));
 }
 for n in 0..17 {if word(mem,MASK+n*4)?!=if n==0{0}else{(1u32<<n)-1}{return None;}}
 Some(k::from_regions(c,regions,0))
}
pub(crate)fn calculate(m:Machine)->Option<Outcome>{let mut p=crate::inflate_fast_generated::calculate(m)?;p.class=match p.context.regs[0]{0=>0,1=>1,0xffff_fffd=>2,_=>return None};Some(p)}

/// Deterministic private cases use owned scratch, never live decoder objects.
pub(crate)fn seed(mem:&mut Mem,mut c:CpuContext,base:u32,n:u32)->CpuContext{
 use crate::mem::MutPtr;
 let put=|m:&mut Mem,a,v|m.write(MutPtr::<u32>::from_bits(a),v);
 mem.bytes_at_mut(MutPtr::<u8>::from_bits(base),262144).fill(n.wrapping_mul(73)as u8);
 let(state,z,window,input,tl,td,sp)=(base+256,base+512,base+32768,base+98304,base+196608,base+212992,base+260096);
 c.regs[0]=1;c.regs[1]=1;c.regs[2]=tl;c.regs[3]=td;c.regs[13]=sp;c.cpsr=(c.cpsr&0x0fffffff)|((n&15)<<28);
 for(a,v)in[(sp,state),(sp+4,z),(state+28,n%16),(state+32,n.wrapping_mul(0x19374091)),(state+40,window),(state+44,window+32768),(state+48,window),(state+52,window+32768-(n%600+258)),(z,input),(z+4,10+n%247),(z+8,n.wrapping_mul(1997))]{put(mem,a,v);}
 let typ=n%8;for i in 0..8u32{let(op,val)=match typ{0=>(0,65+n%26),1=>(96,0),2=>(64,0),3|4=>(16,3+n%256),5=>if i<2{(1,2-i)}else{(96,0)},6=>(17,3+n%255),_=>(0,128+n%128)};put(mem,tl+i*8,op|1<<8);put(mem,tl+i*8+4,val);put(mem,td+i*8,(if typ==4{64}else{16})|1<<8);put(mem,td+i*8+4,if n%3==0{32768}else{n%256+1});}
 for i in 0..512{mem.write(MutPtr::<u8>::from_bits(input+i),n.wrapping_mul(79).wrapping_add(i*37)as u8);}c
}
