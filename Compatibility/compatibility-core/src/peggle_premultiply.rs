/* This Source Code Form is subject to the Mozilla Public License,v2.0. */
//! Two bounded dispatch points. Exit uses one Thumb SVC halfword, preserving
//! the separate original branch into EXIT+2. Every wrapper/owner stays original.
use crate::{Environment,cpu::CpuContext,abi::GuestFunction,mem::{ConstPtr,MutPtr}};
use crate::premultiply_kernel::{self as k,Outcome,ENTRY,EXIT};
use crate::dyld::{export_c_func,FunctionExports};
#[derive(Default)]pub(crate)struct State{installed:bool,disabled:bool,entry:Option<GuestFunction>,host:u32,prefix:[u8;10],active:Option<(usize,u32)>,captured:Option<CpuContext>,corpus:bool,shadows:u64,calls:u64,fast:u64}
fn report(kind:&str,count:u64){crate::diagnostics::event(&format!("{{\"event\":\"premultiply_native\",\"kind\":\"{kind}\",\"count\":{count}}}"));}
fn go(env:&mut Environment,a:u32){env.cpu.branch(GuestFunction::from_addr_with_thumb_bit(a));}
fn tail(env:&mut Environment){let a=env.libc_state.peggle_premultiply.entry.unwrap();env.cpu.branch(a);}
fn equal(a:CpuContext,b:CpuContext)->bool{a.regs==b.regs&&a.extregs==b.extregs&&a.cpsr==b.cpsr&&a.fpscr==b.fpscr}
fn disable(env:&mut Environment,why:&str){let s=&mut env.libc_state.peggle_premultiply;s.disabled=true;env.mem.bytes_at_mut(MutPtr::<u8>::from_bits(ENTRY),10).copy_from_slice(&s.prefix);env.mem.write(MutPtr::<u16>::from_bits(EXIT),0xe7ffu16);env.cpu.invalidate_cache_range(ENTRY,10);env.cpu.invalidate_cache_range(EXIT,2);report(why,0);}
fn inputs(env:&Environment,mut c:CpuContext)->Option<Outcome>{
 c.regs[15]=ENTRY;c.cpsr|=0x20;let sp=c.regs[13];if !env.mem.is_mapped_range(sp,k::FRAME){return None;}
 let frame=env.mem.bytes_at(ConstPtr::<u8>::from_bits(sp),k::FRAME);let(a,n)=k::span(c,frame)?;
 // Accept only a complete image at its first loop iteration. Lazy divider
 // linking or an oversized image stays original for the rest of that image.
 let image=u32::from_le_bytes(frame[0x84..0x88].try_into().ok()?);
 if !env.mem.is_mapped_range(image,16){return None;}
 let header=env.mem.bytes_at(ConstPtr::<u8>::from_bits(image),16);
 let word=|offset|u32::from_le_bytes(header[offset..offset+4].try_into().unwrap());
 let expected=word(4).checked_mul(word(8))?.checked_mul(4)?;
 if word(12)!=a||expected!=n||!k::disjoint(image,16,sp,k::FRAME)||n!=0&&!k::disjoint(image,16,a,n){return None;}
 let s=&env.libc_state.peggle_premultiply;let tramp=s.entry?.addr_without_thumb_bit();
 if n!=0&&(!env.mem.is_mapped_range(a,n)||!k::disjoint(a,n,tramp,28)||!k::disjoint(a,n,s.host,8)){return None;}
 let pixels=if n==0{&[][..]}else{env.mem.bytes_at(ConstPtr::<u8>::from_bits(a),n)};k::calculate(c,frame,pixels)
}
fn division_ready(env:&Environment)->bool{env.mem.is_mapped_range(0x2f7cbc,8)&&env.mem.get_bytes_fallible(ConstPtr::<u8>::from_bits(0x2f7cbc).cast(),8).is_some_and(|b|{let a=u32::from_le_bytes(b[..4].try_into().unwrap());a>=0xef000003&&a<0xef800000&&b[4..8]==0xe12fff1eu32.to_le_bytes()})}
/// Called before generic ARM-SVC dispatch, exclusively at the installed exit.
pub(crate)fn handle_exit(env:&mut Environment,svc:u32)->bool{
 let s=&env.libc_state.peggle_premultiply;
 if svc!=255||!s.installed||s.disabled||env.cpu.cpsr()&0x20==0||env.cpu.regs()[15]!=EXIT+2{return false;}
 if s.active==Some((env.current_thread,env.cpu.regs()[13])){
  let mut c=env.cpu.visible_context();c.regs[15]=EXIT;env.libc_state.peggle_premultiply.captured=Some(c);let ret=env.dyld.return_to_host_routine();env.cpu.branch(ret);
 }else{go(env,(EXIT+2)|1);}true
}
fn compare(env:&mut Environment,p:Outcome)->bool{
 let sp=p.context.regs[13];env.libc_state.peggle_premultiply.active=Some((env.current_thread,sp));env.libc_state.peggle_premultiply.captured=None;tail(env);env.run_call();
 let actual=env.libc_state.peggle_premultiply.captured.take();env.libc_state.peggle_premultiply.active=None;
 let ok=actual.is_some_and(|c|equal(c,p.context))&&env.mem.bytes_at(ConstPtr::<u8>::from_bits(sp),k::FRAME)==p.frame&&(p.length==0||env.mem.bytes_at(ConstPtr::<u8>::from_bits(p.start),p.length)==p.pixels);
 // The current original result is authoritative even on mismatch. Resume at
 // exactly EXIT+2; do not repeat already-mutated image pixels or the wrapper.
 if let Some(actual)=actual{env.cpu.restore_visible_context(actual);}else{panic!("premultiply reference returned outside guarded exit");}ok
}
fn corpus(env:&mut Environment)->bool{
 let saved=env.cpu.visible_context();let Some(a)=env.mem.try_alloc(16384).map(|p|p.cast::<u8>())else{return false;};let base=a.to_bits();let(mut ok,mut done)=(true,0);
 for n in 0..128u32{env.mem.bytes_at_mut(a,16384).fill(0xa5);let(sp,start,count)=(base+4096,base+8192,n%65);env.mem.write(MutPtr::<u32>::from_bits(sp+0x84),base+256);env.mem.write(MutPtr::<u32>::from_bits(base+260),count);env.mem.write(MutPtr::<u32>::from_bits(base+264),1u32);env.mem.write(MutPtr::<u32>::from_bits(base+268),start);env.mem.write(MutPtr::<u32>::from_bits(sp+0x80),start);env.mem.write(MutPtr::<u32>::from_bits(sp+0x7c),start+count*4);
  for j in 0..count{let pixel=match n%4{0=>j.wrapping_mul(0x10203451),1=>0xffffffff,2=>j*0x01010101,_=>0x80000000|j*0x010203};env.mem.write(MutPtr::<u32>::from_bits(start+j*4),pixel);}
  let mut c=saved;c.regs[13]=sp;c.cpsr=(saved.cpsr&0x0fffffff)|((n&15)<<28);env.cpu.restore_visible_context(c);
  let Some(p)=inputs(env,c)else{ok=false;break;};if !compare(env,p){ok=false;break;}done+=1;
 }env.cpu.restore_visible_context(saved);env.mem.free(a.cast());report(if ok{"corpus_passed"}else{"corpus_failed"},done);ok
}
fn timing(kind:&str,index:u64,bytes:u32,milliseconds:f64){crate::diagnostics::event(&format!("{{\"event\":\"premultiply_timing\",\"kind\":\"{kind}\",\"index\":{index},\"bytes\":{bytes},\"wall_ms_inclusive\":{milliseconds:.4},\"cpu_time\":false}}"));}
fn premultiply(env:&mut Environment){
 let s=&env.libc_state.peggle_premultiply;
 if s.disabled||s.active.is_some(){return tail(env);}
 let prediction_began=std::time::Instant::now();
 let Some(p)=inputs(env,env.cpu.visible_context())else{return tail(env);};let prediction_ms=prediction_began.elapsed().as_secs_f64()*1000.0;if !division_ready(env){return tail(env);}
 if !s.corpus{let corpus_began=std::time::Instant::now();if !corpus(env){disable(env,"disabled_after_corpus_rejection");return tail(env);}env.libc_state.peggle_premultiply.corpus=true;timing("private_corpus",128,0,corpus_began.elapsed().as_secs_f64()*1000.0);}
 let s=&mut env.libc_state.peggle_premultiply;s.calls+=1;
 if s.shadows<8||s.calls%64==0{let sample=s.calls<=8||p.length!=0&&s.shadows<8;let index=s.calls;let bytes=p.length;let reference_began=std::time::Instant::now();let nonempty=p.length!=0;let ok=compare(env,p);if sample{timing("private_prediction",index,bytes,prediction_ms);timing("real_original_validation",index,bytes,reference_began.elapsed().as_secs_f64()*1000.0);}if !ok{disable(env,"disabled_after_real_input_mismatch");}else if nonempty{let s=&mut env.libc_state.peggle_premultiply;s.shadows+=1;if s.shadows==8{report("real_image_gate_passed",8);}}go(env,(EXIT+2)|1);return;}
 let commit_began=std::time::Instant::now();let bytes=p.length;
 if p.length!=0{env.mem.bytes_at_mut(MutPtr::<u8>::from_bits(p.start),p.length).copy_from_slice(&p.pixels);}env.mem.bytes_at_mut(MutPtr::<u8>::from_bits(p.context.regs[13]),k::FRAME).copy_from_slice(&p.frame);env.cpu.restore_visible_context(p.context);go(env,(EXIT+2)|1);
 let s=&mut env.libc_state.peggle_premultiply;s.fast+=1;let n=s.fast;if n==1||n%128==0{report("fast_calls",n);}if n==1{timing("private_prediction",n,bytes,prediction_ms);timing("native_commit",n,bytes,commit_began.elapsed().as_secs_f64()*1000.0);}
}
const FUNCTIONS:FunctionExports=&[export_c_func!(premultiply())];
fn entry_trampoline(prefix:&[u8])->[u8;28]{let mut b=[0u8;28];b[..6].copy_from_slice(&prefix[..6]);b[6..8].copy_from_slice(&0xd303u16.to_le_bytes());b[8..12].copy_from_slice(&0xf000f8dfu32.to_le_bytes());b[12..16].copy_from_slice(&(EXIT|1).to_le_bytes());b[16..18].copy_from_slice(&0x20ffu16.to_le_bytes());b[18..20].copy_from_slice(&0xbf00u16.to_le_bytes());b[20..24].copy_from_slice(&0xf000f8dfu32.to_le_bytes());b[24..28].copy_from_slice(&((ENTRY+10)|1).to_le_bytes());b}
pub(crate)fn install(verified:bool,bins:&[crate::mach_o::MachO],mem:&mut crate::mem::Mem,dyld:&mut crate::dyld::Dyld,libc:&mut crate::libc::State){
 use digest::Digest;if !verified||std::env::var("PEGGLE_HD_PREMULTIPLY_NATIVE").as_deref()!=Ok("1"){return;}
 let(symbol,address,size,hash)=("__ZN4Sexy11SexyAppBase8GetImageERKSs11PixelFormatb",0x26a3a8u32,1284,"0f4e42e918781ff229d13849b3f959340aff8b17");
 if !mem.is_mapped_range(address,size){report("image_guard_rejected",0);return;}
 let body=mem.bytes_at(ConstPtr::<u8>::from_bits(address),size);
 if bins.first().and_then(|b|b.exported_symbols.get(symbol))!=Some(&(address|1))||format!("{:x}",sha1::Sha1::digest(body))!=hash||mem.read(ConstPtr::<u16>::from_bits(EXIT))!=0xe7ff{report("image_guard_rejected",0);return;}
 let prefix: [u8;10]=mem.bytes_at(ConstPtr::<u8>::from_bits(ENTRY),10).try_into().unwrap();let bytes=entry_trampoline(&prefix);let trampoline=mem.alloc(28).cast::<u8>();mem.bytes_at_mut(trampoline,28).copy_from_slice(&bytes);
 let host=dyld.create_guest_function(mem,symbol,FUNCTIONS[0].1);libc.peggle_premultiply=State{installed:true,entry:Some(GuestFunction::from_addr_with_thumb_bit(trampoline.to_bits()|1)),host:host.addr_without_thumb_bit(),prefix,..State::default()};
 mem.write(MutPtr::<u32>::from_bits(ENTRY),0xf004f8dfu32);mem.write(MutPtr::<u16>::from_bits(ENTRY+4),0xbf00u16);mem.write(MutPtr::<u32>::from_bits(ENTRY+6),host.addr_with_thumb_bit());mem.write(MutPtr::<u16>::from_bits(EXIT),0xdfffu16);report("guarded_block_installed",0);
}
