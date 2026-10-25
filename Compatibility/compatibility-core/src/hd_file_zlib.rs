/* This Source Code Form is subject to the Mozilla Public License, v. 2.0. */
//! Standard file inflate only, at the audited UncompressFileToBuffer callsite.
//! Shadow complete original streams before native ownership of later streams.
//! File I/O, loop/error decisions and unknown allocators remain original.
use crate::{Environment,mem::{ConstPtr,MutPtr},png_native_core::{Native,GuestStream,Action}};
use std::collections::HashMap;
#[derive(Default)]pub(crate)struct State{enabled:bool,disabled:bool,completed:u32,started:u64,streams:HashMap<u32,Stream>,events:u32,fast_calls:u64}
struct Pending{total_in:u32,total_out:u32,avail_out:u32,adler:u32,status:i32,output:u32,bytes:Vec<u8>}
struct Shadow{initializing:bool,native:Native,stream:MutPtr<GuestStream>,output:MutPtr<u8>,destination:u32,capacity:u32,pending:Option<Pending>,calls:u32,matched:bool,ended:bool}
enum Stream{Shadow(Shadow),Active(Native)}
fn event(env:&mut Environment,kind:&str,count:u64){let s=&mut env.libc_state.hd_file_zlib;if s.events<64||kind.contains("mismatch")||kind.contains("gate")||(count!=0&&count%1024==0){s.events=s.events.saturating_add(1);crate::diagnostics::event(&format!("{{\"event\":\"file_zlib_native\",\"kind\":\"{kind}\",\"count\":{count}}}"));}}
fn fallback(env:&mut Environment,args:[u32;4])->i32{env.cpu.regs_mut()[1..4].copy_from_slice(&args[1..4]);env.cpu.regs_mut()[12]=2;args[0]as i32}
fn frame_word(env:&Environment,p:MutPtr<GuestStream>,offset:u32)->Option<u32>{let base=p.to_bits().checked_sub(0x14)?;let a=base.checked_add(offset)?;env.mem.get_bytes_fallible(ConstPtr::<u8>::from_bits(a).cast(),4).map(|b|u32::from_le_bytes(b.try_into().unwrap()))}
fn errno(env:&mut Environment)->Option<(MutPtr<i32>,i32)>{let p=crate::libc::errno::try_errno_ptr(env)?;Some((p,env.mem.read(p)))}
fn restore_errno(env:&mut Environment,s:Option<(MutPtr<i32>,i32)>){if let Some((p,v))=s{env.mem.write(p,v);}}
fn close(env:&mut Environment,stream:Stream){match stream{Stream::Active(mut n)=>n.close(env),Stream::Shadow(mut s)=>{let old=errno(env);s.native.close(env);env.mem.free(s.stream.cast());env.mem.free(s.output.cast());restore_errno(env,old);}}}
pub(crate)fn contains_token(env:&Environment,token:u32)->bool{token!=0&&env.libc_state.hd_file_zlib.streams.values().any(|s|match s{Stream::Active(n)=>n.token.to_bits()==token,Stream::Shadow(s)=>s.native.token.to_bits()==token})}
pub(crate)fn owns(env:&Environment,p:MutPtr<GuestStream>)->bool{env.libc_state.hd_file_zlib.streams.contains_key(&p.to_bits())}
/// None leaves PNG/unknown-stream routing untouched. All format/API validation
/// precedes this in the common zlib shim; never clear unknown custom callbacks.
pub(crate)fn try_init(env:&mut Environment,p:MutPtr<GuestStream>,bits:i32,args:[u32;4])->Option<i32>{
 if !env.libc_state.hd_file_zlib.enabled||env.libc_state.hd_file_zlib.disabled||bits!=15||env.cpu.regs()[14]&!1!=0x26edf4||env.cpu.regs()[13].checked_add(0x14)!=Some(p.to_bits()){return None;}
 let g=env.mem.read(p);if g.zalloc!=0||g.zfree!=0||g.opaque!=0{return None;}
 let capacity=frame_word(env,p,0x50)?;let destination=frame_word(env,p,0x54)?;
 if capacity==0||capacity>8*1024*1024||destination==0||env.mem.get_bytes_fallible(ConstPtr::<u8>::from_bits(destination).cast(),capacity).is_none(){event(env,"bounds_fallback",0);return None;}
 let frame=p.to_bits()-0x14;let end=destination.checked_add(capacity)?;
 if !(end<=frame||destination>=frame.checked_add(0x60)?){return None;}
 if let Some(previous)=env.libc_state.hd_file_zlib.streams.remove(&p.to_bits()){match previous{Stream::Shadow(_)=>close(env,previous),active=>{env.libc_state.hd_file_zlib.streams.insert(p.to_bits(),active);return Some(-2);}}}
 if env.libc_state.hd_file_zlib.streams.len()>=4{event(env,"stream_limit_fallback",0);return None;}
 env.libc_state.hd_file_zlib.started+=1;
 if env.libc_state.hd_file_zlib.completed>=4&&env.libc_state.hd_file_zlib.started%64!=0{
  match Native::create_file(env,p,true,usize::MAX,usize::MAX){Ok(n)=>{env.libc_state.hd_file_zlib.streams.insert(p.to_bits(),Stream::Active(n));event(env,"known_file_stream_native_initialized",0);return Some(0);},Err(_)=>{event(env,"native_init_fallback",0);return None;}}
 }
 let saved_errno=errno(env);let Some(stream)=env.mem.try_alloc(56).map(|p|p.cast::<GuestStream>())else{return None;};env.mem.write(stream,GuestStream::default());
 let Some(output)=env.mem.try_alloc(capacity).map(|p|p.cast::<u8>())else{env.mem.free(stream.cast());return None;};
 let made=Native::create_file(env,stream,true,usize::MAX,usize::MAX);restore_errno(env,saved_errno);
 match made{Ok(native)=>{env.libc_state.hd_file_zlib.streams.insert(p.to_bits(),Stream::Shadow(Shadow{initializing:true,native,stream,output,destination,capacity,pending:None,calls:0,matched:true,ended:false}));event(env,"original_stream_shadow_started",0);Some(fallback(env,args))},Err(_)=>{env.mem.free(stream.cast());env.mem.free(output.cast());event(env,"shadow_init_fallback",0);None}}
}
fn compare_previous(env:&mut Environment,p:MutPtr<GuestStream>,s:&mut Shadow)->bool{
 let Some(e)=s.pending.take()else{return true;};let g=env.mem.read(p);let status=frame_word(env,p,0x4c).map(|v|v as i32);
 // The audited caller overwrites next_in/avail_in and next_out before the next
 // inflate call. Compare the persistent fields it preserves plus every output
 // byte and its stored return code. data_type/msg are never used by this caller.
 let same=g.total_in==e.total_in&&g.total_out==e.total_out&&g.avail_out==e.avail_out&&g.adler==e.adler&&status==Some(e.status)&&env.mem.get_bytes_fallible(ConstPtr::<u8>::from_bits(e.output).cast(),e.bytes.len()as u32).is_some_and(|v|v==e.bytes);
 s.ended=e.status==1;s.matched&=same;
 if !same{env.libc_state.hd_file_zlib.disabled=true;event(env,"original_stream_differential_mismatch",s.calls as u64);}
 same
}
fn inner(env:&mut Environment,p:MutPtr<GuestStream>,kind:u8,flush:i32,stream:&mut Stream,args:[u32;4])->(i32,bool){
 match stream{
  Stream::Shadow(s)=>{
   // The original inflateInit_ wrapper has24bytes of guest stack; its
   // inflateInit2_ callee has48more. That callee calls Reset internally.
   // It is still original-owned and must initialize its original state.
   let lr=env.cpu.regs()[14]&!1;
   if kind==1&&s.initializing&&s.calls==0&&s.pending.is_none()&&lr==0x28c046&&env.cpu.regs()[13].checked_add(0x5c)==Some(p.to_bits()){
    s.initializing=false;let n=env.libc_state.hd_file_zlib.started;event(env,"initial_reset_original_forwarded",n);
    return(fallback(env,args),false);
   }
   if !compare_previous(env,p,s){return(fallback(env,args),true);}
   if kind==2{
    if !s.initializing&&s.matched&&s.ended&&s.calls>0&&env.cpu.regs()[13].checked_add(0x14)==Some(p.to_bits())&&[0x26ee36,0x26ee90,0x26eea6].contains(&lr){
     // Recheck the entire completed original output, including chunks already
     // compared earlier, before counting a successful complete-stream gate.
     let g=env.mem.read(p);
     let identical=g.total_out<=s.capacity&&env.mem.bytes_at(ConstPtr::<u8>::from_bits(s.destination),g.total_out)==env.mem.bytes_at(s.output.cast_const(),g.total_out);
     if !identical{env.libc_state.hd_file_zlib.disabled=true;event(env,"original_stream_full_output_mismatch",s.calls as u64);return(fallback(env,args),true);}
     env.libc_state.hd_file_zlib.completed+=1;let n=env.libc_state.hd_file_zlib.completed;event(env,"complete_original_stream_gate_passed",u64::from(n));if n==4{event(env,"native_stream_gate_ready",4);}}
    return(fallback(env,args),true);
   }
   let g=env.mem.read(p);let lr=env.cpu.regs()[14]&!1;
   let mut reasons=0u32;
   if kind!=0{reasons|=1;}if flush!=0{reasons|=2;}if lr!=0x26ee6c{reasons|=4;}
   if env.cpu.regs()[13].checked_add(0x14)!=Some(p.to_bits()){reasons|=8;}
   if g.zalloc!=0x28bb99{reasons|=16;}if g.zfree!=0x28bbc9{reasons|=32;}if g.opaque!=0{reasons|=64;}
   if g.avail_in>4096{reasons|=128;}if g.total_out>s.capacity||g.avail_out>s.capacity.saturating_sub(g.total_out){reasons|=256;}
   if s.destination.checked_add(g.total_out)!=Some(g.next_out){reasons|=512;}if s.initializing{reasons|=1024;}
   if reasons!=0{
    let n=env.libc_state.hd_file_zlib.started;
    if env.libc_state.hd_file_zlib.events<64{crate::diagnostics::event(&format!("{{\"event\":\"file_zlib_contract_rejection\",\"phase\":\"shadow\",\"kind\":{kind},\"reason_mask\":{reasons},\"lr\":{lr},\"initializing\":{}}}",s.initializing));}
    event(env,"shadow_contract_fallback",n);return(fallback(env,args),true);
   }
   let mut sg=env.mem.read(s.stream);sg.next_in=g.next_in;sg.avail_in=g.avail_in;sg.next_out=s.output.to_bits()+g.total_out;sg.avail_out=g.avail_out;env.mem.write(s.stream,sg);
   let saved_errno=errno(env);let action=s.native.step_with_flush(env,0);restore_errno(env,saved_errno);
   let Action::Return(status)=action else{event(env,"shadow_operation_fallback",0);return(fallback(env,args),true);};let after=env.mem.read(s.stream);let produced=after.total_out.saturating_sub(g.total_out);
   if produced>s.capacity-g.total_out{event(env,"shadow_size_fallback",0);return(fallback(env,args),true);}
   s.pending=Some(Pending{total_in:after.total_in,total_out:after.total_out,avail_out:after.avail_out,adler:after.adler,status,output:g.next_out,bytes:env.mem.bytes_at((s.output+g.total_out).cast_const(),produced).to_vec()});s.calls+=1;
   (fallback(env,args),false)
  },
  Stream::Active(n)=>{
   let lr=env.cpu.regs()[14]&!1;
   if env.cpu.regs()[13].checked_add(0x14)!=Some(p.to_bits())||kind==1||kind==0&&(lr!=0x26ee6c||flush!=0)||kind==2&&![0x26ee36,0x26ee90,0x26eea6].contains(&lr){event(env,"unsupported_active_file_contract",0);return(-2,false);}
   let action=if kind==2{if n.compatible(env){n.close(env);Action::Return(0)}else{Action::UnsupportedAcceleratedState}}else{n.step_with_flush(env,flush)};
   let code=match action{Action::Return(v)=>v,Action::DeferredPngAllocationError(_)=>-4,_=>-2};
   if kind==0{let state=&mut env.libc_state.hd_file_zlib;state.fast_calls+=1;let count=state.fast_calls;if count==1||count%1024==0{event(env,"fast_inflate_operations",count);}}
   (code,kind==2&&!n.active)
  }
 }
}
pub(crate)fn operate(env:&mut Environment,p:MutPtr<GuestStream>,flush:i32,kind:u8,args:[u32;4])->i32{
 let mut stream=env.libc_state.hd_file_zlib.streams.remove(&p.to_bits()).unwrap();
 let outcome=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||inner(env,p,kind,flush,&mut stream,args)));
 match outcome{Ok((code,done))=>{if done{close(env,stream);}else{env.libc_state.hd_file_zlib.streams.insert(p.to_bits(),stream);}code},Err(payload)=>{env.libc_state.hd_file_zlib.streams.insert(p.to_bits(),stream);std::panic::resume_unwind(payload)}}
}
pub(crate)fn drain(env:&mut Environment){if env.mem.is_null()||env.libc_state.is_null(){return;}let streams=std::mem::take(&mut env.libc_state.hd_file_zlib.streams);for(_,s)in streams{close(env,s);}}
pub(crate)fn configure(verified:bool,bins:&[crate::mach_o::MachO],mem:&crate::mem::Mem,libc:&mut crate::libc::State){use digest::Digest;
 if !verified||std::env::var("PEGGLE_HD_FILE_ZLIB_NATIVE").as_deref()!=Ok("1"){return;}
 let guards=[("_inflate_blocks_reset",0x289990,0xa0,"86a3e57f5591f0d4741d653c3890e06bc79a1570"),("__ZL22UncompressFileToBufferP7__sFILEPci",0x26eda4,0x128,"62a98ae9d33d2eccb6414a2ca50707a4cc997b17"),("_zcalloc",0x28bb98,0x30,"eef250ab3d3664f1660c69826548430fc53e1835"),("_zcfree",0x28bbc8,0x24,"501d892c0c4b1b7fd728c4a1673bc4f08fbeb53b")];
 let valid=guards.iter().all(|(name,a,n,h)|bins[0].exported_symbols.get(*name)==Some(&(a|1))&&format!("{:x}",sha1::Sha1::digest(mem.bytes_at(ConstPtr::<u8>::from_bits(*a),*n)))==*h);
 libc.hd_file_zlib.enabled=valid;crate::diagnostics::event(&format!("{{\"event\":\"file_zlib_native\",\"kind\":\"{}\",\"count\":0}}",if valid{"caller_and_allocator_guards_passed"}else{"image_guard_rejected"}));
}

/// Read-only anchor: at least one whole file stream has completed original
/// decoder/full-output comparison, before optional inner-loop acceleration.
pub(crate)fn interpreted_stream_anchor(e:&Environment)->bool{e.libc_state.hd_file_zlib.completed>=1&&!e.libc_state.hd_file_zlib.disabled}
