/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `sched.h`.

use crate::dyld::{export_c_func, FunctionExports};
use crate::Environment;

fn sched_yield(env: &mut Environment) -> i32 {
    log_dbg!(
        "TODO: thread {} requested processor yield, ignoring",
        env.current_thread
    );
    0 // success
}

fn sched_get_priority_max(_env: &mut Environment, _policy: i32) -> i32 { 47 }
fn sched_get_priority_min(_env: &mut Environment, _policy: i32) -> i32 { 15 }
pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(sched_yield()),
    export_c_func!(sched_get_priority_max(_)),
    export_c_func!(sched_get_priority_min(_)),
];
