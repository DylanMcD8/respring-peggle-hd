/* This Source Code Form is subject to the Mozilla Public License, v. 2.0. */
//! Experimental known-PNG native engine; opt-in HD shim owns all image guards.
use std::ffi::{c_char,c_int,c_uint,c_ulong,c_void};
#[repr(C)]pub(crate) struct ZStream {next_in:*const u8,avail_in:c_uint,total_in:c_ulong,next_out:*mut u8,avail_out:c_uint,total_out:c_ulong,msg:*const c_char,state:*mut c_void,zalloc:Option<unsafe extern "C" fn(*mut c_void,c_uint,c_uint)->*mut c_void>,zfree:Option<unsafe extern "C" fn(*mut c_void,*mut c_void)>,opaque:*mut c_void,data_type:c_int,adler:c_ulong,reserved:c_ulong}
#[link(name="z")]extern "C"{fn zlibVersion()->*const c_char;fn inflateInit2_(s:*mut ZStream,bits:c_int,v:*const c_char,n:c_int)->c_int;fn inflate(s:*mut ZStream,flush:c_int)->c_int;fn inflateReset(s:*mut ZStream)->c_int;fn inflateEnd(s:*mut ZStream)->c_int;}
// The audited file caller requires the embedded 1.1.3 decoder's exact
// per-call output boundaries. All C globals are isolated from system zlib.
extern "C" {
 fn peggle113_zlibVersion()->*const c_char;
 fn peggle113_inflateInit2_(s:*mut ZStream,bits:c_int,v:*const c_char,n:c_int)->c_int;
 fn peggle113_inflate(s:*mut ZStream,flush:c_int)->c_int;
 fn peggle113_inflateReset(s:*mut ZStream)->c_int;
 fn peggle113_inflateEnd(s:*mut ZStream)->c_int;
}
use crate::{Environment,mem,libc};
use crate::mem::{ConstPtr,MutPtr,MutVoidPtr,SafeRead};
#[repr(C)]#[derive(Clone,Copy,Default,Debug,PartialEq,Eq)]pub(crate) struct GuestStream{pub(crate) next_in:u32,pub(crate) avail_in:u32,pub(crate) total_in:u32,pub(crate) next_out:u32,pub(crate) avail_out:u32,pub(crate) total_out:u32,pub(crate) msg:u32,pub(crate) state:u32,pub(crate) zalloc:u32,pub(crate) zfree:u32,pub(crate) opaque:u32,pub(crate) data_type:i32,pub(crate) adler:u32,pub(crate) reserved:u32}
unsafe impl SafeRead for GuestStream{}
const PNG_ALLOC:u32=0x290dd9;const PNG_FREE:u32=0x290e65;
#[derive(Clone,Copy,Debug,PartialEq,Eq)]pub(crate) enum Failure{Allocation,CallbackPanic,InvalidFree,OutsideOperation,Overflow}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]pub(crate) enum Action{Return(i32),OriginalGuestFallback,DeferredPngAllocationError(Failure),UnsupportedAcceleratedState}
#[derive(Clone,Copy)]pub(crate) struct Entry{pub(crate) guest:u32,pub(crate) host:*mut c_void,pub(crate) size:u32,pub(crate) live:bool}
impl Default for Entry{fn default()->Self{Self{guest:0,host:std::ptr::null_mut(),size:0,live:false}}}
pub(crate) struct Allocator{pub(crate) mem:*mut mem::Mem,pub(crate) entries:[Entry;8],pub(crate) first_failure:Option<Failure>,pub(crate) clear_errno:bool,pub(crate) allocations:usize,pub(crate) frees:usize,pub(crate) limit:usize,pub(crate) panic_at:usize}
impl Allocator{
 fn new(limit:usize,panic_at:usize)->Self{Self{mem:std::ptr::null_mut(),entries:[Entry::default();8],first_failure:None,clear_errno:false,allocations:0,frees:0,limit,panic_at}}
 fn fail(&mut self,f:Failure){if self.first_failure.is_none(){self.first_failure=Some(f)}}
 pub(crate) fn allocate(&mut self,n:u32,size:u32)->*mut c_void{
  if self.mem.is_null(){self.fail(Failure::OutsideOperation);return std::ptr::null_mut()}
  self.allocations+=1;
  if self.allocations==self.panic_at{panic!("synthetic allocator failure before guest heap mutation")}
  let Some(bytes)=n.checked_mul(size)else{self.fail(Failure::Overflow);return std::ptr::null_mut()};
  if bytes==0{return std::ptr::null_mut()}
  self.clear_errno=true;
  let slot=self.entries.iter().position(|e|!e.live);
  if self.allocations>self.limit || slot.is_none(){self.fail(Failure::Allocation);return std::ptr::null_mut()}
  let mem=unsafe{&mut*self.mem};let Some(guest)=mem.try_alloc(bytes)else{self.fail(Failure::Allocation);return std::ptr::null_mut()};mem.bytes_at_mut(guest.cast::<u8>(),bytes).fill(0);
  let host=unsafe{mem.direct_memory_access_ptr().cast::<u8>().add(guest.to_bits()as usize).cast::<c_void>()};
  self.entries[slot.unwrap()]=Entry{guest:guest.to_bits(),host,size:bytes,live:true};host
 }
 pub(crate) fn release(&mut self,p:*mut c_void){
  if self.mem.is_null(){self.fail(Failure::OutsideOperation);return}
  self.frees+=1;if p.is_null(){return}self.clear_errno=true;
  let Some(slot)=self.entries.iter().position(|e|e.live&&e.host==p)else{self.fail(Failure::InvalidFree);return};
  unsafe{(&mut*self.mem).free(MutVoidPtr::from_bits(self.entries[slot].guest))};self.entries[slot].live=false;
 }
 pub(crate) fn live(&self)->usize{self.entries.iter().filter(|e|e.live).count()}
}
pub(crate) unsafe extern "C" fn guest_allocate(opaque:*mut c_void,n:c_uint,size:c_uint)->*mut c_void{
 if opaque.is_null(){return std::ptr::null_mut()}
 let ctx=unsafe{&mut*opaque.cast::<Allocator>()};match std::panic::catch_unwind(std::panic::AssertUnwindSafe(||ctx.allocate(n,size))){Ok(p)=>p,Err(_)=>{ctx.fail(Failure::CallbackPanic);std::ptr::null_mut()}}
}
pub(crate) unsafe extern "C" fn guest_release(opaque:*mut c_void,p:*mut c_void){if opaque.is_null(){return}let ctx=unsafe{&mut*opaque.cast::<Allocator>()};if std::panic::catch_unwind(std::panic::AssertUnwindSafe(||ctx.release(p))).is_err(){ctx.fail(Failure::CallbackPanic)}}
struct ActiveScope(*mut Allocator);
impl Drop for ActiveScope{fn drop(&mut self){unsafe{(*self.0).mem=std::ptr::null_mut()}}}
pub(crate) struct Native{pub(crate) z:Box<ZStream>,pub(crate) allocator:Box<Allocator>,pub(crate) active:bool,pub(crate) token:MutPtr<u8>,pub(crate) guest:MutPtr<GuestStream>,pub(crate) poisoned:bool,pub(crate) opaque:u32,pub(crate) standard_file:bool}
impl Native{
 pub(crate) fn compatible(&self,env:&Environment)->bool{let g=env.mem.read(self.guest);g.state==self.token.to_bits()&&g.zalloc==if self.standard_file{0x28bb99}else{PNG_ALLOC}&&g.zfree==if self.standard_file{0x28bbc9}else{PNG_FREE}&&g.opaque==self.opaque}

