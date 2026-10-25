/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `libkern/OSAtomic.h`
//!
//! Atomic operations.
//!
//! Right now touchHLE is a single host thread application.
//! Thus, the execution of host functions couldn't be interrupted
//! by other threads. So we consider host functions to be atomic!

use crate::dyld::FunctionExports;
use crate::export_c_func;
use crate::mem::{MutPtr, MutVoidPtr};
use crate::Environment;
use crate::environment::ThreadBlock;
use std::sync::atomic::{fence, Ordering};

fn OSAtomicAdd32(env: &mut Environment, amount: i32, value_ptr: MutPtr<i32>) -> i32 {
    OSAtomicAdd32Barrier(env, amount, value_ptr)
}

fn OSAtomicAdd32Barrier(env: &mut Environment, the_amount: i32, the_value: MutPtr<i32>) -> i32 {
    let curr = env.mem.read(the_value);
    let new = curr.wrapping_add(the_amount);
    fence(Ordering::SeqCst);
    env.mem.write(the_value, new);
    new
}

fn OSAtomicCompareAndSwap32(
    env: &mut Environment,
    old_value: i32,
    new_value: i32,
    the_value: MutPtr<i32>,
) -> bool {
    OSAtomicCompareAndSwap32Barrier(env, old_value, new_value, the_value)
}

fn OSAtomicCompareAndSwapIntBarrier(
    env: &mut Environment,
    old_value: i32,
    new_value: i32,
    the_value: MutPtr<i32>,
) -> bool {
    OSAtomicCompareAndSwap32Barrier(env, old_value, new_value, the_value)
}

fn OSAtomicCompareAndSwap32Barrier(
    env: &mut Environment,
    old_value: i32,
    new_value: i32,
    the_value: MutPtr<i32>,
) -> bool {
    if old_value == env.mem.read(the_value) {
        env.mem.write(the_value, new_value);
        true
    } else {
        false
    }
}

fn OSAtomicCompareAndSwapPtr(
    env: &mut Environment,
    old_value: MutVoidPtr,
    new_value: MutVoidPtr,
    the_value: MutPtr<MutVoidPtr>,
) -> bool {
    OSAtomicCompareAndSwapPtrBarrier(env, old_value, new_value, the_value)
}

fn OSAtomicCompareAndSwapPtrBarrier(
    env: &mut Environment,
    old_value: MutVoidPtr,
    new_value: MutVoidPtr,
    the_value: MutPtr<MutVoidPtr>,
) -> bool {
    if old_value == env.mem.read(the_value) {
        env.mem.write(the_value, new_value);
        true
    } else {
        false
    }
}

fn OSMemoryBarrier(_env: &mut Environment) {
    fence(Ordering::SeqCst);
}

fn OSAtomicOr32Barrier(env: &mut Environment, mask: u32, value: MutPtr<u32>) -> i32 {
    fence(Ordering::SeqCst);
    let new = env.mem.read(value) | mask;
    env.mem.write(value, new);
    fence(Ordering::SeqCst);
    new as i32
}

fn atomic_bit_position(bit: u32) -> (u32, u8) {
    // OSAtomic numbers bits most-significant-first within each byte.
    (bit >> 3, 0x80 >> (bit & 7))
}

fn OSAtomicTestAndClearBarrier(env: &mut Environment, bit: u32, address: MutVoidPtr) -> bool {
    let (offset, mask) = atomic_bit_position(bit);
    let byte = address.cast::<u8>() + offset;
    fence(Ordering::SeqCst);
    let old = env.mem.read(byte);
    env.mem.write(byte, old & !mask);
    fence(Ordering::SeqCst);
    old & mask != 0
}

fn OSSpinLockTry(env: &mut Environment, lock: MutPtr<i32>) -> bool {
    // Guest threads share a single host execution thread, so this host call
    // performs the read/modify/write atomically with respect to all guests.
    if env.mem.read(lock) != 0 { return false; }
    env.mem.write(lock, 1);
    fence(Ordering::Acquire);
    true
}

fn OSSpinLockLock(env: &mut Environment, lock: MutPtr<i32>) {
    while !OSSpinLockTry(env, lock) {
        // A host busy loop would prevent the guest owner from releasing it.
        env.yield_thread(ThreadBlock::NotBlocked);
    }
}

fn OSSpinLockUnlock(env: &mut Environment, lock: MutPtr<i32>) {
    fence(Ordering::Release);
    env.mem.write(lock, 0);
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(OSAtomicAdd32(_, _)),
    export_c_func!(OSAtomicAdd32Barrier(_, _)),
    export_c_func!(OSAtomicCompareAndSwap32(_, _, _)),
    export_c_func!(OSAtomicCompareAndSwapIntBarrier(_, _, _)),
    export_c_func!(OSAtomicCompareAndSwap32Barrier(_, _, _)),
    export_c_func!(OSAtomicCompareAndSwapPtr(_, _, _)),
    export_c_func!(OSAtomicCompareAndSwapPtrBarrier(_, _, _)),
    export_c_func!(OSMemoryBarrier()),
    export_c_func!(OSAtomicOr32Barrier(_, _)),
    export_c_func!(OSAtomicTestAndClearBarrier(_, _)),
    export_c_func!(OSSpinLockTry(_)),
    export_c_func!(OSSpinLockLock(_)),
    export_c_func!(OSSpinLockUnlock(_)),
];

#[cfg(test)]
mod tests {
    #[test]
    fn osatomic_bit_indices_follow_apple_byte_order() {
        for (bit, expected) in [(0, (0, 0x80)), (7, (0, 1)), (8, (1, 0x80)),
                                (31, (3, 1)), (56, (7, 0x80))] {
            assert_eq!(super::atomic_bit_position(bit), expected);
        }
    }
}
