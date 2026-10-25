/* This Source Code Form is subject to the Mozilla Public License,v2.0. */
//! Guarded complete-image basic blocks, separate gates for each pixel format.
use crate::{Environment,cpu::CpuContext,abi::GuestFunction,mem::{ConstPtr,MutPtr}};
use crate::pixel_pack_kernel::{self as k,Outcome,ENTRIES,EXITS,SLOTS};
use crate::dyld::{export_c_func,FunctionExports};
#[derive(Default)]pub(crate)struct State{installed:[bool;3],disabled:[bool;3],original:[Option<GuestFunction>;3],host:[u32;3],prefix:[[u8;10];3],exit_prefix:[u16;3],active:Option<(usize,usize,u32)>,captured:Option<CpuContext>,corpus:[bool;3],shadows:[u64;3],calls:[u64;3],fast:[u64;3]}
fn event(class:usize,kind:&str,count:u64){crate::diagnostics::event(&format!("{{\"event\":\"pixel_pack_native\",\"class\":{class},\"kind\":\"{kind}\",\"count\":{count}}}"));}
fn go(e:&mut Environment,a:u32){e.cpu.branch(GuestFunction::from_addr_with_thumb_bit(a));}
fn tail(e:&mut Environment,class:usize){let target=e.libc_state.peggle_pixel_pack.original[class].unwrap();e.cpu.branch(target);}
fn equal(a:CpuContext,b:CpuContext)->bool{a.regs==b.regs&&a.extregs==b.extregs&&a.cpsr==b.cpsr&&a.fpscr==b.fpscr}
fn disable(e:&mut Environment,class:usize,why:&str){let s=&mut e.libc_state.peggle_pixel_pack;s.disabled[class]=true;e.mem.bytes_at_mut(MutPtr::<u8>::from_bits(ENTRIES[class]),10).copy_from_slice(&s.prefix[class]);e.mem.write(MutPtr::<u16>::from_bits(EXITS[class]),s.exit_prefix[class]);e.cpu.invalidate_cache_range(ENTRIES[class],10);e.cpu.invalidate_cache_range(EXITS[class],2);event(class,why,0);}
fn inputs(e:&Environment,mut c:CpuContext,class:usize)->Option<Outcome>{
 if class>=3{return None;}let sp=c.regs[13];if !e.mem.is_mapped_range(sp,224){return None;}
 c.regs[15]=ENTRIES[class];c.cpsr|=0x20;let frame=e.mem.bytes_at(ConstPtr::<u8>::from_bits(sp),224);let count=k::word(frame,0x64)as i32;let a=k::word(frame,0x68);let n=(count.max(0)as u32).checked_mul(4)?;
 if count>0{let image=k::word(frame,0x84);if !e.mem.is_mapped_range(image,16)||n>k::MAX_BYTES||!e.mem.is_mapped_range(a,n){return None;}
  let h=e.mem.bytes_at(ConstPtr::<u8>::from_bits(image),16);if k::word(h,12)!=a||k::word(h,4).checked_mul(k::word(h,8))?.checked_mul(4)?!=n||k::word(frame,0x98)!=class as u32+2||!k::disjoint(image,16,sp,224)||!k::disjoint(image,16,a,n){return None;}
  let s=&e.libc_state.peggle_pixel_pack;for i in 0..3{if let Some(t)=s.original[i]{if !k::disjoint(a,n,t.addr_without_thumb_bit(),20)||!k::disjoint(a,n,s.host[i],8){return None;}}}
 }
 let pixels=if n==0{&[][..]}else{e.mem.bytes_at(ConstPtr::<u8>::from_bits(a),n)};k::calculate(c,frame,pixels,class)
}
pub(crate)fn handle_exit(e:&mut Environment,svc:u32)->bool{
 if !(251..=253).contains(&svc)||e.cpu.cpsr()&0x20==0{return false;}let class=(svc-251)as usize;let s=&e.libc_state.peggle_pixel_pack;if !s.installed[class]||s.disabled[class]||e.cpu.regs()[15]!=EXITS[class]+2{return false;}
 if s.active==Some((class,e.current_thread,e.cpu.regs()[13])){let mut c=e.cpu.visible_context();c.regs[15]=EXITS[class];e.libc_state.peggle_pixel_pack.captured=Some(c);let ret=e.dyld.return_to_host_routine();e.cpu.branch(ret);}else{go(e,0x26a7d1);}true
}
fn compare(e:&mut Environment,p:Outcome,class:usize)->bool{
 let sp=p.context.regs[13];e.libc_state.peggle_pixel_pack.active=Some((class,e.current_thread,sp));e.libc_state.peggle_pixel_pack.captured=None;tail(e,class);e.run_call();let actual=e.libc_state.peggle_pixel_pack.captured.take();e.libc_state.peggle_pixel_pack.active=None;
 let ok=actual.is_some_and(|c|equal(c,p.context))&&e.mem.bytes_at(ConstPtr::<u8>::from_bits(sp),224)==p.frame&&(p.length==0||e.mem.bytes_at(ConstPtr::<u8>::from_bits(p.start),p.length)==p.buffer);
 if let Some(c)=actual{e.cpu.restore_visible_context(c);}else{panic!("pixel packing left guarded block without exit");}ok
}
fn corpus(e:&mut Environment,class:usize)->bool{
 let saved=e.cpu.visible_context();let Some(a)=e.mem.try_alloc(16384).map(|p|p.cast::<u8>())else{return false;};let base=a.to_bits();let(mut ok,mut done)=(true,0);
 for case in 0..128u32{e.mem.bytes_at_mut(a,16384).fill(0xa5);let(sp,start,image)=(base+4096,base+8192,base+256);let count=case%65;e.mem.write(MutPtr::<u32>::from_bits(sp+0x64),count);e.mem.write(MutPtr::<u32>::from_bits(sp+0x68),start);e.mem.write(MutPtr::<u32>::from_bits(sp+SLOTS[class]as u32),start);e.mem.write(MutPtr::<u32>::from_bits(sp+0x84),image);e.mem.write(MutPtr::<u32>::from_bits(sp+0x98),class as u32+2);e.mem.write(MutPtr::<u32>::from_bits(image+4),count);e.mem.write(MutPtr::<u32>::from_bits(image+8),1u32);e.mem.write(MutPtr::<u32>::from_bits(image+12),start);
  for j in 0..count{e.mem.write(MutPtr::<u32>::from_bits(start+j*4),case.wrapping_mul(0x10203041)^j.wrapping_mul(0x01030711));}
  let mut c=saved;c.regs[13]=sp;c.cpsr=(saved.cpsr&0x0fffffff)|((case&15)<<28);e.cpu.restore_visible_context(c);let Some(p)=inputs(e,c,class)else{ok=false;break;};if !compare(e,p,class){ok=false;break;}done+=1;
 }e.cpu.restore_visible_context(saved);e.mem.free(a.cast());event(class,if ok{"corpus_passed"}else{"corpus_failed"},done);ok
}
fn pack(e:&mut Environment,class:usize){let s=&e.libc_state.peggle_pixel_pack;if s.disabled[class]||s.active.is_some(){return tail(e,class);}
 let Some(p)=inputs(e,e.cpu.visible_context(),class)else{return tail(e,class);};if !s.corpus[class]{if !corpus(e,class){disable(e,class,"disabled_after_corpus_rejection");return tail(e,class);}e.libc_state.peggle_pixel_pack.corpus[class]=true;}
 let s=&mut e.libc_state.peggle_pixel_pack;s.calls[class]+=1;
 if s.shadows[class]<8||s.calls[class]%64==0{let nonempty=p.length!=0;if !compare(e,p,class){disable(e,class,"disabled_after_real_block_mismatch");}else if nonempty{let s=&mut e.libc_state.peggle_pixel_pack;s.shadows[class]+=1;if s.shadows[class]==8{event(class,"real_image_gate_passed",8);}}go(e,0x26a7d1);return;}
 // Match the original write footprint: only the compacted first half changes.
 if p.length!=0{e.mem.bytes_at_mut(MutPtr::<u8>::from_bits(p.start),p.length/2).copy_from_slice(&p.buffer[..(p.length/2)as usize]);}
 e.mem.bytes_at_mut(MutPtr::<u8>::from_bits(p.context.regs[13]),224).copy_from_slice(&p.frame);e.cpu.restore_visible_context(p.context);go(e,0x26a7d1);let s=&mut e.libc_state.peggle_pixel_pack;s.fast[class]+=1;let n=s.fast[class];if n==1||n%128==0{event(class,"fast_calls",n);}
}
fn pack4444(e:&mut Environment){pack(e,0)}fn pack565(e:&mut Environment){pack(e,1)}fn pack5551(e:&mut Environment){pack(e,2)}
const FUNCTIONS:FunctionExports=&[export_c_func!(pack4444()),export_c_func!(pack565()),export_c_func!(pack5551())];
fn trampoline(prefix:&[u8],entry:u32)->[u8;20]{let mut b=[0u8;20];b[..10].copy_from_slice(prefix);b[10..12].copy_from_slice(&0xbf00u16.to_le_bytes());b[12..16].copy_from_slice(&0xf000f8dfu32.to_le_bytes());b[16..20].copy_from_slice(&((entry+10)|1).to_le_bytes());b}
pub(crate)fn install(verified:bool,bins:&[crate::mach_o::MachO],mem:&mut crate::mem::Mem,dyld:&mut crate::dyld::Dyld,libc:&mut crate::libc::State){
 use digest::Digest;if !verified||std::env::var("PEGGLE_HD_PIXEL_PACK_NATIVE").as_deref()!=Ok("1"){return;}
 // The404byte region is disjoint from C31's premultiplication patches. The
 // full executable identity plus exact symbol and region digest guard it.
 if bins.first().and_then(|b|b.exported_symbols.get("__ZN4Sexy11SexyAppBase8GetImageERKSs11PixelFormatb"))!=Some(&0x26a3a9)||!mem.is_mapped_range(0x26a5de,404)||format!("{:x}",sha1::Sha1::digest(mem.bytes_at(ConstPtr::<u8>::from_bits(0x26a5de),404)))!="c21abb75cca1fcc908f727b64a2a9246018006eb"{event(255,"image_guard_rejected",0);return;}
 for class in 0..3{let entry=ENTRIES[class];let prefix:[u8;10]=mem.bytes_at(ConstPtr::<u8>::from_bits(entry),10).try_into().unwrap();let tr=mem.alloc(20).cast::<u8>();mem.bytes_at_mut(tr,20).copy_from_slice(&trampoline(&prefix,entry));let host=dyld.create_guest_function(mem,FUNCTIONS[class].0,FUNCTIONS[class].1);let s=&mut libc.peggle_pixel_pack;s.installed[class]=true;s.original[class]=Some(GuestFunction::from_addr_with_thumb_bit(tr.to_bits()|1));s.host[class]=host.addr_without_thumb_bit();s.prefix[class]=prefix;s.exit_prefix[class]=mem.read(ConstPtr::<u16>::from_bits(EXITS[class]));mem.write(MutPtr::<u32>::from_bits(entry),0xf004f8dfu32);mem.write(MutPtr::<u16>::from_bits(entry+4),0xbf00u16);mem.write(MutPtr::<u32>::from_bits(entry+6),host.addr_with_thumb_bit());mem.write(MutPtr::<u16>::from_bits(EXITS[class]),0xdf00u16+251+class as u16);event(class,"guarded_block_installed",0);}
}