 pub(crate) fn operation(&mut self,env:&mut Environment,f:impl FnOnce(&mut ZStream)->i32)->(i32,Option<Failure>){
  assert!(self.allocator.mem.is_null());self.allocator.first_failure=None;self.allocator.clear_errno=false;
  let errno=libc::errno::try_errno_ptr(env);
  self.allocator.mem=&mut*env.mem;
  let active=ActiveScope(&mut*self.allocator);
  // No guest execution/yield and no held Environment field borrow inside C.
  let code=f(&mut self.z);
  drop(active);
  if self.allocator.clear_errno{if let Some(p)=errno{env.mem.write(p,0i32)}}
  (code,self.allocator.first_failure)
 }
 pub(crate) fn create(env:&mut Environment,guest:MutPtr<GuestStream>,verified:bool,limit:usize,panic_at:usize)->Result<Self,Action>{
  Self::create_mode(env,guest,verified,limit,panic_at,false)
 }
 /// Only the separately image/caller-guarded standard file adapter uses this.
 pub(crate) fn create_file(env:&mut Environment,guest:MutPtr<GuestStream>,verified:bool,limit:usize,panic_at:usize)->Result<Self,Action>{
  Self::create_mode(env,guest,verified,limit,panic_at,true)
 }
 fn create_mode(env:&mut Environment,guest:MutPtr<GuestStream>,verified:bool,limit:usize,panic_at:usize,standard_file:bool)->Result<Self,Action>{
  let g=env.mem.read(guest);
  if !verified||if standard_file{g.zalloc!=0||g.zfree!=0||g.opaque!=0}else{g.zalloc!=PNG_ALLOC||g.zfree!=PNG_FREE||g.opaque==0}{return Err(Action::OriginalGuestFallback)}
  if !standard_file&&g.state!=0{return Err(Action::UnsupportedAcceleratedState)}
  if libc::errno::try_errno_ptr(env).is_none(){return Err(Action::DeferredPngAllocationError(Failure::Allocation))}
  let Some(token)=env.mem.try_alloc(260).map(|p|p.cast::<u8>())else{return Err(Action::DeferredPngAllocationError(Failure::Allocation))};let mut allocator=Box::new(Allocator::new(limit,panic_at));let mut z:Box<ZStream>=Box::new(unsafe{std::mem::zeroed()});z.zalloc=Some(guest_allocate);z.zfree=Some(guest_release);z.opaque=(&mut*allocator as*mut Allocator).cast();
  let mut s=Self{z,allocator,active:false,token,guest,poisoned:false,opaque:g.opaque,standard_file};let(code,error)=s.operation(env,|z|unsafe{if standard_file{peggle113_inflateInit2_(z,15,peggle113_zlibVersion(),std::mem::size_of::<ZStream>()as i32)}else{inflateInit2_(z,15,zlibVersion(),std::mem::size_of::<ZStream>()as i32)}});s.active=code==0;
  if let Some(f)=error{s.close(env);return Err(Action::DeferredPngAllocationError(f))}
  if code!=0{s.close(env);return Err(Action::Return(code))}
  let mut g=g;if standard_file{g.zalloc=0x28bb99;g.zfree=0x28bbc9;}g.state=token.to_bits();g.total_in=0;g.total_out=0;g.msg=0;g.adler=s.z.adler as u32;g.data_type=s.z.data_type;env.mem.write(guest,g);Ok(s)
 }
 pub(crate) fn step(&mut self,env:&mut Environment)->Action{self.step_with_flush(env,0)}
 pub(crate) fn step_with_flush(&mut self,env:&mut Environment,flush:i32)->Action{
  let mut g=env.mem.read(self.guest);
  if self.poisoned||!self.active||!self.compatible(env){return Action::UnsupportedAcceleratedState}
  if g.next_out==0 || (g.next_in==0 && g.avail_in!=0){return Action::Return(-2)}
  let input_len=g.avail_in.min(1024*1024);let output_len=g.avail_out.min(8*1024*1024);
  let input=if input_len==0{Vec::new()}else{let Some(bytes)=env.mem.get_bytes_fallible(ConstPtr::<u8>::from_bits(g.next_in).cast(),input_len)else{return Action::Return(-2)};bytes.to_vec()};
  if env.mem.get_bytes_fallible(ConstPtr::<u8>::from_bits(g.next_out).cast(),output_len).is_none(){return Action::Return(-2)}
  let mut output=vec![0u8;output_len as usize];
  self.z.next_in=input.as_ptr();self.z.avail_in=input_len;self.z.next_out=output.as_mut_ptr();self.z.avail_out=output_len;
  let file=self.standard_file;let(code,error)=self.operation(env,|z|unsafe{if file{peggle113_inflate(z,flush)}else{inflate(z,flush)}});
  self.z.next_in=std::ptr::null();self.z.next_out=std::ptr::null_mut();
  if let Some(f)=error{self.poisoned=true;return Action::DeferredPngAllocationError(f)}
  let consumed=input_len-self.z.avail_in;let produced=output_len-self.z.avail_out;
  env.mem.bytes_at_mut(MutPtr::<u8>::from_bits(g.next_out),produced).copy_from_slice(&output[..produced as usize]);g.next_in+=consumed;g.avail_in-=consumed;g.next_out+=produced;g.avail_out-=produced;g.total_in=self.z.total_in as u32;g.total_out=self.z.total_out as u32;g.adler=self.z.adler as u32;g.data_type=self.z.data_type;g.msg=0;if !self.z.msg.is_null(){let message=unsafe{std::ffi::CStr::from_ptr(self.z.msg)}.to_bytes();let n=message.len().min(255);let to=self.token+4;env.mem.bytes_at_mut(to,n as u32+1)[..n].copy_from_slice(&message[..n]);env.mem.write(to+n as u32,0u8);g.msg=to.to_bits();}env.mem.write(self.guest,g);Action::Return(code)
 }
 pub(crate) fn reset(&mut self,env:&mut Environment)->Action{if self.poisoned||!self.active||!self.compatible(env){return Action::UnsupportedAcceleratedState}let file=self.standard_file;let(code,error)=self.operation(env,|z|unsafe{if file{peggle113_inflateReset(z)}else{inflateReset(z)}});if let Some(f)=error{return Action::DeferredPngAllocationError(f)}if code==0{let mut g=env.mem.read(self.guest);g.total_in=0;g.total_out=0;g.adler=self.z.adler as u32;g.data_type=self.z.data_type;g.msg=0;env.mem.write(self.guest,g)}Action::Return(code)}
 pub(crate) fn close(&mut self,env:&mut Environment){if self.active{let file=self.standard_file;let(code,error)=self.operation(env,|z|unsafe{if file{peggle113_inflateEnd(z)}else{inflateEnd(z)}});assert_eq!(code,0);assert_eq!(error,None);self.active=false;}assert_eq!(self.allocator.live(),0);assert!(self.allocator.mem.is_null());if !self.token.is_null(){let token=self.token;self.token=MutPtr::null();let mut g=env.mem.read(self.guest);if g.state==token.to_bits(){g.state=0;g.msg=0;env.mem.write(self.guest,g)}env.mem.free(token.cast());}}
}
impl Drop for Native{fn drop(&mut self){assert!(!self.active,"must end native streams before dropping guest Mem");assert!(self.token.is_null());}}
