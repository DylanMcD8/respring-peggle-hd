/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `cxxabi.h`
//!
//! Resources:
//! - [Itanium C++ ABI specification](https://itanium-cxx-abi.github.io/cxx-abi/abi.html#dso-dtor-runtime-api)

use crate::abi::GuestFunction;
use crate::dyld::{export_c_func, ConstantExports, FunctionExports, HostConstant};
use crate::mem::MutVoidPtr;
use crate::Environment;
use std::collections::HashMap;

#[derive(Default)]
pub struct State {
    sjlj_contexts: HashMap<crate::ThreadId, Vec<MutVoidPtr>>,
}

pub const CONSTANTS: ConstantExports = &[("___stack_chk_guard", HostConstant::Custom(|env| {
    let bytes = uuid::Uuid::new_v4().into_bytes();
    let guard = u32::from_le_bytes(bytes[..4].try_into().unwrap()) | 1;
    env.mem.alloc_and_write(guard).cast().cast_const()
}))];

fn __stack_chk_fail(_env: &mut Environment) {
    panic!("Guest stack protector detected corruption");
}

// Clang registers stack-local contexts even on paths that never throw. Keep
// these per guest thread. Actual exception propagation is intentionally not
// claimed: Resume fails explicitly instead of continuing with corrupt state.
fn _Unwind_SjLj_Register(env: &mut Environment, context: MutVoidPtr) {
    assert!(!context.is_null());
    env.libc_state.cxxabi.sjlj_contexts.entry(env.current_thread).or_default().push(context);
}

fn _Unwind_SjLj_Unregister(env: &mut Environment, context: MutVoidPtr) {
    let contexts = env.libc_state.cxxabi.sjlj_contexts.get_mut(&env.current_thread).unwrap();
    assert_eq!(contexts.pop(), Some(context), "SjLj registration order mismatch");
}

fn _Unwind_SjLj_Resume(_env: &mut Environment, _exception: MutVoidPtr) {
    panic!("Guest requested SjLj exception propagation, which is not implemented");
}

fn __cxa_atexit(
    _env: &mut Environment,
    func: GuestFunction, // void (*func)(void *)
    p: MutVoidPtr,
    d: MutVoidPtr,
) -> i32 {
    // TODO: when this is implemented, make sure it's properly compatible with
    // C atexit.
    log!(
        "TODO: __cxa_atexit({:?}, {:?}, {:?}) (unimplemented)",
        func,
        p,
        d
    );
    0 // success
}

fn __cxa_finalize(_env: &mut Environment, d: MutVoidPtr) {
    log!("TODO: __cxa_finalize({:?}) (unimplemented)", d);
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(__stack_chk_fail()),
    export_c_func!(_Unwind_SjLj_Register(_)),
    export_c_func!(_Unwind_SjLj_Unregister(_)),
    export_c_func!(_Unwind_SjLj_Resume(_)),
    export_c_func!(__cxa_atexit(_, _, _)),
    export_c_func!(__cxa_finalize(_)),
];
