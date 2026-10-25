/* This Source Code Form is subject to the Mozilla Public License,v2.0. */
//! C36 whole non-owning decoder loop; original full-stream anchors stay intact.
use crate::{Environment,cpu::CpuContext,abi::GuestFunction,mem::{ConstPtr,MutPtr}};
use crate::dyld::{export_c_func,FunctionExports};use crate::inflate_fast_kernel as k;
#[derive(Default)]pub(crate)struct State{original:Option<GuestFunction>,prefix:[u8;12],disabled:bool,validating:bool,corpus:bool,total:u64,shadows:[u64;3],calls:[u64;3],fast:[u64;3]}
fn event(class:usize,kind:&str,count:u64){crate::diagnostics::event(&format!("{{\"event\":\"inflate_fast_native\",\"class\":{class},\"kind\":\"{kind}\",\"count\":{count}}}"));}
fn original(e:&mut Environment){e.libc_state.peggle_inflate_fast.original.unwrap().call_without_pushing_stack_frame(e);}
fn equal(a:CpuContext,b:CpuContext)->bool{a.regs==b.regs&&a.extregs==b.extregs&&a.cpsr==b.cpsr&&a.fpscr==b.fpscr}
fn predict(e:&Environment)->Option<crate::native_batch_kernel::Outcome>{let g=&e.libc_state.peggle_native_batch;k::calculate(k::snapshot(&e.mem,e.cpu.visible_context(),g.code_guard())?)}
fn compare(e:&mut Environment,p:crate::native_batch_kernel::Outcome)->bool{original(e);equal(e.cpu.visible_context(),p.context)&&p.regions.iter().all(|r|e.mem.bytes_at(ConstPtr::<u8>::from_bits(r.address),r.bytes.len()as u32)==&r.bytes[..])}
fn disable(e:&mut Environment,why:&str){e.libc_state.peggle_inflate_fast.disabled=true;let prefix=e.libc_state.peggle_inflate_fast.prefix;e.mem.bytes_at_mut(MutPtr::<u8>::from_bits(k::ENTRY),12).copy_from_slice(&prefix);e.cpu.invalidate_cache_range(k::ENTRY,12);event(255,why,0);}
fn corpus(e:&mut Environment)->bool{let saved=e.cpu.visible_context();let Some(a)=e.mem.try_alloc(262144).map(|p|p.cast::<u8>())else{return false;};e.libc_state.peggle_inflate_fast.validating=true;let mut passed=0;
 for n in 0..128{let c=k::seed(&mut e.mem,saved,a.to_bits(),n);e.cpu.restore_visible_context(c);let Some(p)=predict(e)else{break;};if !compare(e,p){break;}passed+=1;}
 e.cpu.restore_visible_context(saved);e.mem.free(a.cast());e.libc_state.peggle_inflate_fast.validating=false;event(255,if passed==128{"corpus_passed"}else{"corpus_failed"},passed);passed==128
}
fn inflate_fast(e:&mut Environment){let s=&e.libc_state.peggle_inflate_fast;if s.disabled||s.validating||!crate::hd_file_zlib::interpreted_stream_anchor(e)||!k::worthwhile(&e.mem,e.cpu.visible_context()){return original(e);}
 let Some(p)=predict(e)else{return original(e);};let class=p.class;
 if !s.corpus{if !corpus(e){disable(e,"disabled_after_corpus_failure");return original(e);}e.libc_state.peggle_inflate_fast.corpus=true;}
 let s=&mut e.libc_state.peggle_inflate_fast;s.calls[class]+=1;
 if s.total<128||s.shadows[class]<16||s.calls[class]%1024==0{if !compare(e,p){disable(e,"disabled_after_real_mismatch");}else{let s=&mut e.libc_state.peggle_inflate_fast;s.total+=1;s.shadows[class]+=1;if s.total==128{event(255,"real_global_gate_passed",128);}if s.shadows[class]==16{event(class,"real_class_gate_passed",16);}}return;}
 for r in &p.regions{if r.writable{e.mem.bytes_at_mut(MutPtr::<u8>::from_bits(r.address),r.bytes.len()as u32).copy_from_slice(&r.bytes);}}e.cpu.restore_visible_context(p.context);let s=&mut e.libc_state.peggle_inflate_fast;s.fast[class]+=1;let n=s.fast[class];if n==1||n%1024==0{event(class,"fast_calls",n);}
}
const FUNCTIONS:FunctionExports=&[export_c_func!(inflate_fast())];
pub(crate)fn install(verified:bool,bins:&[crate::mach_o::MachO],mem:&mut crate::mem::Mem,dyld:&mut crate::dyld::Dyld,libc:&mut crate::libc::State)->bool{use digest::Digest;if !verified||std::env::var("PEGGLE_HD_INFLATE_FAST_NATIVE").as_deref()!=Ok("1"){return false;}
 let name="_inflate_fast";if bins.first().and_then(|b|b.exported_symbols.get(name))!=Some(&(k::ENTRY|1))||!mem.is_mapped_range(k::ENTRY,1540)||format!("{:x}",sha1::Sha1::digest(mem.bytes_at(ConstPtr::<u8>::from_bits(k::ENTRY),1540)))!="50ddd905ccf384977a7e36a0c0864adf817455e4"{event(255,"image_guard_rejected",0);return false;}
 let prefix:[u8;12]=mem.bytes_at(ConstPtr::<u8>::from_bits(k::ENTRY),12).try_into().unwrap();let tr=mem.alloc(20).cast::<u32>();mem.bytes_at_mut(tr.cast::<u8>(),12).copy_from_slice(&prefix);mem.write(tr+3,0xf000f8dfu32);mem.write(tr+4,k::ENTRY+12|1);let host=dyld.create_guest_function(mem,name,FUNCTIONS[0].1);crate::peggle_native_batch::protect_code(libc,tr.to_bits(),20);crate::peggle_native_batch::protect_code(libc,host.addr_without_thumb_bit(),8);libc.peggle_inflate_fast=State{original:Some(GuestFunction::from_addr_with_thumb_bit(tr.to_bits()|1)),prefix,..State::default()};mem.write(MutPtr::<u32>::from_bits(k::ENTRY),0xf000f8dfu32);mem.write(MutPtr::<u32>::from_bits(k::ENTRY+4),host.addr_with_thumb_bit());mem.write(MutPtr::<u32>::from_bits(k::ENTRY+8),0xbf00bf00u32);event(255,"guarded_whole_loop_installed_first_stream_original",0);true
}
