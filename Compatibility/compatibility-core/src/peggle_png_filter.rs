/* This Source Code Form is subject to the Mozilla Public License, v. 2.0. */
//! Guarded native PNG row reconstruction with independent per-filter gates.
//! Unsupported/error paths tail original guest code without an added Rust call.
use crate::{Environment,cpu::CpuContext,abi::GuestFunction,mem::{ConstPtr,MutPtr},png_filter_kernel::{self as kernel,Outcome}};
use crate::dyld::{export_c_func,FunctionExports};
#[derive(Default)]struct Entry{corpus:bool,shadows:u64,fast:u64,calls:u64}
#[derive(Default)]pub(crate)struct State{original:Option<GuestFunction>,address:u32,prefix:[u8;8],disabled:bool,validating:bool,entries:[Entry;5],fallback:u64}
struct Inputs{filter:u32,header:[u8;12],previous:Vec<u8>,outcome:Outcome}
fn report(filter:u32,kind:&str,n:u64){crate::diagnostics::event(&format!("{{\"event\":\"png_filter_native\",\"filter\":{filter},\"kind\":\"{kind}\",\"count\":{n}}}"));}
fn equal(a:CpuContext,b:CpuContext)->bool{a.regs==b.regs&&a.extregs==b.extregs&&a.cpsr==b.cpsr&&a.fpscr==b.fpscr}
fn original(env:&mut Environment){let sp=env.cpu.regs()[13];let lr=env.cpu.regs()[14];env.libc_state.peggle_png_filter.original.unwrap().call_without_pushing_stack_frame(env);env.mem.write(MutPtr::<u32>::from_bits(sp-4),lr);}
fn fallback(env:&mut Environment){let sp=env.cpu.regs()[13];let ip=env.cpu.regs()[12];env.mem.write(MutPtr::<u32>::from_bits(sp-4),ip);env.cpu.regs_mut()[12]=2;}
fn finish(env:&mut Environment){let sp=env.cpu.regs()[13];let ip=env.cpu.regs()[12];let cpsr=env.cpu.cpsr();env.mem.write(MutPtr::<u32>::from_bits(sp-4),ip);env.mem.write(MutPtr::<u32>::from_bits(sp-8),cpsr);env.cpu.regs_mut()[12]=0;}
fn disable(env:&mut Environment,filter:u32,why:&str){let s=&mut env.libc_state.peggle_png_filter;s.disabled=true;let(a,p)=(s.address,s.prefix);env.mem.bytes_at_mut(MutPtr::<u8>::from_bits(a),8).copy_from_slice(&p);env.cpu.invalidate_cache_range(a,8);report(filter,why,0);}
fn division_ready(env:&Environment)->bool{env.mem.get_bytes_fallible(ConstPtr::<u8>::from_bits(0x2f7bec).cast(),8).is_some_and(|b|{let a=u32::from_le_bytes(b[..4].try_into().unwrap());a>=0xef000003&&a<0xef800000&&b[4..8]==0xe12fff1eu32.to_le_bytes()})}
fn inputs(env:&Environment,c:CpuContext)->Option<Inputs>{
 let sp=c.regs[13];if sp<176{return None;}let filter=u32::from_le_bytes(env.mem.get_bytes_fallible(ConstPtr::<u8>::from_bits(sp).cast(),4)?.try_into().unwrap());if filter>4{return None;}
 let header:[u8;12]=env.mem.get_bytes_fallible(ConstPtr::<u8>::from_bits(c.regs[1]).cast(),12)?.try_into().unwrap();let n=u32::from_le_bytes(header[4..8].try_into().unwrap());if n>65536{return None;}
 let row=if n==0{Vec::new()}else{env.mem.get_bytes_fallible(ConstPtr::<u8>::from_bits(c.regs[2]).cast(),n)?.to_vec()};let previous=if n==0{Vec::new()}else{env.mem.get_bytes_fallible(ConstPtr::<u8>::from_bits(c.regs[3]).cast(),n)?.to_vec()};let stack=env.mem.get_bytes_fallible(ConstPtr::<u8>::from_bits(sp-176).cast(),176)?;
 let outcome=kernel::calculate(c,filter,&header,row,&previous,stack)?;Some(Inputs{filter,header,previous,outcome})
}
fn compare(env:&mut Environment,before:CpuContext,input:&Inputs)->bool{
 original(env);let n=input.outcome.row.len()as u32;equal(env.cpu.visible_context(),input.outcome.context)&&env.mem.bytes_at(ConstPtr::<u8>::from_bits(before.regs[13]-176),176)==input.outcome.stack&&env.mem.bytes_at(ConstPtr::<u8>::from_bits(before.regs[1]),12)==input.header&&(n==0||env.mem.bytes_at(ConstPtr::<u8>::from_bits(before.regs[2]),n)==input.outcome.row)&&(n==0||env.mem.bytes_at(ConstPtr::<u8>::from_bits(before.regs[3]),n)==input.previous)&&env.mem.read(ConstPtr::<u32>::from_bits(before.regs[13]))==input.filter
}
fn corpus_gate(env:&mut Environment,filter:u32)->bool{
 let saved=env.cpu.visible_context();let Some(p)=env.mem.try_alloc(4096).map(|p|p.cast::<u8>())else{return false;};let a=p.to_bits();env.libc_state.peggle_png_filter.validating=true;let mut ok=true;let mut done=0;
 for n in 0..64u32{
  env.mem.bytes_at_mut(p,4096).fill(0xa5);let depth=[1,2,4,8,16,24,32,48,64][n as usize%9];let length=((depth+7)/8)+n%129;let header=a+16;let row=a+128;let prev=a+768;let sp=a+4000;env.mem.write(MutPtr::<u32>::from_bits(header+4),length);env.mem.write(MutPtr::<u8>::from_bits(header+11),depth as u8);env.mem.write(MutPtr::<u32>::from_bits(sp),filter);
  for j in 0..length{env.mem.write(MutPtr::<u8>::from_bits(row+j),(n*73+j*31)as u8);env.mem.write(MutPtr::<u8>::from_bits(prev+j),(n*17+j*91)as u8);}
  let mut c=saved;c.regs[0]=a;c.regs[1]=header;c.regs[2]=row;c.regs[3]=prev;c.regs[13]=sp;c.cpsr=(saved.cpsr&0x0fffffff)|((n&15)<<28);env.cpu.restore_visible_context(c);let Some(input)=inputs(env,c)else{ok=false;break;};if !compare(env,c,&input){ok=false;report(filter,"corpus_state_or_memory_mismatch_at",u64::from(n));break;}done+=1;
 }
 env.cpu.restore_visible_context(saved);env.mem.free(p.cast());env.libc_state.peggle_png_filter.validating=false;report(filter,if ok{"corpus_passed"}else{"corpus_failed"},done);ok
}
fn png_filter(env:&mut Environment){
 if env.libc_state.peggle_png_filter.disabled||env.libc_state.peggle_png_filter.validating{return fallback(env);}
 let before=env.cpu.visible_context();let Some(input)=inputs(env,before)else{let s=&mut env.libc_state.peggle_png_filter;s.fallback+=1;let n=s.fallback;if n==1||n%65536==0{report(255,"unsupported_contract_original_tail",n);}return fallback(env);};let f=input.filter as usize;
 if f==3&&!division_ready(env){report(3,"division_link_original_tail",0);return fallback(env);}
 if !env.libc_state.peggle_png_filter.entries[f].corpus{if !corpus_gate(env,input.filter){disable(env,input.filter,"disabled_after_corpus_rejection");return fallback(env);}env.libc_state.peggle_png_filter.entries[f].corpus=true;}
 let s=&mut env.libc_state.peggle_png_filter.entries[f];s.calls+=1;let shadow=s.shadows<64||s.calls%4096==0;
 if shadow{if !compare(env,before,&input){disable(env,input.filter,"disabled_after_real_input_mismatch");return finish(env);}let s=&mut env.libc_state.peggle_png_filter.entries[f];s.shadows+=1;if s.shadows==64{report(input.filter,"real_input_gate_passed",64);}return finish(env);}
 let n=input.outcome.row.len()as u32;if n!=0{env.mem.bytes_at_mut(MutPtr::<u8>::from_bits(before.regs[2]),n).copy_from_slice(&input.outcome.row);}env.mem.bytes_at_mut(MutPtr::<u8>::from_bits(before.regs[13]-176),176).copy_from_slice(&input.outcome.stack);env.cpu.restore_visible_context(input.outcome.context);let s=&mut env.libc_state.peggle_png_filter.entries[f];s.fast+=1;let n=s.fast;if n==1||n%1024==0{report(input.filter,"fast_rows",n);}finish(env);
}
fn restore_result_flags(env:&mut Environment){let saved=env.mem.read(ConstPtr::<u32>::from_bits(env.cpu.regs()[13]-8));let current=env.cpu.cpsr();env.cpu.set_cpsr((current&!0xf8000000)|(saved&0xf8000000));}
const FUNCTIONS:FunctionExports=&[export_c_func!(png_filter()),export_c_func!(restore_result_flags())];
pub(crate)fn install(verified:bool,bins:&[crate::mach_o::MachO],mem:&mut crate::mem::Mem,dyld:&mut crate::dyld::Dyld,libc:&mut crate::libc::State){
 use digest::Digest;if !verified||std::env::var("PEGGLE_HD_PNG_FILTER_NATIVE").as_deref()!=Ok("1"){return;}
 let address=0x28f828;let symbol="_png_read_filter_row";let hash="d802aa8f0ab8f1862450305c61f1dfd799d99df0";
 if bins[0].exported_symbols.get(symbol)!=Some(&(address|1))||format!("{:x}",sha1::Sha1::digest(mem.bytes_at(ConstPtr::<u8>::from_bits(address),0x2f0)))!=hash{report(255,"image_guard_rejected",0);return;}
 let prefix:[u8;8]=mem.bytes_at(ConstPtr::<u8>::from_bits(address),8).try_into().unwrap();let original=mem.alloc(20).cast::<u32>();let first12=mem.bytes_at(ConstPtr::<u8>::from_bits(address),12).to_vec();mem.bytes_at_mut(original.cast::<u8>(),12).copy_from_slice(&first12);mem.write(original+3,0xf000_f8dfu32);mem.write(original+4,(address+12)|1);
 let host=dyld.create_guest_function(mem,symbol,FUNCTIONS[0].1);let svc=mem.read(ConstPtr::<u32>::from_bits(host.addr_without_thumb_bit()));let flags_host=dyld.create_guest_function(mem,"HD_png_filter_restore_flags",FUNCTIONS[1].1);let flags_svc=mem.read(ConstPtr::<u32>::from_bits(flags_host.addr_without_thumb_bit()));let veneer=mem.alloc(60).cast::<u32>();for(i,w)in crate::png_filter_veneer::words(svc,flags_svc,original.to_bits()|1).into_iter().enumerate(){mem.write(veneer+i as u32,w);}
 libc.peggle_png_filter=State{original:Some(GuestFunction::from_addr_with_thumb_bit(original.to_bits()|1)),address,prefix,..State::default()};let p=MutPtr::<u32>::from_bits(address);mem.write(p,0xf000_f8dfu32);mem.write(p+1,veneer.to_bits());report(255,"guarded_hook_installed",0);
}
