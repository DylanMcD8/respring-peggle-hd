/* This Source Code Form is subject to the Mozilla Public License, v. 2.0. */
//! Native measured endpoint exchange. Original sort/pair/callback logic remains.
use crate::{Environment,cpu::CpuContext,abi::GuestFunction,mem::{ConstPtr,MutPtr}};
use crate::dyld::{export_c_func,FunctionExports};
use crate::collision_swap_kernel::{self as kernel,Outcome};
#[derive(Default)]pub(crate)struct State{original:Option<GuestFunction>,prefix:[u8;8],address:u32,disabled:bool,validating:bool,corpus:bool,shadows:u64,calls:u64,fast:u64,fallback:u64}
#[derive(Clone,Copy)]struct Inputs{a:u32,b:u32,sp:u32,left:[u8;12],right:[u8;12]}
fn report(kind:&str,count:u64){crate::diagnostics::event(&format!("{{\"event\":\"collision_swap_native\",\"kind\":\"{kind}\",\"count\":{count}}}"));}
fn original(env:&mut Environment){env.libc_state.peggle_collision_swap.original.unwrap().call_without_pushing_stack_frame(env);}
fn input(env:&Environment)->Option<Inputs>{let r=env.cpu.regs();let(a,b,sp)=(r[0],r[1],r[13]);if !kernel::eligible(a,b,sp,env.cpu.cpsr())||env.mem.get_bytes_fallible(ConstPtr::<u8>::from_bits(sp-20).cast(),20).is_none(){return None;}
 Some(Inputs{a,b,sp,left:env.mem.get_bytes_fallible(ConstPtr::<u8>::from_bits(a).cast(),12)?.try_into().unwrap(),right:env.mem.get_bytes_fallible(ConstPtr::<u8>::from_bits(b).cast(),12)?.try_into().unwrap()})}
