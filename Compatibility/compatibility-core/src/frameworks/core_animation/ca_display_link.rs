/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `CADisplayLink`

use crate::frameworks::foundation::ns_run_loop::NSRunLoopMode;
use crate::frameworks::foundation::ns_timer::set_time_interval;
use crate::frameworks::foundation::NSInteger;
use crate::objc::{
    autorelease, id, msg, msg_class, msg_send, nil, objc_classes, release, retain, ClassExports,
    HostObject, NSZonePtr, SEL,
};

#[derive(Default)]
struct CADisplayLinkHostObject {
    target: id,
    selector: Option<SEL>,
    /// Weak reference. The timer retains the display link (as its target),
    /// so the timer necessarily outlives the display link. After `invalidate`,
    /// this pointer must not be used.
    ns_timer: id,
    paused: bool,
    pacing_target:u32,
    simulation_steps:u64,
    simulation_samples:u64,
    pacing_report:Option<std::time::Instant>,
}
impl HostObject for CADisplayLinkHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation CADisplayLink: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    env.objc.alloc_object(this, Box::new(CADisplayLinkHostObject::default()), &mut env.mem)
}

+ (id)displayLinkWithTarget:(id)target selector:(SEL)sel {
    let display_link: id = msg![env; this new];
    // Because timer will pass itself as a second arg in ns_timer:handle_timer,
    // we need to use a re-direction: first fire the timer on the display link,
    // then call the original selector, passing the link as a second argument!
    let redirect_sel: SEL = env.objc.lookup_selector("_touchHLE_displayLinkTimerDidFire:").unwrap();
    let render_selector=sel.as_str(&env.mem)=="renderGLScene";
    let loading=render_selector&&env.peggle_hd_is_loading();
    let initial_hz=if render_selector{crate::frame_pacing::target(loading)}else{None};
    let initial_interval=initial_hz.map_or(1.0/60.0,|hz|1.0/hz as f64);
    let ns_timer = msg_class![env; NSTimer timerWithTimeInterval:initial_interval
                     target:display_link
                   selector:redirect_sel
                   userInfo:nil
                    repeats:true];
    crate::frameworks::foundation::ns_timer::mark_display_link_render(env,ns_timer,initial_hz.is_some());
    crate::diagnostics::event(&format!("{{\"event\":\"display_link_configuration\",\"original_render_selector\":{render_selector},\"adaptive\":{},\"initial_hz\":{:.3}}}",initial_hz.is_some(),1.0/initial_interval));
    retain(env, target);
    let host_object = env.objc.borrow_mut::<CADisplayLinkHostObject>(display_link);
    host_object.target = target;
    host_object.selector = Some(sel);
    host_object.ns_timer = ns_timer;
    host_object.pacing_target=initial_hz.unwrap_or(0);
    log_dbg!("[CADisplayLink displayLinkWithTarget:{:?} selector:{}] => {:?}", target, sel.as_str(&env.mem), display_link);
    autorelease(env, display_link)
}

- (bool)isPaused {
    env.objc.borrow::<CADisplayLinkHostObject>(this).paused
}
- (())setPaused:(bool)paused {
    env.objc.borrow_mut::<CADisplayLinkHostObject>(this).paused = paused;
}

- (())setFrameInterval:(NSInteger)frameInterval {
    log_dbg!("[(CADisplayLink*){:?} setFrameInterval:{}]", this, frameInterval);
    assert!(frameInterval >= 1);
    let requested_interval = frameInterval as f64 / 60.0;
    let render_selector=env.objc.borrow::<CADisplayLinkHostObject>(this).selector.unwrap().as_str(&env.mem)=="renderGLScene";
    let loading=render_selector&&env.peggle_hd_is_loading();
    let adaptive_hz=if render_selector{crate::frame_pacing::target(loading)}else{None};
    let interval=adaptive_hz.map(|hz|1.0/hz as f64).or_else(||env.options.display_link_fps.map(|fps|1.0/fps)).unwrap_or(requested_interval);
    if let Some(fps) = env.options.display_link_fps {
        log!("CADisplayLink scheduling: guest frameInterval={}, requested={}fps, target={}fps, effective interval={}s", frameInterval, 1.0/requested_interval, fps, interval);
    }
    let ns_timer = env.objc.borrow::<CADisplayLinkHostObject>(this).ns_timer;
    if let Some(hz)=adaptive_hz{crate::frameworks::foundation::ns_timer::reset_display_interval(env,ns_timer,interval);env.objc.borrow_mut::<CADisplayLinkHostObject>(this).pacing_target=hz;}else{set_time_interval(env, ns_timer, interval);}
    crate::diagnostics::event(&format!("{{\"event\":\"display_link_interval_request\",\"guest_frame_interval\":{frameInterval},\"original_render_selector\":{render_selector},\"adaptive\":{},\"effective_hz\":{:.3}}}",adaptive_hz.is_some(),1.0/interval));
}

