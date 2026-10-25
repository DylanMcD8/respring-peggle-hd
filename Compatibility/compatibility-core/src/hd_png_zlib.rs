/* This Source Code Form is subject to the Mozilla Public License, v. 2.0. */
//! Experimental HD PNG adapter. Requires explicit launch opt-in and exact image.
//! No guest code is invoked from C. Guest-side veneers handle error/fallback tails.
use crate::{Environment,mem::{MutPtr,ConstPtr},png_native_core::{Native,GuestStream,Action}};
use crate::dyld::{export_c_func,FunctionExports};
use std::collections::HashMap;
#[derive(Default)]pub(crate) struct State{streams:HashMap<u32,Native>,enabled:bool,events:u32}
fn event(env:&mut Environment,kind:&str){let state=&mut env.libc_state.hd_png_zlib;if state.events<32{state.events+=1;crate::diagnostics::event(&format!("{{\"event\":\"hd_png_native\",\"kind\":\"{kind}\"}}"));}}
fn begin(env:&mut Environment)->[u32;4]{let args=env.cpu.regs()[..4].try_into().unwrap();env.cpu.regs_mut()[12]=0;args}
fn fallback(env:&mut Environment,args:[u32;4])->i32{env.cpu.regs_mut()[1..4].copy_from_slice(&args[1..4]);env.cpu.regs_mut()[12]=2;args[0]as i32}
fn result(env:&mut Environment,action:Action,opaque:u32,args:[u32;4])->i32{match action{
 Action::Return(code)=>code,
 Action::OriginalGuestFallback=>fallback(env,args),
 Action::UnsupportedAcceleratedState=>{event(env,"unsupported_native_state");-2},
 Action::DeferredPngAllocationError(_)=>{event(env,"deferred_png_allocation_error");env.cpu.regs_mut()[1]=0x33877a;env.cpu.regs_mut()[12]=1;opaque as i32}
}}
fn init(env:&mut Environment,p:MutPtr<GuestStream>,bits:i32,version:ConstPtr<u8>,size:i32,args:[u32;4])->i32{
 if version.is_null()||size!=56||env.mem.get_bytes_fallible(version.cast(),1).is_none_or(|b|b[0]!=b'1'){return -6}
 if p.is_null(){return -2}
 let Some(bytes)=env.mem.get_bytes_fallible(p.cast_const().cast(),56)else{return -2};let _=bytes;
 if let Some(code)=crate::hd_file_zlib::try_init(env,p,bits,args){return code;}
 let g=env.mem.read(p);
 // This callback identity and embedded-stream relationship are HD-specific.
 if !env.libc_state.hd_png_zlib.enabled||bits!=15||g.opaque.checked_add(0xa4)!=Some(p.to_bits()){return fallback(env,args)}
 if env.libc_state.hd_png_zlib.streams.contains_key(&p.to_bits()){return -2}
 if g.state!=0||env.libc_state.hd_png_zlib.streams.len()>=128{return fallback(env,args)}
 match Native::create(env,p,true,usize::MAX,usize::MAX){
  Ok(s)=>{env.libc_state.hd_png_zlib.streams.insert(p.to_bits(),s);event(env,"known_png_stream_native_initialized");0},
  Err(action)=>result(env,action,g.opaque,args)
 }
}
fn hd_init(env:&mut Environment,p:MutPtr<GuestStream>,version:ConstPtr<u8>,size:i32)->i32{let args=begin(env);init(env,p,15,version,size,args)}
fn hd_init2(env:&mut Environment,p:MutPtr<GuestStream>,bits:i32,version:ConstPtr<u8>,size:i32)->i32{let args=begin(env);init(env,p,bits,version,size,args)}
fn operate(env:&mut Environment,p:MutPtr<GuestStream>,flush:i32,kind:u8,args:[u32;4])->i32{
 if crate::hd_file_zlib::owns(env,p){return crate::hd_file_zlib::operate(env,p,flush,kind,args);}
 if p.is_null()||env.mem.get_bytes_fallible(p.cast_const().cast(),56).is_none(){return -2}
 if crate::hd_file_zlib::contains_token(env,env.mem.read(p).state){return -2;}
 let Some(mut stream)=env.libc_state.hd_png_zlib.streams.remove(&p.to_bits())else{
  let state=env.mem.read(p).state;
  if env.libc_state.hd_png_zlib.streams.values().any(|s|s.token.to_bits()==state){return -2}
  return fallback(env,args)
 };
 let opaque=env.mem.read(p).opaque;
 // Reinsert ownership before any Rust unwind reaches Environment teardown.
 let outcome=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||match kind{
  0=>stream.step_with_flush(env,flush),1=>stream.reset(env),_=>{if !stream.compatible(env){Action::UnsupportedAcceleratedState}else{stream.close(env);Action::Return(0)}}
 }));
 if stream.active||outcome.is_err(){env.libc_state.hd_png_zlib.streams.insert(p.to_bits(),stream);}else{drop(stream)}
 match outcome{Ok(action)=>result(env,action,opaque,args),Err(payload)=>std::panic::resume_unwind(payload)}
}
fn hd_inflate(env:&mut Environment,p:MutPtr<GuestStream>,flush:i32)->i32{let args=begin(env);operate(env,p,flush,0,args)}
fn hd_reset(env:&mut Environment,p:MutPtr<GuestStream>)->i32{let args=begin(env);operate(env,p,0,1,args)}
fn hd_end(env:&mut Environment,p:MutPtr<GuestStream>)->i32{let args=begin(env);operate(env,p,0,2,args)}
const FUNCTIONS:FunctionExports=&[export_c_func!(hd_init(_,_,_)),export_c_func!(hd_init2(_,_,_,_)),export_c_func!(hd_inflate(_,_)),export_c_func!(hd_reset(_)),export_c_func!(hd_end(_))];
/// Runs before ordinary Environment field drop, including coroutine cleanup.
pub(crate) fn drain(env:&mut Environment){
 if env.mem.is_null()||env.libc_state.is_null(){return}
 let mut streams=std::mem::take(&mut env.libc_state.hd_png_zlib.streams);
 for stream in streams.values_mut(){stream.close(env)}
}
pub(crate) fn install(verified:bool,bins:&[crate::mach_o::MachO],mem:&mut crate::mem::Mem,dyld:&mut crate::dyld::Dyld,libc:&mut crate::libc::State){
 use digest::Digest;
 if !verified||!(std::env::var("PEGGLE_HD_PNG_NATIVE").as_deref()==Ok("1")||std::env::var("PEGGLE_HD_FILE_ZLIB_NATIVE").as_deref()==Ok("1")){return}
 let valid=GUARDS.iter().all(|(name,address,size,sha)|bins[0].exported_symbols.get(*name)==Some(&(address|1))&&format!("{:x}",sha1::Sha1::digest(mem.bytes_at(ConstPtr::<u8>::from_bits(*address),*size)))==*sha)
  &&mem.bytes_at(ConstPtr::<u8>::from_bits(0x33877a),14)==b"Out of Memory\0";
 if !valid{crate::diagnostics::event("{\"event\":\"hd_png_native\",\"kind\":\"image_guard_rejected\"}");return}
 // First16-byte prefixes were individually audited: complete Thumb instructions,
 // no branch/call/PC-relative operand. No original executable files are patched.
 for(name,address,_,_)in &GUARDS[..5]{
  let prefix=mem.bytes_at(ConstPtr::<u8>::from_bits(*address),16).to_vec();let original=mem.alloc(24).cast::<u32>();mem.bytes_at_mut(original.cast::<u8>(),16).copy_from_slice(&prefix);mem.write(original+4,0xf000_f8dfu32);mem.write(original+5,(address+16)|1);
  let function=match *name{"_inflateInit_"=>FUNCTIONS[0].1,"_inflateInit2_"=>FUNCTIONS[1].1,"_inflate"=>FUNCTIONS[2].1,"_inflateReset"=>FUNCTIONS[3].1,_=>FUNCTIONS[4].1};
  let host=dyld.create_guest_function(mem,name,function);let svc=mem.read(MutPtr::<u32>::from_bits(host.addr_without_thumb_bit()));assert_eq!(svc&0xff000000,0xef000000);assert_eq!(svc&0x00800000,0);
  let veneer=mem.alloc(32).cast::<u32>();
  // SVC; CMP r12,1; LDREQ PC,error; CMP r12,2; LDREQ PC,original; BX LR.
  for(i,word) in crate::png_tail_veneer::words(svc,0x29094d,original.to_bits()|1).iter().enumerate(){mem.write(veneer+i as u32,*word)}
  let target=MutPtr::<u32>::from_bits(*address);mem.write(target,0xf000_f8dfu32);mem.write(target+1,veneer.to_bits());
 }
 libc.hd_png_zlib.enabled=std::env::var("PEGGLE_HD_PNG_NATIVE").as_deref()==Ok("1");
 crate::diagnostics::event(&format!("{{\"event\":\"zlib_hook_dispatch_configuration\",\"png_enabled\":{},\"file_enabled\":{}}}",libc.hd_png_zlib.enabled,std::env::var("PEGGLE_HD_FILE_ZLIB_NATIVE").as_deref()==Ok("1")));
 if libc.hd_png_zlib.enabled{crate::diagnostics::event("{\"event\":\"hd_png_native\",\"kind\":\"opt_in_guarded_hooks_installed\"}");}
}
const GUARDS:&[(&str,u32,u32,&str)]=&[
("_inflateInit_",0x28c058,0x28,"b8abf9430fb94d79bcaf1d65f880c4b98f5474f6"),
("_inflateInit2_",0x28be8c,0x1cc,"02d0f9a38f9d68cc4bac61b06733467ddc7a062e"),
("_inflate",0x28c080,0x530,"19e63af07784292ed0af42e9e6aac20a68d41d12"),
("_inflateReset",0x28bd7c,0x88,"51fd3780a7d1307a45c0ff9086863f1ddb7423c4"),
("_inflateEnd",0x28be04,0x88,"e8bd4f7a7fac0f8b721ef03cd34d38d03be949a1"),
("_png_zalloc",0x290dd8,0x8c,"d0d740380407c42ac5c7eb24add59333a1b52420"),
("_png_zfree",0x290e64,0x18,"d20859a17d3bd4a353535abea39181cd524e03aa"),
("_png_malloc",0x299508,0x54,"0df061c7724c8fe44257e38339da8fe85db90fb0"),
("_png_free",0x29955c,0x30,"18870f991f4d5e63394404c7c9e7b0a95c0d6f7e"),
("_png_error",0x29094c,0x38,"9f26a4ccf10cf97105437c1b205a8c0a55c4277c"),
("_png_default_error",0x290984,0x40,"33845c12f74c4eba8156cf259c715dbab862f51e"),
];
