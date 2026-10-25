//! C35 changes rendering cadence only. Original100Hz simulation remains intact.
use std::sync::atomic::{AtomicBool,Ordering};
static ENABLED:AtomicBool=AtomicBool::new(false);
#[cfg(target_os="ios")]unsafe extern "C" {fn peggle_pacing_configure(value:i32);fn peggle_pacing_target(loading:i32)->i32;fn peggle_pacing_sample(seconds:f64,loading:i32);fn peggle_pacing_observed()->f64;fn peggle_pacing_composition(seconds:f64,loading:i32);fn peggle_pacing_begin();fn peggle_pacing_wait(credit:f64);fn peggle_pacing_status(out:*mut Status);}
pub(crate)fn configure(verified:bool){let enabled=verified&&std::env::var("PEGGLE_HD_ADAPTIVE_PRESENT").as_deref()==Ok("1");ENABLED.store(enabled,Ordering::Relaxed);#[cfg(target_os="ios")]unsafe{peggle_pacing_configure(i32::from(enabled));}}
pub(crate)fn target(loading:bool)->Option<u32>{if !ENABLED.load(Ordering::Relaxed){return None;}#[cfg(target_os="ios")]unsafe{let hz=peggle_pacing_target(i32::from(loading)).clamp(30,120)as u32;report(loading);return Some(hz);}#[cfg(not(target_os="ios"))]Some(if loading{60}else{120})}
pub(crate)fn sample(seconds:f64,loading:bool){#[cfg(target_os="ios")]unsafe{peggle_pacing_sample(seconds,i32::from(loading));}}
pub(crate)fn observed()->f64{#[cfg(target_os="ios")]unsafe{return peggle_pacing_observed();}#[cfg(not(target_os="ios"))]0.0}

/// Read-only original UpdateFrames counter, guarded by the unchanged image
/// identity and either original application vtable. Never logged as an address.
pub(crate)fn simulation_counter(env:&crate::Environment)->Option<(u32,u32)>{
 use crate::mem::ConstPtr;
 if !ENABLED.load(Ordering::Relaxed)||!env.mem.is_mapped_range(0x38647c,4){return None;}
 let app=env.mem.read(ConstPtr::<u32>::from_bits(0x38647c));
 if !env.mem.is_mapped_range(app,160){return None;}
 let vtable=env.mem.read(ConstPtr::<u32>::from_bits(app));
 if vtable!=0x351e88&&vtable!=0x355a98{return None;}
 Some((app,env.mem.read(ConstPtr::<u32>::from_bits(app+0x9c))))
}

/// Inclusive compositor/native-swap cost, combined with the next guest render
/// callback for conservative workload fallback. No extra callback/timer.
pub(crate)struct CompositionWork{began:Option<std::time::Instant>,loading:bool}
impl CompositionWork{pub fn begin(enabled:bool,loading:bool)->Self{Self{began:enabled.then(std::time::Instant::now),loading}}}
impl Drop for CompositionWork{fn drop(&mut self){if let Some(start)=self.began{let elapsed=start.elapsed().as_secs_f64();#[cfg(target_os="ios")]unsafe{peggle_pacing_composition(elapsed,i32::from(self.loading));}}}}

#[repr(C)]#[derive(Default)]struct Status{enabled:i32,allowed:i32,maximum:i32,low_power:i32,thermal:i32,active:i32,loading:i32,target:i32,reason:i32,hold_reason:i32,bad:i32,hold_remaining:f64,game_wall:f64,work:f64,wait:f64,composition:f64}
fn reason(code:i32)->&'static str{match code{1=>"normal",2=>"feature_disabled",3=>"user30",4=>"hardware",5=>"low_power",6=>"thermal_serious",7=>"thermal_critical",8=>"inactive",9=>"startup",10=>"recovery",11=>"sustained_workload",12=>"isolated_stall",13=>"repeated_stalls",_=>"unknown"}}
#[cfg(target_os="ios")]unsafe fn report(loading:bool){
 static LAST:std::sync::Mutex<Option<(std::time::Instant,i32,i32)>>=std::sync::Mutex::new(None);
 let mut s=Status::default();peggle_pacing_status(&mut s);let now=std::time::Instant::now();let mut last=LAST.lock().unwrap();if last.is_some_and(|(t,h,r)|h==s.target&&r==s.reason&&now.duration_since(t).as_secs_f64()<5.0){return;}*last=Some((now,s.target,s.reason));drop(last);
 crate::diagnostics::event(&format!("{{\"event\":\"adaptive_pacing_status\",\"target_hz\":{},\"reason\":\"{}\",\"loading\":{loading},\"hardware_max_hz\":{},\"low_power\":{},\"thermal\":{},\"active\":{},\"user_high_refresh\":{},\"hold_remaining_s\":{:.3},\"game_wall_ms\":{:.4},\"credited_limiter_wait_ms\":{:.4},\"composition_ms\":{:.4},\"estimated_work_ms\":{:.4},\"timing_scope\":\"last_completed_callback\",\"bad_callbacks\":{}}}",s.target,reason(s.reason),s.maximum,s.low_power!=0,s.thermal,s.active!=0,s.allowed!=0,s.hold_remaining,s.game_wall*1000.0,s.wait*1000.0,s.composition*1000.0,s.work*1000.0,s.bad));
}
pub(crate)fn begin_callback(){#[cfg(target_os="ios")]unsafe{peggle_pacing_begin();}}
/// Credit only the actually elapsed REQUESTED wait. Oversleep/backpressure
/// remains workload. No guest deadline, clock, simulation or audio change.
pub(crate)fn deliberate_wait(env:&mut crate::Environment,duration:std::time::Duration,adaptive:bool){let start=std::time::Instant::now();env.sleep(duration);if adaptive{let credit=start.elapsed().min(duration).as_secs_f64();#[cfg(target_os="ios")]unsafe{peggle_pacing_wait(credit);}}}

pub(crate)fn reason_code()->u32{#[cfg(target_os="ios")]unsafe{let mut s=Status::default();peggle_pacing_status(&mut s);return s.reason.max(0)as u32;}#[cfg(not(target_os="ios"))]0}

pub(crate)fn enabled()->bool{ENABLED.load(Ordering::Relaxed)}
