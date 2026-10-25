/* This Source Code Form is subject to the Mozilla Public License, v. 2.0. */
//! Guarded ImageFont mapping. Independent opt-in, corpus/real/periodic gates.
use crate::{Environment,cpu::CpuContext,abi::GuestFunction,mem::{ConstPtr,MutPtr}};
use crate::dyld::{export_c_func,FunctionExports};
use crate::mapped_char_kernel::{self as kernel,Outcome};
#[derive(Default)]pub(crate)struct State{original:Option<GuestFunction>,prefix:[u8;8],address:u32,disabled:bool,validating:bool,corpus_passed:bool,shadows:u64,fast:u64,calls:u64,fallback:u64}
#[derive(Clone,Copy)]struct Inputs{this:u32,data:u32,table:u32,count:u32,stack:u32}
fn report(kind:&str,n:u64){crate::diagnostics::event(&format!("{{\"event\":\"mapped_char_native\",\"kind\":\"{kind}\",\"count\":{n}}}"));}
fn equal(a:CpuContext,b:CpuContext)->bool{a.regs==b.regs&&a.extregs==b.extregs&&a.cpsr==b.cpsr&&a.fpscr==b.fpscr}
fn original(env:&mut Environment){let(sp,lr)=(env.cpu.regs()[13],env.cpu.regs()[14]);env.libc_state.peggle_mapped_char.original.unwrap().call_without_pushing_stack_frame(env);
 // Normalize only CallFromHost's substituted return-trap LR spill. The routine
 // unconditionally pushes LR at entry SP-4 before its only __divsi3 callback.
 env.mem.write(MutPtr::<u32>::from_bits(sp-4),lr);
}
fn disable(env:&mut Environment,why:&str){let s=&mut env.libc_state.peggle_mapped_char;s.disabled=true;let(a,p)=(s.address,s.prefix);env.mem.bytes_at_mut(MutPtr::<u8>::from_bits(a),8).copy_from_slice(&p);env.cpu.invalidate_cache_range(a,8);report(why,0);}
fn inputs(env:&Environment,c:CpuContext)->Option<(Inputs,Outcome)>{
 let this=c.regs[0];if !env.mem.is_mapped_range(this,32){return None;}let font=env.mem.get_bytes_fallible(ConstPtr::<u8>::from_bits(this).cast(),32)?;let data=u32::from_le_bytes(font[28..32].try_into().ok()?);if !env.mem.is_mapped_range(data,96){return None;}let header=env.mem.get_bytes_fallible(ConstPtr::<u8>::from_bits(data).cast(),96)?;
 let table=u32::from_le_bytes(header[88..92].try_into().ok()?);let count=u32::from_le_bytes(header[92..96].try_into().ok()?);if count>kernel::MAX_COUNT{return None;}
 let stack=c.regs[13].checked_sub(40)?;if !env.mem.is_mapped_range(stack,40)||count!=0&&!env.mem.is_mapped_range(table,count*4){return None;}let prior=env.mem.get_bytes_fallible(ConstPtr::<u8>::from_bits(stack).cast(),40)?;let bytes=if count==0{&[][..]}else{env.mem.get_bytes_fallible(ConstPtr::<u8>::from_bits(table).cast(),count*4)?};Some((Inputs{this,data,table,count,stack},kernel::calculate(c,data,table,count,bytes,prior)?))
}
fn division_ready(env:&Environment)->bool{env.mem.is_mapped_range(0x2f7bec,8)&&env.mem.get_bytes_fallible(ConstPtr::<u8>::from_bits(0x2f7bec).cast(),8).is_some_and(|b|{let a=u32::from_le_bytes(b[..4].try_into().unwrap());a>=0xef000003&&a<0xef800000&&b[4..8]==0xe12fff1eu32.to_le_bytes()})}
fn compare(env:&mut Environment,input:Inputs,p:Outcome)->bool{
 let font=env.mem.bytes_at(ConstPtr::<u8>::from_bits(input.this),32).to_vec();let header=env.mem.bytes_at(ConstPtr::<u8>::from_bits(input.data),96).to_vec();let table=if input.count==0{Vec::new()}else{env.mem.bytes_at(ConstPtr::<u8>::from_bits(input.table),input.count*4).to_vec()};original(env);
 equal(env.cpu.visible_context(),p.context)&&env.mem.bytes_at(ConstPtr::<u8>::from_bits(input.stack),40)==p.stack&&env.mem.bytes_at(ConstPtr::<u8>::from_bits(input.this),32)==font&&env.mem.bytes_at(ConstPtr::<u8>::from_bits(input.data),96)==header&&(input.count==0||env.mem.bytes_at(ConstPtr::<u8>::from_bits(input.table),input.count*4)==table)
}
fn corpus_gate(env:&mut Environment)->bool{
 let saved=env.cpu.visible_context();let Some(a)=env.mem.try_alloc(8192).map(|p|p.cast::<u8>())else{return false;};let base=a.to_bits();env.libc_state.peggle_mapped_char.validating=true;let(mut ok,mut completed)=(true,0);
 for n in 0..128u32{
  env.mem.bytes_at_mut(a,8192).fill(0xa5);let(this,data,table,sp)=(base+16,base+256,base+512,base+8000);let count=n%33;env.mem.write(MutPtr::<u32>::from_bits(this+28),data);env.mem.write(MutPtr::<u32>::from_bits(data+88),table);env.mem.write(MutPtr::<u32>::from_bits(data+92),count);
  for j in 0..count{let key=match n%3{0=>j*997,1=>73,_=>(j*997)^0xa5a5};env.mem.write(MutPtr::<u16>::from_bits(table+j*4),key as u16);env.mem.write(MutPtr::<u16>::from_bits(table+j*4+2),j.wrapping_mul(1237)as u16);}
  let mut c=saved;c.regs[0]=this;c.regs[13]=sp;c.cpsr=(saved.cpsr&0x0fffffff)|((n&15)<<28);c.regs[1]=match n%5{0=>73,1=>0xffff0049,2=>0x80000049,3=>count/2*997,_=>n.wrapping_mul(0x10203041)};env.cpu.restore_visible_context(c);
  let Some((input,p))=inputs(env,c)else{ok=false;break;};if !compare(env,input,p){report("corpus_state_or_memory_mismatch_at",u64::from(n));ok=false;break;}completed+=1;
 }
 env.cpu.restore_visible_context(saved);env.mem.free(a.cast());env.libc_state.peggle_mapped_char.validating=false;report(if ok{"corpus_passed"}else{"corpus_failed"},completed);ok
}
fn mapped_char(env:&mut Environment){
 let s=&env.libc_state.peggle_mapped_char;if s.disabled||s.validating{return original(env);}
 let before=env.cpu.visible_context();let Some((input,p))=inputs(env,before)else{let s=&mut env.libc_state.peggle_mapped_char;s.fallback+=1;let n=s.fallback;if n==1||n%65536==0{report("address_control_fallback",n);}return original(env);};
 if !division_ready(env){report("division_link_original_forwarded",0);return original(env);}
 if !env.libc_state.peggle_mapped_char.corpus_passed{if !corpus_gate(env){disable(env,"disabled_after_corpus_rejection");return original(env);}env.libc_state.peggle_mapped_char.corpus_passed=true;}
 let s=&mut env.libc_state.peggle_mapped_char;s.calls+=1;let shadow=s.shadows<128||s.calls%4096==0;
 if shadow{if !compare(env,input,p){disable(env,"disabled_after_real_input_mismatch");return;}let s=&mut env.libc_state.peggle_mapped_char;s.shadows+=1;if s.shadows==128{report("real_input_gate_passed",128);}return;}
 env.mem.bytes_at_mut(MutPtr::<u8>::from_bits(input.stack),40).copy_from_slice(&p.stack);env.cpu.restore_visible_context(p.context);let s=&mut env.libc_state.peggle_mapped_char;s.fast+=1;let n=s.fast;if n==1||n%65536==0{report("fast_calls",n);}
}
const FUNCTIONS:FunctionExports=&[export_c_func!(mapped_char())];
pub(crate)fn install(verified:bool,bins:&[crate::mach_o::MachO],mem:&mut crate::mem::Mem,dyld:&mut crate::dyld::Dyld,libc:&mut crate::libc::State){
 use digest::Digest;if !verified||std::env::var("PEGGLE_HD_MAPPED_CHAR_NATIVE").as_deref()!=Ok("1"){return;}
 let(symbol,address,size,hash)=("__ZN4Sexy9ImageFont13GetMappedCharEw",0x245af0u32,164,"92fb8a847ffd616dcdf015971276ade95d5fff19");let prefix=[0x80,0xb5,0x6f,0x46,0x88,0xb0,0x06,0x90];
 if !mem.is_mapped_range(address,size){report("image_guard_rejected",0);return;}let Some(body)=mem.get_bytes_fallible(ConstPtr::<u8>::from_bits(address).cast(),size)else{report("image_guard_rejected",0);return;};
 if bins.first().and_then(|b|b.exported_symbols.get(symbol))!=Some(&(address|1))||body[..8]!=prefix||format!("{:x}",sha1::Sha1::digest(body))!=hash{report("image_guard_rejected",0);return;}
 let trampoline=mem.alloc(16).cast::<u32>();mem.bytes_at_mut(trampoline.cast::<u8>(),8).copy_from_slice(&prefix);mem.write(trampoline+2,0xf000_f8dfu32);mem.write(trampoline+3,(address+8)|1);let host=dyld.create_guest_function(mem,symbol,FUNCTIONS[0].1);libc.peggle_mapped_char=State{original:Some(GuestFunction::from_addr_with_thumb_bit(trampoline.to_bits()|1)),prefix,address,..State::default()};let p=MutPtr::<u32>::from_bits(address);mem.write(p,0xf000_f8dfu32);mem.write(p+1,host.addr_with_thumb_bit());report("guarded_hook_installed",0);
}
