/* MPL-2.0. Conservative process-lifetime accounting for the C51 experiment. */
//! Enabled only by validated Full Device startup. Never refund retained/shared
//! resources on wrapper drop: names may still be referenced by a layer/context.
//! Deliberately overcharges resized/recreated storage, requiring app restart
//! instead of risking unbounded retained allocations. Not a total RSS limit.
use std::sync::{Mutex,OnceLock,atomic::{AtomicBool,Ordering}};
use super::{GLES,gles11_raw as gl};
pub const MAX_PLANE:usize=32*1024*1024;
pub const MAX_PERSISTENT_GPU:usize=224*1024*1024;
pub const MAX_CPU:usize=160*1024*1024;
pub const MAX_TEMP_GPU:usize=32*1024*1024;
static ENABLED:AtomicBool=AtomicBool::new(false);
static BUDGET:OnceLock<Mutex<Ledger>>=OnceLock::new();
#[derive(Default,Debug,Clone,Copy,PartialEq,Eq)]pub struct Ledger{pub persistent:usize,pub cpu:usize,pub temporary:usize}
impl Ledger{
 pub fn charge(&mut self,gpu:usize,cpu:usize)->Result<(),&'static str>{
  let g=self.persistent.checked_add(gpu).ok_or("gpu_budget_overflow")?;
  let c=self.cpu.checked_add(cpu).ok_or("cpu_budget_overflow")?;
  if g>MAX_PERSISTENT_GPU||c>MAX_CPU{return Err("full_device_lifetime_budget");}
  self.persistent=g;self.cpu=c;Ok(())
 }
 pub fn temporary(&mut self,bytes:usize)->Result<(),&'static str>{let n=self.temporary.checked_add(bytes).ok_or("temporary_overflow")?;if n>MAX_TEMP_GPU{return Err("temporary_gpu_budget");}self.temporary=n;Ok(())}
}
#[derive(Clone,Copy,Debug)]pub struct Caps{pub texture:i32,pub renderbuffer:i32,pub viewport:[i32;2]}
pub fn rgba_bytes(w:i32,h:i32,max:i32)->Option<usize>{if w<=0||h<=0||max<=0||w>max||h>max{return None;}let n=(w as usize).checked_mul(h as usize)?.checked_mul(4)?;(n<=MAX_PLANE).then_some(n)}
pub fn validate(w:i32,h:i32,c:Caps)->Result<usize,&'static str>{let b=rgba_bytes(w,h,c.texture).ok_or("texture_or_plane_limit")?;if w>c.renderbuffer||h>c.renderbuffer||w>c.viewport[0]||h>c.viewport[1]{return Err("renderbuffer_or_viewport_limit");}Ok(b)}
pub unsafe fn caps(g:&mut dyn GLES)->Caps{let(mut t,mut r,mut v)=(0,0,[0;2]);g.GetIntegerv(gl::MAX_TEXTURE_SIZE,&mut t);g.GetIntegerv(gl::MAX_RENDERBUFFER_SIZE_OES,&mut r);g.GetIntegerv(gl::MAX_VIEWPORT_DIMS,v.as_mut_ptr());Caps{texture:t,renderbuffer:r,viewport:v}}
pub fn enabled()->bool{ENABLED.load(Ordering::Relaxed)}
fn ledger()->std::sync::MutexGuard<'static,Ledger>{BUDGET.get_or_init(||Mutex::new(Ledger::default())).lock().expect("Full Device budget poisoned")}
pub fn charge(gpu:usize,cpu:usize)->Result<(),&'static str>{if !enabled(){return Ok(());}let mut b=ledger();b.charge(gpu,cpu)?;crate::diagnostics::event(&format!("{{\"event\":\"full_device_budget\",\"persistent_gpu_bytes\":{},\"reserved_cpu_bytes\":{},\"refund_on_wrapper_drop\":false}}",b.persistent,b.cpu));Ok(())}
/// Host drawable plus one possible compositor texture are reserved up front.
/// Actual guest storage then reserves its RBO, one layer upload texture and
/// five CPU planes (retained cache + four simultaneous direct-gate vectors).
pub unsafe fn preflight(g:&mut dyn GLES,w:i32,h:i32,host_w:i32,host_h:i32)->Result<(),&'static str>{
 let c=caps(g);let target=validate(w,h,c)?;let host=validate(host_w,host_h,c)?;
 let mut proposed=Ledger::default();proposed.charge(host.checked_add(target).ok_or("startup_overflow")?,0)?;
 if g.GetError()!=gl::NO_ERROR{return Err("host_cap_query_gl_error");}
 if enabled(){return Err("full_device_already_configured");}
 *ledger()=proposed;ENABLED.store(true,Ordering::Relaxed);
 crate::diagnostics::event(&format!("{{\"event\":\"full_device_preflight\",\"texture_cap\":{},\"renderbuffer_cap\":{},\"viewport_cap\":[{},{}],\"target\":[{w},{h}],\"plane_bytes\":{target},\"copy_buffers\":1,\"persistent_gpu_limit\":{MAX_PERSISTENT_GPU},\"cpu_limit\":{MAX_CPU},\"temporary_gpu_limit\":{MAX_TEMP_GPU}}}",c.texture,c.renderbuffer,c.viewport[0],c.viewport[1]));Ok(())
}
pub unsafe fn drawable(g:&mut dyn GLES,w:i32,h:i32)->Result<(),&'static str>{if !enabled(){return Ok(());}let b=validate(w,h,caps(g))?;charge(b.checked_mul(2).ok_or("drawable_overflow")?,b.checked_mul(5).ok_or("oracle_overflow")?)}
pub unsafe fn texture(g:&mut dyn GLES,w:i32,h:i32)->Result<(),&'static str>{if !enabled(){return Ok(());}let b=validate(w,h,caps(g))?;charge(b,0)}
pub struct Temporary(usize);
pub fn temporary(bytes:usize)->Result<Temporary,&'static str>{if !enabled(){return Ok(Temporary(0));}ledger().temporary(bytes)?;Ok(Temporary(bytes))}
impl Drop for Temporary{fn drop(&mut self){if self.0!=0{let mut b=ledger();b.temporary=b.temporary.checked_sub(self.0).expect("temporary accounting");}}}
#[cfg(test)]pub fn fixture_reset(){ENABLED.store(false,Ordering::Relaxed);*ledger()=Ledger::default();}
#[cfg(test)]mod tests{use super::*;
 fn full()->Caps{Caps{texture:8192,renderbuffer:8192,viewport:[8192,8192]}}
 #[test]fn bounds_and_caps(){assert_eq!(validate(3200,2400,full()),Ok(30_720_000));assert_eq!(validate(2400,3200,full()),Ok(30_720_000));for(w,h)in[(0,1),(-1,2),(4096,2049),(i32::MAX,i32::MAX)]{assert!(validate(w,h,full()).is_err());}for c in[Caps{texture:2048,..full()},Caps{renderbuffer:2048,..full()},Caps{viewport:[3000,8192],..full()}]{assert!(validate(3200,2400,c).is_err());}}
 #[test]fn process_lifetime_budget_counts_all_allocations_and_fails_atomically(){let b=30_720_000;let mut l=Ledger::default();l.charge(2*b,0).unwrap();l.charge(2*b,5*b).unwrap();l.charge(b,0).unwrap();l.charge(b,0).unwrap();assert_eq!(l.persistent,6*b);l.temporary(b).unwrap();assert!(l.temporary(b).is_err());let previous=l;assert!(l.charge(2*b,5*b).is_err());assert_eq!(l,previous);assert!(l.charge(usize::MAX,0).is_err());assert_eq!(l,previous);}
}
