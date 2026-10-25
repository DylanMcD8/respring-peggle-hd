/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. https://mozilla.org/MPL/2.0/ */
//! Bounded, process-owned diagnostic records. Durations are CPU-side elapsed
//! time, not GPU execution time. Inclusive scopes must not be added together.
use std::cell::RefCell;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::{atomic::{AtomicBool, Ordering}, Mutex, OnceLock};
use std::time::Instant;

static ENABLED: AtomicBool = AtomicBool::new(false);
static INTERPRETER_TIMING: AtomicBool = AtomicBool::new(false);
static PC_SAMPLING: AtomicBool = AtomicBool::new(false);
static HD_MARKERS: AtomicBool = AtomicBool::new(false);
static OUTPUT: OnceLock<Mutex<File>> = OnceLock::new();

pub fn initialize(path: &Path, enabled: bool) -> std::io::Result<()> {
    let file = OpenOptions::new().create(true).append(true).open(path)?;
    let _ = OUTPUT.set(Mutex::new(file));
    ENABLED.store(enabled, Ordering::Relaxed);
    // Detailed per-dispatch clocks are optional; coarse CPU/frame reports and
    // all safety/error events remain available when this timer is disabled.
    INTERPRETER_TIMING.store(enabled && std::env::var("PEGGLE_PERF_INTERPRETER_TIMING").as_deref() == Ok("1"), Ordering::Relaxed);
    event(&format!("{{\"event\":\"interpreter_timing_policy\",\"enabled\":{}}}", INTERPRETER_TIMING.load(Ordering::Relaxed)));
    PC_SAMPLING.store(enabled && std::env::var("PEGGLE_C53_PC_SAMPLING").as_deref()==Ok("1"),Ordering::Relaxed);
    event(&format!("{{\"event\":\"launch\",\"candidate\":53,\"diagnostics\":{enabled},\"pid\":{}}}", std::process::id()));
    Ok(())
}

pub fn event(json: &str) {
    if let Some(output) = OUTPUT.get() {
        if let Ok(mut file) = output.lock() {
            let _ = writeln!(file, "{json}");
        }
    }
}

pub fn panic_message(message: &str) {
    let mut escaped = String::new();
    for ch in message.chars().take(2048) {
        match ch {
            '"' => escaped.push_str("\\\""), '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"), '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            c if c < ' ' => escaped.push_str(&format!("\\u{:04x}", c as u32)),
            c => escaped.push(c),
        }
    }
    event(&format!("{{\"event\":\"panic\",\"message\":\"{escaped}\"}}"));
}

#[derive(Copy, Clone)]
pub enum Phase { Interpreter, GuestGl, Readback, Composition, Audio, Swap, Allocation, TextureUpload, ResourceRead, ImageDecode, GpuTransfer, PresentContext, GpuProducerContext, GpuCopyErrorsBefore, GpuCopyQueries, GpuCopySetup, GpuCopyCommand, GpuCopyFlush, GpuCopyRestoreErrors, HostContext, HostSharedFlush, GpuCopyUpstreamWait }
const NAMES: [&str; 22] = ["interpreter", "guest_gl", "readback", "composition_inclusive", "audio_inclusive", "swap_inclusive", "allocation", "texture_upload_inclusive", "resource_read_inclusive", "host_image_decode_inclusive", "gpu_transfer_inclusive", "present_context_and_rbo_query", "gpu_producer_context", "gpu_copy_prior_errors", "gpu_copy_state_queries", "gpu_copy_fbo_texture_setup", "gpu_copy_command", "gpu_copy_flush", "gpu_copy_restore_and_errors", "host_context", "host_shared_flush", "gpu_copy_upstream_finish"];

#[derive(Copy, Clone, Default)]
struct Timing { calls: u64, ns: u64, max_ns: u64 }
impl Timing {
    fn add(&mut self, ns: u64) { self.calls += 1; self.ns += ns; self.max_ns = self.max_ns.max(ns); }
}

pub struct Scope(Option<(Phase, Instant)>);
pub fn scope(phase: Phase) -> Scope {
    let enabled = ENABLED.load(Ordering::Relaxed) &&
        (!matches!(phase, Phase::Interpreter) || INTERPRETER_TIMING.load(Ordering::Relaxed));
    Scope(enabled.then(|| (phase, Instant::now())))
}
impl Drop for Scope {
    fn drop(&mut self) {
        if let Some((phase, start)) = self.0 {
            STATE.with(|s| { if let Ok(mut s) = s.try_borrow_mut() { s.timings[phase as usize].add(start.elapsed().as_nanos() as u64); } });
        }
    }
}