- (())addToRunLoop:(id)run_loop forMode:(NSRunLoopMode)mode {
    log_dbg!("[(CADisplayLink*){:?} addToRunLoop:{:?} forMode:{:?}]", this, run_loop, mode);
    let ns_timer = env.objc.borrow::<CADisplayLinkHostObject>(this).ns_timer;
    () = msg![env; run_loop addTimer:ns_timer forMode:mode];
}

- (())invalidate {
    log_dbg!("[(CADisplayLink*){:?} invalidate]", this);
    let ns_timer = env.objc.borrow::<CADisplayLinkHostObject>(this).ns_timer;
    () = msg![env; ns_timer invalidate];
}

- (())dealloc {
    let &CADisplayLinkHostObject { target, .. } = env.objc.borrow(this);
    release(env, target);
    env.objc.dealloc_object(this, &mut env.mem);
}

- (())_touchHLE_displayLinkTimerDidFire:(id)timer { // NSTimer *
    let &CADisplayLinkHostObject {
        target,
        selector,
        ns_timer,
        paused,
        ..
    } = env.objc.borrow::<CADisplayLinkHostObject>(this);
    assert_eq!(ns_timer, timer);
    if paused {
        // This could be improved, as we're still running the timer,
        // but just not passing the actual call.
        return;
    }
    // Restrict adaptive scheduling to the verified game's original render callback.
    let adaptive=selector.unwrap().as_str(&env.mem)=="renderGLScene";
    let loading=adaptive&&env.peggle_hd_is_loading();
    let hz=if adaptive{crate::frame_pacing::target(loading)}else{None};
    if let Some(hz)=hz {
        let changed=env.objc.borrow::<CADisplayLinkHostObject>(this).pacing_target!=hz;
        if changed{crate::frameworks::foundation::ns_timer::reset_display_interval(env,ns_timer,1.0/hz as f64);env.objc.borrow_mut::<CADisplayLinkHostObject>(this).pacing_target=hz;}
    }
    let simulation_before=if hz.is_some(){crate::frame_pacing::simulation_counter(env)}else{None};
    let clock_before=env.startup_time;
    if hz.is_some(){crate::frame_pacing::begin_callback();}
    let ui_token=hz.is_some_and(|hz|crate::peggle_ui_cadence::begin(env,0,hz));
    let began=std::time::Instant::now();
    () = msg_send(env, (target, selector.unwrap(), this));
    if let Some(hz)=hz {
        let native_pause=env.startup_time.saturating_duration_since(clock_before);
        let seconds=began.elapsed().saturating_sub(native_pause).as_secs_f64();
        crate::frame_pacing::sample(seconds,loading);
        crate::peggle_ui_cadence::end(env,ui_token,seconds);
        let simulation_delta=simulation_before.zip(crate::frame_pacing::simulation_counter(env)).and_then(|((a,x),(b,y))|{let delta=y.wrapping_sub(x);(a==b&&delta<=1000).then_some(delta)});
        let now=std::time::Instant::now();let state=env.objc.borrow_mut::<CADisplayLinkHostObject>(this);
        if let Some(delta)=simulation_delta{state.simulation_steps+=u64::from(delta);state.simulation_samples+=1;}
        if state.pacing_report.is_none_or(|last|now.duration_since(last).as_secs_f64()>=5.0){
            let interval=state.pacing_report.map_or(0.0,|last|now.duration_since(last).as_secs_f64());
            state.pacing_report=Some(now);
            crate::diagnostics::event(&format!("{{\"event\":\"render_pacing\",\"target_hz\":{hz},\"host_display_callback_hz\":{:.2},\"loading\":{loading},\"simulation_step_s\":0.01,\"observed_simulation_steps\":{},\"observed_counter_samples\":{},\"interval_s\":{interval:.3},\"metric\":\"timer_callback_ceiling_not_scanout\"}}",crate::frame_pacing::observed(),state.simulation_steps,state.simulation_samples));
            state.simulation_steps=0;state.simulation_samples=0;
        }
    }
}

@end

};