fn predicted(mut c:CpuContext,p:Outcome)->CpuContext{c.regs[0]=p.r0;c.regs[1]=p.r1;c.regs[2]=p.r2;c}
fn equal(a:CpuContext,b:CpuContext)->bool{a.regs==b.regs&&a.extregs==b.extregs&&a.cpsr==b.cpsr&&a.fpscr==b.fpscr}
fn apply(env:&mut Environment,i:Inputs,p:Outcome){env.mem.bytes_at_mut(MutPtr::<u8>::from_bits(i.a),12).copy_from_slice(&p.left);env.mem.bytes_at_mut(MutPtr::<u8>::from_bits(i.b),12).copy_from_slice(&p.right);env.mem.bytes_at_mut(MutPtr::<u8>::from_bits(i.sp-20),20).copy_from_slice(&p.stack);let r=env.cpu.regs_mut();r[0]=p.r0;r[1]=p.r1;r[2]=p.r2;}
fn disable(env:&mut Environment,why:&str){let s=&mut env.libc_state.peggle_collision_swap;s.disabled=true;let(a,p)=(s.address,s.prefix);env.mem.bytes_at_mut(MutPtr::<u8>::from_bits(a),8).copy_from_slice(&p);env.cpu.invalidate_cache_range(a,8);report(why,0);}
fn corpus(env:&mut Environment)->bool{let saved=env.cpu.visible_context();let Some(base)=env.mem.try_alloc(256).map(|p|p.cast::<u8>())else{return false;};let a=base.to_bits();env.libc_state.peggle_collision_swap.validating=true;let mut passed=true;
 for n in 0..128u32{for(j,v)in env.mem.bytes_at_mut(base,256).iter_mut().enumerate(){*v=(j as u32*37+n*71)as u8;}
  let mut before=saved;before.regs[0]=a+32;before.regs[1]=if n&1==0{a+64}else{a+32};before.regs[13]=a+240;before.cpsr=(saved.cpsr&0x09ff03ff)|((n&15)<<28);before.fpscr=n.wrapping_mul(0x10203041);env.cpu.restore_visible_context(before);
  let i=input(env).unwrap();let p=kernel::calculate(i.a,i.b,i.left,i.right);let mut expected=env.mem.bytes_at(base.cast_const(),256).to_vec();expected[32..44].copy_from_slice(&p.left);let q=(i.b-a)as usize;expected[q..q+12].copy_from_slice(&p.right);expected[220..240].copy_from_slice(&p.stack);
  original(env);if !equal(env.cpu.visible_context(),predicted(before,p))||env.mem.bytes_at(base.cast_const(),256)!=expected{passed=false;report("corpus_mismatch_at",n as u64);break;}
 }
 env.cpu.restore_visible_context(saved);env.mem.free(base.cast());env.libc_state.peggle_collision_swap.validating=false;report(if passed{"corpus_passed"}else{"corpus_failed"},if passed{128}else{0});passed
}
fn collision_swap(env:&mut Environment){let s=&env.libc_state.peggle_collision_swap;if s.disabled||s.validating{return original(env);}
 let Some(i)=input(env)else{let s=&mut env.libc_state.peggle_collision_swap;s.fallback+=1;if s.fallback==1||s.fallback%65536==0{report("address_alias_or_control_fallback",s.fallback);}return original(env);};
 if !env.libc_state.peggle_collision_swap.corpus{if !corpus(env){disable(env,"disabled_after_corpus_rejection");return original(env);}env.libc_state.peggle_collision_swap.corpus=true;}
 let p=kernel::calculate(i.a,i.b,i.left,i.right);let s=&mut env.libc_state.peggle_collision_swap;s.calls+=1;
 if s.shadows<128||s.calls%4096==0{let before=env.cpu.visible_context();original(env);if !equal(env.cpu.visible_context(),predicted(before,p))||env.mem.bytes_at(ConstPtr::<u8>::from_bits(i.a),12)!=p.left||env.mem.bytes_at(ConstPtr::<u8>::from_bits(i.b),12)!=p.right||env.mem.bytes_at(ConstPtr::<u8>::from_bits(i.sp-20),20)!=p.stack{disable(env,"disabled_after_real_input_mismatch");return;}
  let s=&mut env.libc_state.peggle_collision_swap;s.shadows+=1;if s.shadows==128{report("real_input_gate_passed",128);}return;
 }
 apply(env,i,p);let s=&mut env.libc_state.peggle_collision_swap;s.fast+=1;if s.fast==1||s.fast%1048576==0{report("fast_calls",s.fast);}
}
const FUNCTIONS:FunctionExports=&[export_c_func!(collision_swap())];
pub(crate)fn install(verified:bool,bins:&[crate::mach_o::MachO],mem:&mut crate::mem::Mem,dyld:&mut crate::dyld::Dyld,libc:&mut crate::libc::State){use digest::Digest;if !verified||std::env::var("PEGGLE_HD_COLLISION_SWAP_NATIVE").as_deref()!=Ok("1"){return;}
 let a=0x61634;let name="__ZSt4swapIN4Sexy12CollisionMgr8EndPointEEvRT_S4_";let prefix:[u8;8]=mem.bytes_at(ConstPtr::<u8>::from_bits(a),8).try_into().unwrap();if bins[0].exported_symbols.get(name)!=Some(&(a|1))||format!("{:x}",sha1::Sha1::digest(mem.bytes_at(ConstPtr::<u8>::from_bits(a),0x38)))!=FUNCTION_SHA1||prefix!=[0x85,0xb0,0x04,0x90,0x03,0x91,0x04,0x98]{report("image_guard_rejected",0);return;}
 let t=mem.alloc(16).cast::<u32>();mem.bytes_at_mut(t.cast::<u8>(),8).copy_from_slice(&prefix);mem.write(t+2,0xf000_f8dfu32);mem.write(t+3,(a+8)|1);let target=dyld.create_guest_function(mem,"HD_collision_swap",FUNCTIONS[0].1);libc.peggle_collision_swap=State{original:Some(GuestFunction::from_addr_with_thumb_bit(t.to_bits()|1)),prefix,address:a,..State::default()};let p=MutPtr::<u32>::from_bits(a);mem.write(p,0xf000_f8dfu32);mem.write(p+1,target.addr_with_thumb_bit());report("guarded_hook_installed",0);
}
const FUNCTION_SHA1:&str="4eeffa4ef13fde6c5863c0abfe523dff812d7f0b";