struct State {
    start: Instant, last_report: Instant, timings: [Timing; 22],
    last_frame: [Option<Instant>; 2], intervals: [Vec<f64>; 2],
    frame_counts: [u64; 2], alloc_bytes: u64, previous_cpu: f64,
    paused: Option<(Instant, f64)>,
    pc: std::collections::BTreeMap<(u32,bool,u8),(u64,u32,u32)>, pc_dropped:u64,
    resources:Vec<String>, marker_times:std::collections::BTreeMap<&'static str,Instant>,
}
impl Default for State {
    fn default() -> Self { Self { start: Instant::now(), last_report: Instant::now(), timings: [Timing::default();22], last_frame: [None;2], intervals: [Vec::with_capacity(1024),Vec::with_capacity(1024)], frame_counts: [0;2], alloc_bytes:0, previous_cpu:0.0, paused:None, pc:Default::default(),pc_dropped:0,resources:Vec::new(),marker_times:Default::default() } }
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

/// Index 0: guest presentRenderbuffer completion. Index 1: native SDL swap.
pub fn frame(host: bool) {
    if !ENABLED.load(Ordering::Relaxed) { return; }
    let i = usize::from(host);
    let now = Instant::now();
    STATE.with(|s| {
        let mut s=s.borrow_mut();
        if let Some(last)=s.last_frame[i] {
            // Bound retained samples even if the host poll is delayed.
            if s.intervals[i].len() < 4096 { s.intervals[i].push(now.duration_since(last).as_secs_f64()*1000.0); }
        }
        s.last_frame[i]=Some(now); s.frame_counts[i]+=1;
    });
}
pub fn allocated(bytes: u32) {
    if ENABLED.load(Ordering::Relaxed) { STATE.with(|s|s.borrow_mut().alloc_bytes+=u64::from(bytes)); }
}

#[repr(C)]
#[derive(Default)]
struct Native {
    cpu_seconds: f64, resident_bytes: u64, thermal: i32, low_power: i32,
    max_fps: i32, battery_state: i32, orientation: i32,
    drawable_width: i32, drawable_height: i32,
}
#[cfg(target_os="ios")]
extern "C" { fn peggle_native_diagnostics(out: *mut Native); }
fn native() -> Native {
    let mut n=Native::default();
    #[cfg(target_os="ios")]
    unsafe { peggle_native_diagnostics(&mut n); }
    n
}

fn stats(values: &mut [f64]) -> (f64,f64,f64,f64,f64) {
    if values.is_empty() { return (0.0,0.0,0.0,0.0,0.0); }
    let mean=values.iter().sum::<f64>()/values.len() as f64;
    let std=(values.iter().map(|v|(v-mean).powi(2)).sum::<f64>()/values.len() as f64).sqrt();
    values.sort_unstable_by(f64::total_cmp);
    let percentile=|p:f64|values[((values.len()-1) as f64*p).ceil() as usize];
    (mean,std,percentile(0.50),percentile(0.95),*values.last().unwrap())
}

/// Called on the host poll, at most once every five seconds. No background
/// timer is added. OS Instruments is required for GPU time and idle power.
pub fn tick() {
    if !ENABLED.load(Ordering::Relaxed) { return; }
    STATE.with(|s| {
        let mut s=s.borrow_mut();
        let now=Instant::now(); let elapsed=now.duration_since(s.last_report).as_secs_f64();
        if elapsed < 5.0 { return; }
        let n=native(); let cpu_delta=(n.cpu_seconds-s.previous_cpu).max(0.0);
        let mut phases=String::new();
        for (i,t) in s.timings.iter().enumerate() {
            if i>0 { phases.push(','); }
            phases.push_str(&format!("\"{}\":{{\"calls\":{},\"ms\":{:.4},\"max_ms\":{:.4}}}",NAMES[i],t.calls,t.ns as f64/1e6,t.max_ns as f64/1e6));
        }
        let mut frames=String::new();
        for i in 0..2 {
            let (mean,std,p50,p95,max)=stats(&mut s.intervals[i]);
            if i>0 { frames.push(','); }
            frames.push_str(&format!("\"{}\":{{\"frames\":{},\"fps\":{:.3},\"interval_mean_ms\":{mean:.3},\"interval_std_ms\":{std:.3},\"p50_ms\":{p50:.3},\"p95_ms\":{p95:.3},\"max_ms\":{max:.3}}}",if i==0 {"guest"} else {"host"},s.frame_counts[i],s.frame_counts[i] as f64/elapsed));
            s.intervals[i].clear();
        }
        event(&format!("{{\"event\":\"window\",\"unix_time_ms\":{},\"interpreter_timing_enabled\":{},\"uptime_s\":{:.3},\"duration_s\":{elapsed:.3},\"cpu_percent_one_core\":{:.2},\"cpu_first_window\":{},\"resident_bytes\":{},\"thermal\":{},\"low_power\":{},\"max_fps\":{},\"battery_state\":{},\"orientation\":{},\"native_drawable\":[{},{}],\"allocated_guest_bytes\":{},\"frames\":{{{frames}}},\"phases\":{{{phases}}},\"gpu_ms\":null,\"power_w\":null}}",std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0,|x|x.as_millis()),INTERPRETER_TIMING.load(Ordering::Relaxed),now.duration_since(s.start).as_secs_f64(),100.0*cpu_delta/elapsed,s.previous_cpu==0.0,n.resident_bytes,n.thermal,n.low_power,n.max_fps,n.battery_state,n.orientation,n.drawable_width,n.drawable_height,s.alloc_bytes));
        let mut pcs:Vec<_>=s.pc.iter().map(|(k,v)|(*k,*v)).collect();
        pcs.sort_by(|a,b|b.1.0.cmp(&a.1.0));pcs.truncate(40);
        let mut entries=Vec::with_capacity(pcs.len());
        for ((pc,thumb,stop),(count,lr,opcode)) in pcs {
            let hint=if HD_MARKERS.load(Ordering::Relaxed) {guest_markers::hint(pc).unwrap_or("unclassified")}else{"unverified_image"};
            entries.push(format!("{{\"pc\":{pc},\"lr\":{lr},\"opcode\":{opcode},\"thumb\":{thumb},\"stop_kind\":{stop},\"count\":{count},\"hint\":\"{hint}\"}}"));
        }
        event(&format!("{{\"event\":\"guest_pc_samples\",\"uptime_s\":{:.3},\"sampling\":\"jittered_instruction_budget_boundaries\",\"cpu_time_weighted\":false,\"guest_threads\":\"combined\",\"unique_pcs\":{},\"dropped\":{},\"top\":[{}]}}",now.duration_since(s.start).as_secs_f64(),s.pc.len(),s.pc_dropped,entries.join(",")));
        if !s.resources.is_empty() {event(&format!("{{\"event\":\"slow_resource_calls\",\"uptime_s\":{:.3},\"calls\":[{}]}}",now.duration_since(s.start).as_secs_f64(),s.resources.join(",")));}
        s.pc.clear();s.pc_dropped=0;s.resources.clear();
        s.last_report=now; s.previous_cpu=n.cpu_seconds; s.timings=[Timing::default();22]; s.alloc_bytes=0; s.frame_counts=[0;2];
    });
}

