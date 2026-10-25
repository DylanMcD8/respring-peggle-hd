/* This Source Code Form is subject to the Mozilla Public License, v. 2.0. */
//! Exact guarded font dictionaries; two independent gates and fallback prefixes.
use crate::{Environment,cpu::CpuContext,abi::GuestFunction,mem::{ConstPtr,MutPtr}};
use crate::dyld::{export_c_func,FunctionExports};
use crate::font_lookup_kernel::{self as kernel,Kind,Outcome};
#[derive(Default)]struct Entry{original:Option<GuestFunction>,prefix:[u8;8],address:u32,disabled:bool,validating:bool,corpus_passed:bool,shadows:u64,fast:u64,calls:u64,fallback:u64}
#[derive(Default)]pub(crate)struct State{entries:[Entry;2]}
#[derive(Clone,Copy)]struct Inputs{this:u32,base:u32,count:u16,stack:u32,header_len:u32}
fn kind(i:usize)->Kind{if i==0{Kind::Glyph}else{Kind::Kerning}}
fn report(i:usize,k:&str,n:u64){crate::diagnostics::event(&format!("{{\"event\":\"font_lookup_native\",\"routine\":\"{}\",\"kind\":\"{k}\",\"count\":{n}}}",if i==0{"glyph"}else{"kerning"}));}
fn equal(a:CpuContext,b:CpuContext)->bool{a.regs==b.regs&&a.extregs==b.extregs&&a.cpsr==b.cpsr&&a.fpscr==b.fpscr}
fn original(env:&mut Environment,i:usize){let sp=env.cpu.regs()[13];let lr=env.cpu.regs()[14];let f=env.libc_state.peggle_font_lookup.entries[i].original.unwrap();f.call_without_pushing_stack_frame(env);
 // CallFromHost substitutes its return trap in LR. Restore only that known
 // instrumentation spill to the caller's original LR, so canonical full-stack
 // comparisons and fast results match an unwrapped direct guest call.
 env.mem.write(MutPtr::<u32>::from_bits(sp-4),lr);
}
fn disable(env:&mut Environment,i:usize,why:&str){let s=&mut env.libc_state.peggle_font_lookup.entries[i];s.disabled=true;let(a,p)=(s.address,s.prefix);env.mem.bytes_at_mut(MutPtr::<u8>::from_bits(a),8).copy_from_slice(&p);env.cpu.invalidate_cache_range(a,8);report(i,why,0);}
fn inputs(env:&Environment,i:usize,c:CpuContext)->Option<(Inputs,Outcome)>{
 let k=kind(i);let this=c.regs[0];let len=if i==0{0x3e}else{0x46};let h=env.mem.get_bytes_fallible(ConstPtr::<u8>::from_bits(this).cast(),len)?;let offset=if i==0{0x38}else{0x40};let base=u32::from_le_bytes(h[offset..offset+4].try_into().unwrap());let count=u16::from_le_bytes(h[offset+4..offset+6].try_into().unwrap());
 if !kernel::eligible(c,k,this,base,count){return None;}
 let stack=c.regs[13]-k.stack_len()as u32;let prior=env.mem.get_bytes_fallible(ConstPtr::<u8>::from_bits(stack).cast(),k.stack_len()as u32)?;
 let table=if count==0{&[][..]}else{env.mem.get_bytes_fallible(ConstPtr::<u8>::from_bits(base).cast(),u32::from(count)*k.stride()as u32)?};
 Some((Inputs{this,base,count,stack,header_len:len},kernel::calculate(c,k,base,count,table,prior)?))
}
fn division_ready(env:&Environment)->bool{env.mem.get_bytes_fallible(ConstPtr::<u8>::from_bits(0x2f7bec).cast(),8).is_some_and(|b|{let a=u32::from_le_bytes(b[..4].try_into().unwrap());a>=0xef000003&&a<0xef800000&&b[4..8]==0xe12fff1eu32.to_le_bytes()})}
fn compare(env:&mut Environment,i:usize,before:CpuContext,input:Inputs,p:Outcome)->bool{
 let header=env.mem.bytes_at(ConstPtr::<u8>::from_bits(input.this),input.header_len).to_vec();let bytes=u32::from(input.count)*kind(i).stride()as u32;let table=if bytes==0{Vec::new()}else{env.mem.bytes_at(ConstPtr::<u8>::from_bits(input.base),bytes).to_vec()};
 original(env,i);equal(env.cpu.visible_context(),p.context)&&env.mem.bytes_at(ConstPtr::<u8>::from_bits(input.stack),p.length as u32)==&p.stack[..p.length]&&env.mem.bytes_at(ConstPtr::<u8>::from_bits(input.this),input.header_len)==header&&(bytes==0||env.mem.bytes_at(ConstPtr::<u8>::from_bits(input.base),bytes)==table)
}
fn corpus_gate(env:&mut Environment,i:usize)->bool{
 let saved=env.cpu.visible_context();let Some(a)=env.mem.try_alloc(4096).map(|p|p.cast::<u8>())else{return false;};let base=a.to_bits();env.libc_state.peggle_font_lookup.entries[i].validating=true;let mut ok=true;let mut completed=0;
 for n in 0..128u32{
  env.mem.bytes_at_mut(a,4096).fill(0xa5);let count=(n%17)as u16;let this=base+16;let table=base+128;let sp=base+4000;let offset=if i==0{0x38}else{0x40};env.mem.write(MutPtr::<u32>::from_bits(this+offset),table);env.mem.write(MutPtr::<u16>::from_bits(this+offset+4),count);
  for j in 0..u32::from(count){let key=if i==0{j*997}else{j.wrapping_mul(0x07110203)};env.mem.write(MutPtr::<u32>::from_bits(table+j*kind(i).stride()as u32),key);}
  let mut c=saved;c.regs[0]=this;c.regs[13]=sp;c.cpsr=(saved.cpsr&0x0fffffff)|((n&15)<<28);let probe=if n%3==0&&count>0{u32::from(count/2)*if i==0{997}else{0x07110203}}else{n.wrapping_mul(0x10203041)};c.regs[1]=0xabcd0000|(probe&0xffff);c.regs[2]=0xfedc0000|(probe>>16);env.cpu.restore_visible_context(c);
  let Some((input,p))=inputs(env,i,c)else{ok=false;break;};if !compare(env,i,c,input,p){ok=false;report(i,"corpus_state_or_memory_mismatch_at",n as u64);break;}completed+=1;
 }
 env.cpu.restore_visible_context(saved);env.mem.free(a.cast());env.libc_state.peggle_font_lookup.entries[i].validating=false;report(i,if ok{"corpus_passed"}else{"corpus_failed"},completed);ok
}
fn lookup(env:&mut Environment,i:usize){
 let s=&env.libc_state.peggle_font_lookup.entries[i];if s.disabled||s.validating{return original(env,i);}
 let before=env.cpu.visible_context();let Some((input,p))=inputs(env,i,before)else{let s=&mut env.libc_state.peggle_font_lookup.entries[i];s.fallback+=1;let n=s.fallback;if n==1||n%65536==0{report(i,"address_control_fallback",n);}return original(env,i);};
 if !division_ready(env){report(i,"division_link_original_forwarded",0);return original(env,i);}
 if !env.libc_state.peggle_font_lookup.entries[i].corpus_passed{if !corpus_gate(env,i){disable(env,i,"disabled_after_corpus_rejection");return original(env,i);}env.libc_state.peggle_font_lookup.entries[i].corpus_passed=true;}
 let s=&mut env.libc_state.peggle_font_lookup.entries[i];s.calls+=1;let shadow=s.shadows<128||s.calls%4096==0;
 if shadow{if !compare(env,i,before,input,p){disable(env,i,"disabled_after_real_input_mismatch");return;}let s=&mut env.libc_state.peggle_font_lookup.entries[i];s.shadows+=1;if s.shadows==128{report(i,"real_input_gate_passed",128);}return;}
 env.mem.bytes_at_mut(MutPtr::<u8>::from_bits(input.stack),p.length as u32).copy_from_slice(&p.stack[..p.length]);env.cpu.restore_visible_context(p.context);let s=&mut env.libc_state.peggle_font_lookup.entries[i];s.fast+=1;let n=s.fast;if n==1||n%65536==0{report(i,"fast_calls",n);}
}
fn glyph(env:&mut Environment){lookup(env,0)}fn kerning(env:&mut Environment){lookup(env,1)}
const FUNCTIONS:FunctionExports=&[export_c_func!(glyph()),export_c_func!(kerning())];
pub(crate)fn install(verified:bool,bins:&[crate::mach_o::MachO],mem:&mut crate::mem::Mem,dyld:&mut crate::dyld::Dyld,libc:&mut crate::libc::State){
 use digest::Digest;if !verified||std::env::var("PEGGLE_HD_FONT_LOOKUP_NATIVE").as_deref()!=Ok("1"){return;}
 let guards=[("__ZN4Sexy9FontLayer11GetCharDataEw",0x23a104,0xa8,"277e371f2a3eceefebf5104f33df74f94735a21e"),("__ZN4Sexy9FontLayer17LookupKerningInfoEww",0x23a1ac,0x9c,"e00f7df6613380efb53b124a81e2e94a20bd8280")];
 for(i,(symbol,address,size,hash))in guards.into_iter().enumerate(){
  if bins[0].exported_symbols.get(symbol)!=Some(&(address|1))||format!("{:x}",sha1::Sha1::digest(mem.bytes_at(ConstPtr::<u8>::from_bits(address),size)))!=hash{report(i,"image_guard_rejected",0);continue;}
  let prefix:[u8;8]=mem.bytes_at(ConstPtr::<u8>::from_bits(address),8).try_into().unwrap();let trampoline=mem.alloc(16).cast::<u32>();mem.bytes_at_mut(trampoline.cast::<u8>(),8).copy_from_slice(&prefix);mem.write(trampoline+2,0xf000_f8dfu32);mem.write(trampoline+3,(address+8)|1);let host=dyld.create_guest_function(mem,symbol,FUNCTIONS[i].1);
  libc.peggle_font_lookup.entries[i]=Entry{original:Some(GuestFunction::from_addr_with_thumb_bit(trampoline.to_bits()|1)),prefix,address,..Entry::default()};let p=MutPtr::<u32>::from_bits(address);mem.write(p,0xf000_f8dfu32);mem.write(p+1,host.addr_with_thumb_bit());report(i,"guarded_hook_installed",0);
 }
}
