/* This Source Code Form is subject to the Mozilla Public License,v2.0. */
//! Four exact bounded integer JPEG block transforms; original owners retained.
use crate::{Environment,cpu::CpuContext,abi::GuestFunction,mem::{ConstPtr,MutPtr}};
use crate::dyld::{export_c_func,FunctionExports};use crate::idct_kernel as k;
#[derive(Default)]struct Hook{original:Option<GuestFunction>,prefix:[u8;10],disabled:bool,validating:bool,corpus:bool,total:u64,shadows:[u64;3],calls:[u64;3],fast:[u64;3]}
#[derive(Default)]pub(crate)struct State{hooks:[Hook;4]}
fn event(r:usize,class:usize,kind:&str,count:u64){crate::diagnostics::event(&format!("{{\"event\":\"jpeg_idct_native\",\"routine\":{r},\"class\":{class},\"kind\":\"{kind}\",\"count\":{count}}}"));}
fn original(e:&mut Environment,r:usize){let(sp,lr)=(e.cpu.regs()[13],e.cpu.regs()[14]);e.libc_state.peggle_jpeg_idct.hooks[r].original.unwrap().call_without_pushing_stack_frame(e);e.mem.write(MutPtr::<u32>::from_bits(sp-4),lr);}
fn equal(a:CpuContext,b:CpuContext)->bool{a.regs==b.regs&&a.extregs==b.extregs&&a.cpsr==b.cpsr&&a.fpscr==b.fpscr}
fn predict(e:&Environment,r:usize)->Option<crate::native_batch_kernel::Outcome>{k::calculate(k::snapshot(&e.mem,e.cpu.visible_context(),r,e.libc_state.peggle_native_batch.code_guard())?,r)}
fn compare(e:&mut Environment,r:usize,p:crate::native_batch_kernel::Outcome)->bool{original(e,r);equal(e.cpu.visible_context(),p.context)&&p.regions.iter().all(|region|e.mem.bytes_at(ConstPtr::<u8>::from_bits(region.address),region.bytes.len()as u32)==&region.bytes[..])}
fn disable(e:&mut Environment,r:usize,why:&str){let s=&mut e.libc_state.peggle_jpeg_idct.hooks[r];s.disabled=true;let prefix=s.prefix;let entry=k::META[r].0;e.mem.bytes_at_mut(MutPtr::<u8>::from_bits(entry),10).copy_from_slice(&prefix);e.cpu.invalidate_cache_range(entry,10);event(r,255,why,0);}
fn corpus(e:&mut Environment,r:usize)->bool{let saved=e.cpu.visible_context();let Some(a)=e.mem.try_alloc(16384).map(|p|p.cast::<u8>())else{return false;};e.libc_state.peggle_jpeg_idct.hooks[r].validating=true;let mut passed=0;
 for n in 0..128{let c=k::seed(&mut e.mem,saved,a.to_bits(),r,n);e.cpu.restore_visible_context(c);let Some(p)=predict(e,r)else{break;};if !compare(e,r,p){break;}passed+=1;}
 e.cpu.restore_visible_context(saved);e.mem.free(a.cast());e.libc_state.peggle_jpeg_idct.hooks[r].validating=false;event(r,255,if passed==128{"corpus_passed"}else{"corpus_failed"},passed);passed==128
}
fn run(e:&mut Environment,r:usize){let s=&e.libc_state.peggle_jpeg_idct.hooks[r];if s.disabled||s.validating{return original(e,r);}let Some(p)=predict(e,r)else{return original(e,r);};let class=p.class;
 if !s.corpus{if !corpus(e,r){disable(e,r,"disabled_after_corpus_failure");return original(e,r);}e.libc_state.peggle_jpeg_idct.hooks[r].corpus=true;}
 let s=&mut e.libc_state.peggle_jpeg_idct.hooks[r];s.calls[class]+=1;
 if s.total<128||s.shadows[class]<16||s.calls[class]%1024==0{if !compare(e,r,p){disable(e,r,"disabled_after_real_mismatch");}else{let s=&mut e.libc_state.peggle_jpeg_idct.hooks[r];s.total+=1;s.shadows[class]+=1;if s.total==128{event(r,255,"real_global_gate_passed",128);}if s.shadows[class]==16{event(r,class,"real_class_gate_passed",16);}}return;}
 for region in &p.regions{if region.writable{e.mem.bytes_at_mut(MutPtr::<u8>::from_bits(region.address),region.bytes.len()as u32).copy_from_slice(&region.bytes);}}e.cpu.restore_visible_context(p.context);let s=&mut e.libc_state.peggle_jpeg_idct.hooks[r];s.fast[class]+=1;let n=s.fast[class];if n==1||n%65536==0{event(r,class,"fast_calls",n);}
}
fn idct_0(e:&mut Environment){run(e,0)}fn idct_1(e:&mut Environment){run(e,1)}fn idct_2(e:&mut Environment){run(e,2)}fn idct_3(e:&mut Environment){run(e,3)}
const FUNCTIONS:FunctionExports=&[export_c_func!(idct_0()),export_c_func!(idct_1()),export_c_func!(idct_2()),export_c_func!(idct_3())];
pub const GUARDS:[(&str,u32,&str);4]=[("_jpeg_idct_islow",1832,"13876b07ba91a3c40147f55f839a4802078a333f"),("_jpeg_idct_ifast",1532,"3818e94660cb2d3f87c05226c81f215568b72876"),("_jpeg_idct_4x4",1124,"6f3b0d21b7d70b12cf90f625a6cc89e3eb01fbeb"),("_jpeg_idct_2x2",712,"4f822f0f764946dde93d8400b23fe0941e56cbc2")];
pub(crate)fn install(verified:bool,bins:&[crate::mach_o::MachO],mem:&mut crate::mem::Mem,dyld:&mut crate::dyld::Dyld,libc:&mut crate::libc::State){use digest::Digest;if !verified||std::env::var("PEGGLE_HD_JPEG_IDCT_NATIVE").as_deref()!=Ok("1"){return;}
 for(r,&(name,n,hash))in GUARDS.iter().enumerate(){let a=k::META[r].0;if bins.first().and_then(|b|b.exported_symbols.get(name))!=Some(&(a|1))||!mem.is_mapped_range(a,n)||format!("{:x}",sha1::Sha1::digest(mem.bytes_at(ConstPtr::<u8>::from_bits(a),n)))!=hash{event(r,255,"image_guard_rejected",0);continue;}install_entry(r,name,a,mem,dyld,libc);}
}
fn install_entry(r:usize,name:&'static str,a:u32,mem:&mut crate::mem::Mem,dyld:&mut crate::dyld::Dyld,libc:&mut crate::libc::State){let prefix:[u8;10]=mem.bytes_at(ConstPtr::<u8>::from_bits(a),10).try_into().unwrap();let tr=mem.alloc(20).cast::<u32>();mem.bytes_at_mut(tr.cast::<u8>(),10).copy_from_slice(&prefix);mem.write(MutPtr::<u16>::from_bits(tr.to_bits()+10),0xbf00u16);mem.write(tr+3,0xf000f8dfu32);mem.write(tr+4,a+10|1);let host=dyld.create_guest_function(mem,name,FUNCTIONS[r].1);crate::peggle_native_batch::protect_code(libc,tr.to_bits(),20);crate::peggle_native_batch::protect_code(libc,host.addr_without_thumb_bit(),8);libc.peggle_jpeg_idct.hooks[r]=Hook{original:Some(GuestFunction::from_addr_with_thumb_bit(tr.to_bits()|1)),prefix,..Hook::default()};mem.write(MutPtr::<u32>::from_bits(a),0xf000f8dfu32);mem.write(MutPtr::<u32>::from_bits(a+4),host.addr_with_thumb_bit());mem.write(MutPtr::<u16>::from_bits(a+8),0xbf00u16);event(r,255,"guarded_block_transform_installed",0);}