pub fn lifecycle_pause(pausing: bool) {
    if !ENABLED.load(Ordering::Relaxed) { return; }
    let now=Instant::now(); let n=native();
    STATE.with(|s| {
        let mut s=s.borrow_mut();
        if pausing {
            s.paused=Some((now,n.cpu_seconds));
            event("{\"event\":\"pause\"}");
        } else if let Some((start,cpu))=s.paused.take() {
            let seconds=now.duration_since(start).as_secs_f64();
            let percent=if seconds>0.0 { 100.0*(n.cpu_seconds-cpu).max(0.0)/seconds } else {0.0};
            event(&format!("{{\"event\":\"resume\",\"paused_s\":{seconds:.3},\"paused_process_cpu_percent_one_core\":{percent:.4},\"thermal\":{},\"power_w\":null}}",n.thermal));
        }
        // A native pause is not a slow gameplay frame. It also must not dilute
        // the next active window's CPU percentage with background wall time.
        s.last_frame=[None;2]; s.intervals.iter_mut().for_each(Vec::clear);
        s.pc.clear();s.pc_dropped=0;s.resources.clear();
        s.timings=[Timing::default();22]; s.frame_counts=[0;2]; s.alloc_bytes=0;
        s.last_report=now; s.previous_cpu=n.cpu_seconds;
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn interpreter_timer_can_be_skipped_without_disabling_other_scopes() {
        ENABLED.store(true, Ordering::Relaxed);
        INTERPRETER_TIMING.store(false, Ordering::Relaxed);
        drop(scope(Phase::Interpreter));
        drop(scope(Phase::GuestGl));
        STATE.with(|s| {
            let s=s.borrow();
            assert_eq!(s.timings[Phase::Interpreter as usize].calls, 0);
            assert_eq!(s.timings[Phase::GuestGl as usize].calls, 1);
        });
        INTERPRETER_TIMING.store(true, Ordering::Relaxed);
        drop(scope(Phase::Interpreter));
        STATE.with(|s| assert_eq!(s.borrow().timings[Phase::Interpreter as usize].calls, 1));
        ENABLED.store(false, Ordering::Relaxed);
    }
    #[test] fn frame_variance_and_tail() {
        let (m,s,p50,p95,max)=stats(&mut [10.0,20.0,30.0,100.0]);
        assert_eq!((m,p50,p95,max),(40.0,30.0,100.0,100.0));
        assert!((s-35.3553390593).abs()<1e-8);
    }
    #[test] fn empty_window_and_single_frame() {
        assert_eq!(stats(&mut []),(0.0,0.0,0.0,0.0,0.0));
        assert_eq!(stats(&mut [16.0]),(16.0,0.0,16.0,16.0,16.0));
    }
    #[test] fn pc_histogram_is_bounded_and_counts_existing_keys() {
        for pc in 0..5000 {pc_sample(pc,1,2,false,0);}
        pc_sample(0,3,4,false,0);
        STATE.with(|s|{let s=s.borrow();assert_eq!(s.pc.len(),4096);assert_eq!(s.pc_dropped,904);assert_eq!(s.pc[&(0,false,0)],(2,3,4));});
    }
    #[test] fn hd_range_labels_are_specific() {
        assert_eq!(guest_markers::hint(0x56952),Some("fever_setup"));
        assert_eq!(guest_markers::hint(0x28c080),Some("guest_zlib"));
        assert_eq!(guest_markers::hint(0xfffffff0),None);
    }
}

