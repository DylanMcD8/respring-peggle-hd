/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSTimer`.

use super::ns_run_loop::NSDefaultRunLoopMode;
use super::NSTimeInterval;
use super::{ns_run_loop, ns_string};
use crate::objc::{
    autorelease, id, msg, msg_class, msg_send, nil, objc_classes, release, retain, ClassExports,
    HostObject, SEL,
};
use crate::Environment;
use std::time::{Duration, Instant};

struct NSTimerHostObject {
    ns_interval: NSTimeInterval,
    /// Copy of `ns_interval` in Rust's type for time intervals. Keep in sync!
    rust_interval: Duration,
    /// Strong reference
    target: id,
    selector: SEL,
    /// Strong reference
    user_info: id,
    repeats: bool,
    due_by: Option<Instant>,
    /// If the timer is currently running its callback, this is set so that the
    /// re-entering the run loop from inside the callback doesn't cause an
    /// infinite loop.
    is_running_callback: bool,
    render_schedule: crate::presentation_schedule::Schedule,
    display_link_render:bool,
    pacing_route_reported:bool,
    /// Weak reference
    run_loop: id,
}
impl HostObject for NSTimerHostObject {}

pub(super) fn shift_after_pause(env: &mut Environment, timer: id, duration: Duration) {
    let timer = env.objc.borrow_mut::<NSTimerHostObject>(timer);
    timer.render_schedule.reset();
    timer.due_by = timer.due_by.map(|due| crate::lifecycle_pause::shifted(due, duration));
}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

// NSTimer doesn't seem to be an abstract class?
@implementation NSTimer: NSObject

+ (id)timerWithTimeInterval:(NSTimeInterval)ns_interval
                     target:(id)target
                   selector:(SEL)selector
                   userInfo:(id)user_info
                    repeats:(bool)repeats {
    let ns_interval = ns_interval.max(0.0001);
    let rust_interval = Duration::from_secs_f64(ns_interval);

    retain(env, target);
    retain(env, user_info);

    let host_object = Box::new(NSTimerHostObject {
        ns_interval,
        rust_interval,
        target,
        selector,
        user_info,
        repeats,
        due_by: Some(Instant::now().checked_add(rust_interval).unwrap()),
        run_loop: nil,
        render_schedule: crate::presentation_schedule::Schedule::default(),
        display_link_render:false,
        pacing_route_reported:false,
        is_running_callback: false
    });
    let new = env.objc.alloc_object(this, host_object, &mut env.mem);

    log_dbg!(
        "New {} timer {:?}, interval {}s, target [{:?} {}], user info {:?}",
        if repeats { "repeating" } else { "single-use" },
        new,
        ns_interval,
        target,
        selector.as_str(&env.mem),
        user_info,
    );

    autorelease(env, new)
}

+ (id)scheduledTimerWithTimeInterval:(NSTimeInterval)interval
                              target:(id)target
                            selector:(SEL)selector
                            userInfo:(id)user_info
                             repeats:(bool)repeats {
    let timer = msg![env; this timerWithTimeInterval:interval
                                              target:target
                                            selector:selector
                                            userInfo:user_info
                                             repeats:repeats];

    let run_loop: id = msg_class![env; NSRunLoop currentRunLoop];
    let mode: id = ns_string::get_static_str(env, NSDefaultRunLoopMode);
    let _: () = msg![env; run_loop addTimer:timer forMode:mode];

    timer
}

- (())dealloc {
    let &NSTimerHostObject {
        target,
        user_info,
        ..
    } = env.objc.borrow(this);
    release(env, target);
    release(env, user_info);
    env.objc.dealloc_object(this, &mut env.mem)
}

- (NSTimeInterval)timeInterval {
    let host_object = env.objc.borrow::<NSTimerHostObject>(this);
    if host_object.repeats {
        host_object.ns_interval
    } else {
        0.0 // this is the documented behaviour!
    }
}
- (id)userInfo {
    env.objc.borrow::<NSTimerHostObject>(this).user_info
}
- (bool)isValid {
    env.objc.borrow::<NSTimerHostObject>(this).due_by.is_some()
}

- (())invalidate {
    let timer = env.objc.borrow_mut::<NSTimerHostObject>(this);
    // Timer might already be invalid, don't try to remove it twice.
    if timer.due_by.take().is_some() {
        let run_loop = timer.run_loop;
        ns_run_loop::remove_timer(env, run_loop, this);
    }
}

- (())fire {
    let &NSTimerHostObject {
        target,
        selector,
        repeats,
        ..
    } = env.objc.borrow(this);

    let pool: id = msg_class![env; NSAutoreleasePool new];

    // Signature should be `- (void)timerDidFire:(NSTimer *)which`.
    let _: () = msg_send(env, (target, selector, this));

    release(env, pool);

    if !repeats {
        () = msg![env; this invalidate];
    }
}

// TODO: more constructors
// TODO: more accessors

@end

};

/// For use by `CADisplayLink`
pub fn set_time_interval(env: &mut Environment, timer: id, interval: NSTimeInterval) {
    let host_object = env.objc.borrow_mut::<NSTimerHostObject>(timer);
    host_object.ns_interval = interval;
    host_object.rust_interval = Duration::from_secs_f64(interval);
}

/// Display-link-only cadence changes discard an old deadline, avoiding bursts.
pub(crate) fn reset_display_interval(env:&mut Environment,timer:id,interval:NSTimeInterval){
 let t=env.objc.borrow_mut::<NSTimerHostObject>(timer);t.ns_interval=interval;t.rust_interval=Duration::from_secs_f64(interval);t.render_schedule.reset();t.due_by=Some(Instant::now()+t.rust_interval);
}

pub(crate)fn mark_display_link_render(env:&mut Environment,timer:id,enabled:bool){env.objc.borrow_mut::<NSTimerHostObject>(timer).display_link_render=enabled;}

/// For use by `NSRunLoop`
pub(super) fn set_run_loop(env: &mut Environment, timer: id, run_loop: id) {
    let host_object = env.objc.borrow_mut::<NSTimerHostObject>(timer);
    assert!(host_object.run_loop == nil); // TODO: what do we do here?
    host_object.run_loop = run_loop;
}

/// For use by `NSRunLoop`: check if a timer is due to fire and fire it if
/// necessary.
///
/// Returns the next firing time, if any.
pub(super) fn handle_timer(env: &mut Environment, timer: id) -> Option<Instant> {
    let &NSTimerHostObject {
        ns_interval,
        rust_interval,
        target,
        selector,
        repeats,
        due_by,
        is_running_callback,
        run_loop,
        display_link_render,
        ..
    } = env.objc.borrow(timer);

    // If a timer is already running its callback, we don't want to re-enter it
    // and cause an infinite loop.
    if is_running_callback {
        return None;
    }

    // Invalidated timers should be removed from the run loop, but if a timer
    // is invalidated from another timer earlier in the current tick of the
    // run loop it might still be run.
    let due_by = due_by?;

    let now = Instant::now();

    if due_by > now {
        return Some(due_by);
    }

    let overdue_by = now.duration_since(due_by);

    // Timer may be released when it's invalidated, so we need to retain it so
    // it's still around to pass to the timer target.
    retain(env, timer);

    // The original AppController also has a direct NSTimer render fallback.
    // Adapt only that exact repeating render selector in this verified image;
    // display-link redirect and every non-render timer retain existing behavior.
    let direct_render = crate::render_timer_route::eligible(env.libc_state.peggle_ui_cadence.enabled,repeats,selector.as_str(&env.mem));
    let loading=direct_render&&env.peggle_hd_is_loading();
    let render_hz=if direct_render{crate::frame_pacing::target(loading)}else{None};
    let (ns_interval,rust_interval)=render_hz.map_or((ns_interval,rust_interval),|hz|{let v=1.0/hz as f64;(v,Duration::from_secs_f64(v))});
    if render_hz.is_some(){set_time_interval(env,timer,ns_interval);}

    // Advancing the timer before sending its message seems like a good idea
    // considering this function is potentially re-entrant.
    let strict_render_schedule=crate::render_timer_route::strict_schedule(render_hz.is_some(),display_link_render,crate::frame_pacing::enabled());
    if strict_render_schedule&&!env.objc.borrow::<NSTimerHostObject>(timer).pacing_route_reported{env.objc.borrow_mut::<NSTimerHostObject>(timer).pacing_route_reported=true;crate::diagnostics::event(&format!("{{\"event\":\"pacing_timer_route\",\"direct_render\":{},\"display_link_redirect\":{},\"strict_future_deadlines\":true}}",direct_render,display_link_render));}
    let new_due_by = if strict_render_schedule{
        Some(env.objc.borrow_mut::<NSTimerHostObject>(timer).render_schedule.poll(now,rust_interval,false).next)
    } else if repeats {
        // When rescheduling a repeating timer, the next firing should be based
        // on when the timer should have fired, not when it actually fired, so
        // that there is no drift over time.
        //
        // For example, if a timer has an interval of 60s and starts at 00:00,
        // the first firing would be scheduled for 01:00, and the second firing
        // should be scheduled for 02:00, even if the first firing was at 01:01.
        //
        // However: if the timer handling is delayed past a whole interval, it
        // should not try to catch up. For example, if the first firing is
        // scheduled for 01:00 but happens at 02:30, then the next firing should
        // be scheduled for 03:00.
        // TODO: Use `.div_duration_f64()` once that is stabilized.
        let advance_by = (overdue_by.as_secs_f64() / ns_interval).max(1.0).ceil();
        assert!(advance_by == (advance_by as u32) as f64);
        let advance_by = advance_by as u32;
        if advance_by > 1 {
            log_dbg!("Warning: Timer {:?} is lagging. It is overdue by {}s and has missed {} interval(s)!", timer, overdue_by.as_secs_f64(), advance_by - 1);
        }
        let advance_by = rust_interval.checked_mul(advance_by).unwrap();
        Some(due_by.checked_add(advance_by).unwrap())
    } else {
        ns_run_loop::remove_timer(env, run_loop, timer);
        None
    };
    env.objc.borrow_mut::<NSTimerHostObject>(timer).due_by = new_due_by;
    env.objc
        .borrow_mut::<NSTimerHostObject>(timer)
        .is_running_callback = true;

    log_dbg!(
        "Timer {:?} fired, sending {:?} message to {:?}",
        timer,
        selector.as_str(&env.mem),
        target
    );

    let pool: id = msg_class![env; NSAutoreleasePool new];

    // Signature should be `- (void)timerDidFire:(NSTimer *)which`.
    let ui_token=render_hz.is_some_and(|hz|crate::peggle_ui_cadence::begin(env,1,hz));
    let clock_before=env.startup_time;let began=Instant::now();
    if render_hz.is_some(){crate::frame_pacing::begin_callback();}
    let _: () = msg_send(env, (target, selector, timer));
    if render_hz.is_some(){let pause=env.startup_time.saturating_duration_since(clock_before);let seconds=began.elapsed().saturating_sub(pause).as_secs_f64();crate::frame_pacing::sample(seconds,loading);crate::peggle_ui_cadence::end(env,ui_token,seconds);}

    env.objc
        .borrow_mut::<NSTimerHostObject>(timer)
        .is_running_callback = false;

    // A display-link callback can change its own rate (or invalidate itself).
    // Return its actual new deadline, rather than the pre-callback deadline.
    let next_due=env.objc.borrow::<NSTimerHostObject>(timer).due_by;
    release(env, timer);
    release(env, pool);

    next_due
}
