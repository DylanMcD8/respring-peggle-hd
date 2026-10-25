/* This Source Code Form is subject to the Mozilla Public License,v2.0. */
//! C36 pure clip rectangle closure. Original text/GL owners remain intact.
use crate::{Environment,cpu::CpuContext,abi::GuestFunction,mem::{ConstPtr,MutPtr}};
use crate::dyld::{export_c_func,FunctionExports};use crate::clip_rect_kernel as k;
#[derive(Default)]pub(crate)struct State{original:Option<GuestFunction>,prefix:[u8;8],disabled:bool,validating:bool,corpus:bool,total:u64,shadows:[u64;2],calls:[u64;2],fast:[u64;2]}
fn event(class:usize,kind:&str,count:u64){crate::diagnostics::event(&format!("{{\"event\":\"clip_rect_native\",\"class\":{class},\"kind\":\"{kind}\",\"count\":{count}}}"));}
fn original(e:&mut Environment){let(sp,lr)=(e.cpu.regs()[13],e.cpu.regs()[14]);e.libc_state.peggle_clip_rect.original.unwrap().call_without_pushing_stack_frame(e);e.mem.write(MutPtr::<u32>::from_bits(sp-4),lr);}
fn equal(a:CpuContext,b:CpuContext)->bool{a.regs==b.regs&&a.extregs==b.extregs&&a.cpsr==b.cpsr&&a.fpscr==b.fpscr}
fn predict(e:&Environment)->Option<crate::native_batch_kernel::Outcome>{let g=&e.libc_state.peggle_native_batch;k::calculate(k::snapshot(&e.mem,e.cpu.visible_context(),g.code_guard())?)}
fn compare(e:&mut Environment,p:crate::native_batch_kernel::Outcome)->bool{original(e);equal(e.cpu.visible_context(),p.context)&&p.regions.iter().all(|r|e.mem.bytes_at(ConstPtr::<u8>::from_bits(r.address),r.bytes.len()as u32)==&r.bytes[..])}
fn disable(e:&mut Environment,why:&str){e.libc_state.peggle_clip_rect.disabled=true;let prefix=e.libc_state.peggle_clip_rect.prefix;e.mem.bytes_at_mut(MutPtr::<u8>::from_bits(k::ENTRY),8).copy_from_slice(&prefix);e.cpu.invalidate_cache_range(k::ENTRY,8);event(255,why,0);}
fn corpus(e:&mut Environment)->bool{let saved=e.cpu.visible_context();let Some(a)=e.mem.try_alloc(4096).map(|p|p.cast::<u8>())else{return false;};e.libc_state.peggle_clip_rect.validating=true;let mut passed=0;
 for n in 0..128{let c=k::seed(&mut e.mem,saved,a.to_bits(),n);e.cpu.restore_visible_context(c);let Some(p)=predict(e)else{break;};if !compare(e,p){break;}passed+=1;}
 e.cpu.restore_visible_context(saved);e.mem.free(a.cast());e.libc_state.peggle_clip_rect.validating=false;event(255,if passed==128{"corpus_passed"}else{"corpus_failed"},passed);passed==128
}
fn clip_rect(e:&mut Environment){let s=&e.libc_state.peggle_clip_rect;if s.disabled||s.validating||!k::linked(&e.mem){return original(e);}
 let Some(p)=predict(e)else{return original(e);};let class=p.class;
 if !s.corpus{if !corpus(e){disable(e,"disabled_after_corpus_failure");return original(e);}e.libc_state.peggle_clip_rect.corpus=true;}
 let s=&mut e.libc_state.peggle_clip_rect;s.calls[class]+=1;
 if s.total<128||s.shadows[class]<16||s.calls[class]%1024==0{if !compare(e,p){disable(e,"disabled_after_real_mismatch");}else{let s=&mut e.libc_state.peggle_clip_rect;s.total+=1;s.shadows[class]+=1;if s.total==128{event(255,"real_global_gate_passed",128);}if s.shadows[class]==16{event(class,"real_class_gate_passed",16);}}return;}
 for r in &p.regions{if r.writable{e.mem.bytes_at_mut(MutPtr::<u8>::from_bits(r.address),r.bytes.len()as u32).copy_from_slice(&r.bytes);}}e.cpu.restore_visible_context(p.context);let s=&mut e.libc_state.peggle_clip_rect;s.fast[class]+=1;let n=s.fast[class];if n==1||n%1024==0{event(class,"fast_calls",n);}
}
const FUNCTIONS:FunctionExports=&[export_c_func!(clip_rect())];
pub(crate)fn install(verified:bool,bins:&[crate::mach_o::MachO],mem:&mut crate::mem::Mem,dyld:&mut crate::dyld::Dyld,libc:&mut crate::libc::State)->bool{use digest::Digest;if !verified||std::env::var("PEGGLE_HD_CLIP_RECT_NATIVE").as_deref()!=Ok("1"){return false;}
 for(name,a,n,hash)in [("__ZSt3maxIiERKT_S2_S2_", 90500, 36, "f30a16a8cd97218c48f574d67254c335a82c31d4"), ("__ZSt3minIiERKT_S2_S2_", 162416, 36, "717a51ff851579abb3798692250e9c5110e3bad9"), ("__ZN4Sexy5TRectIiEC1Eiiii", 1398480, 56, "f12c83e8a49f573232daf5b6a6dabf25e3590097"), ("__ZN4Sexy5TRectIiEC2Eiiii", 1408376, 40, "cb192fd635f130e380707e0a3d091debff400ca9")] {if bins.first().and_then(|b|b.exported_symbols.get(name))!=Some(&(a|1))||!mem.is_mapped_range(a,n)||format!("{:x}",sha1::Sha1::digest(mem.bytes_at(ConstPtr::<u8>::from_bits(a),n)))!=hash{event(255,"helper_image_guard_rejected",0);return false;}}
 let name="__ZNK4Sexy5TRectIiE12IntersectionERKS1_";if bins.first().and_then(|b|b.exported_symbols.get(name))!=Some(&(k::ENTRY|1))||!mem.is_mapped_range(k::ENTRY,236)||format!("{:x}",sha1::Sha1::digest(mem.bytes_at(ConstPtr::<u8>::from_bits(k::ENTRY),236)))!="24b77826ff58f7c2dfabe64c1cd945974356ab69"{event(255,"image_guard_rejected",0);return false;}
 let prefix:[u8;8]=mem.bytes_at(ConstPtr::<u8>::from_bits(k::ENTRY),8).try_into().unwrap();let tr=mem.alloc(16).cast::<u32>();mem.bytes_at_mut(tr.cast::<u8>(),8).copy_from_slice(&prefix);mem.write(tr+2,0xf000f8dfu32);mem.write(tr+3,k::ENTRY+8|1);let host=dyld.create_guest_function(mem,name,FUNCTIONS[0].1);crate::peggle_native_batch::protect_code(libc,tr.to_bits(),16);crate::peggle_native_batch::protect_code(libc,host.addr_without_thumb_bit(),8);libc.peggle_clip_rect=State{original:Some(GuestFunction::from_addr_with_thumb_bit(tr.to_bits()|1)),prefix,..State::default()};mem.write(MutPtr::<u32>::from_bits(k::ENTRY),0xf000f8dfu32);mem.write(MutPtr::<u32>::from_bits(k::ENTRY+4),host.addr_with_thumb_bit());event(255,"guarded_pure_closure_installed",0);true
}