mod guest_markers { include!("guest_markers.rs"); }

pub fn pc_sampling_enabled()->bool {PC_SAMPLING.load(Ordering::Relaxed)}
pub fn configure_hd_markers(verified:bool) {
    HD_MARKERS.store(verified,Ordering::Relaxed);
    let unix_ms=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0,|x|x.as_millis());
    event(&format!("{{\"event\":\"guest_image_verification\",\"verified_hd\":{verified},\"unix_time_ms\":{unix_ms},\"pc_sampling\":{}}}",pc_sampling_enabled()));
}
pub fn pc_sample(pc:u32,lr:u32,opcode:u32,thumb:bool,stop:u8) {
    STATE.with(|s|{let mut s=s.borrow_mut();let key=(pc,thumb,stop);
        if let Some(v)=s.pc.get_mut(&key){v.0+=1;v.1=lr;v.2=opcode;}
        else if s.pc.len()<4096{s.pc.insert(key,(1,lr,opcode));}else{s.pc_dropped+=1;}
    });
}
pub fn marker_for_caller(lr:u32) {
    if !ENABLED.load(Ordering::Relaxed)||!HD_MARKERS.load(Ordering::Relaxed){return}
    if guest_markers::hint(lr&!1)!=Some("fever_setup"){return}
    STATE.with(|s|{let mut s=s.borrow_mut();let now=Instant::now();
        if s.marker_times.get("fever_setup").is_some_and(|last|now.duration_since(*last).as_secs_f64()<1.0){return}
        s.marker_times.insert("fever_setup",now);
        event(&format!("{{\"event\":\"guest_marker\",\"uptime_s\":{:.3},\"name\":\"fever_setup\",\"evidence\":\"wcstol_caller_in_verified_AddExtremeFever\",\"lr\":{lr}}}",now.duration_since(s.start).as_secs_f64()));
    });
}
pub struct ResourceScope { _timing:Scope, start:Option<Instant>,kind:&'static str,requested:u64,caller:u32,width:i32,height:i32 }
pub fn resource_scope(phase:Phase,kind:&'static str,requested:u64,caller:u32,width:i32,height:i32)->ResourceScope {
    ResourceScope {_timing:scope(phase),start:ENABLED.load(Ordering::Relaxed).then(Instant::now),kind,requested,caller,width,height}
}
impl Drop for ResourceScope {
    fn drop(&mut self){if let Some(start)=self.start {
        let ms=start.elapsed().as_secs_f64()*1000.0;
        if ms<20.0{return}
        STATE.with(|s|{let mut s=s.borrow_mut();if s.resources.len()<16 {
            let hint=if HD_MARKERS.load(Ordering::Relaxed){guest_markers::hint(self.caller&!1).unwrap_or("unclassified")}else{"unverified_image"};
            let v=format!("{{\"kind\":\"{}\",\"elapsed_ms\":{ms:.4},\"requested_bytes\":{},\"caller_lr\":{},\"dimensions\":[{},{}],\"caller_hint\":\"{hint}\"}}",self.kind,self.requested,self.caller,self.width,self.height);
            s.resources.push(v);
        }});
    }}
}

pub fn startup_loader_state(loading:bool) {
    if !ENABLED.load(Ordering::Relaxed){return}
    thread_local!{static LAST:std::cell::Cell<Option<bool>>=const{std::cell::Cell::new(None)};}
    LAST.with(|last|{if last.get()!=Some(loading){last.set(Some(loading));STATE.with(|s|{let s=s.borrow();event(&format!("{{\"event\":\"startup_loader_state\",\"uptime_s\":{:.3},\"loading\":{loading}}}",s.start.elapsed().as_secs_f64()));});}});
}
