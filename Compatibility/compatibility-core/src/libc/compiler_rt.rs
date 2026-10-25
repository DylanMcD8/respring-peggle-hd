/* This Source Code Form is subject to the Mozilla Public License, v. 2.0. */
//! Compiler arithmetic helpers exported by the guest's libSystem ABI.
use crate::dyld::{export_c_func, FunctionExports};
use crate::Environment;

fn __divsi3(_env: &mut Environment, a: i32, b: i32) -> i32 { a.wrapping_div(b) }
fn __udivsi3(_env: &mut Environment, a: u32, b: u32) -> u32 { a / b }
fn __modsi3(_env: &mut Environment, a: i32, b: i32) -> i32 { a.wrapping_rem(b) }
fn __umodsi3(_env: &mut Environment, a: u32, b: u32) -> u32 { a % b }
fn __divdi3(_env: &mut Environment, a: i64, b: i64) -> i64 { a.wrapping_div(b) }
fn __udivdi3(_env: &mut Environment, a: u64, b: u64) -> u64 { a / b }
fn __umoddi3(_env: &mut Environment, a: u64, b: u64) -> u64 { a % b }
fn __fixdfdi(_env: &mut Environment, a: f64) -> i64 { a as i64 }
fn __fixsfdi(_env: &mut Environment, a: f32) -> i64 { a as i64 }
fn __fixunssfdi(_env: &mut Environment, a: f32) -> u64 { a as u64 }
fn __floatdidf(_env: &mut Environment, a: i64) -> f64 { a as f64 }
fn __floatdisf(_env: &mut Environment, a: i64) -> f32 { a as f32 }
fn __floatundidf(_env: &mut Environment, a: u64) -> f64 { a as f64 }

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(__divsi3(_, _)), export_c_func!(__udivsi3(_, _)),
    export_c_func!(__modsi3(_, _)), export_c_func!(__umodsi3(_, _)),
    export_c_func!(__divdi3(_, _)), export_c_func!(__udivdi3(_, _)),
    export_c_func!(__umoddi3(_, _)), export_c_func!(__fixdfdi(_)),
    export_c_func!(__fixsfdi(_)), export_c_func!(__fixunssfdi(_)),
    export_c_func!(__floatdidf(_)), export_c_func!(__floatdisf(_)),
    export_c_func!(__floatundidf(_)),
];
