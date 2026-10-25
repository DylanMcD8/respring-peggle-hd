/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `CFData` and `CFMutableData`.
//!
//! These are toll-free bridged to `NSData` and `NSMutableData` in Apple's
//! implementation. Here they are the same types.

use super::cf_allocator::{kCFAllocatorDefault, CFAllocatorRef};
use super::{CFIndex, CFRange};
use crate::dyld::FunctionExports;
use crate::export_c_func;
use crate::frameworks::foundation::{NSRange, NSUInteger};
use crate::mem::{ConstPtr, ConstVoidPtr, MutPtr, MutVoidPtr};
use crate::objc::{id, msg, msg_class};
use crate::Environment;

pub type CFDataRef = super::CFTypeRef;
pub type CFMutableDataRef = super::CFTypeRef;

fn CFDataCreateMutable(env: &mut Environment, allocator: CFAllocatorRef, capacity: CFIndex) -> CFMutableDataRef {
    assert!(allocator == kCFAllocatorDefault || env.mem.read(allocator).is_system_default());
    let capacity: NSUInteger = capacity.try_into().unwrap();
    let data: id = msg_class![env; NSMutableData alloc];
    msg![env; data initWithCapacity:capacity]
}
fn CFDataGetMutableBytePtr(env: &mut Environment, data: CFMutableDataRef) -> MutPtr<u8> {
    let bytes: MutVoidPtr = msg![env; data mutableBytes];
    bytes.cast()
}
fn CFDataSetLength(env: &mut Environment, data: CFMutableDataRef, length: CFIndex) {
    let length: NSUInteger = length.try_into().unwrap();
    () = msg![env; data setLength:length];
}

pub fn CFDataCreate(
    env: &mut Environment,
    allocator: CFAllocatorRef,
    bytes: ConstPtr<u8>,
    length: CFIndex,
) -> CFDataRef {
    assert!(allocator == kCFAllocatorDefault || env.mem.read(allocator).is_system_default()); // unimplemented
    let bytes: ConstVoidPtr = bytes.cast();
    let length: NSUInteger = length.try_into().unwrap();
    let new: id = msg_class![env; NSData alloc];
    msg![env; new initWithBytes:bytes length:length]
}

fn CFDataCreateWithBytesNoCopy(
    env: &mut Environment,
    allocator: CFAllocatorRef,
    bytes: ConstPtr<u8>,
    length: CFIndex,
    deallocator: CFAllocatorRef,
) -> CFDataRef {
    assert!(allocator == kCFAllocatorDefault || env.mem.read(allocator).is_system_default()); // unimplemented
    assert!(env.mem.read(deallocator).is_null()); // unimplemented
    let bytes: MutVoidPtr = bytes.cast().cast_mut();
    let length: NSUInteger = length.try_into().unwrap();
    let new: id = msg_class![env; NSData alloc];
    msg![env; new initWithBytesNoCopy:bytes length:length freeWhenDone:false]
}

pub fn CFDataGetLength(env: &mut Environment, data: CFDataRef) -> CFIndex {
    let len: NSUInteger = msg![env; data length];
    len.try_into().unwrap()
}

pub fn CFDataGetBytePtr(env: &mut Environment, data: CFDataRef) -> ConstPtr<u8> {
    let ptr: ConstVoidPtr = msg![env; data bytes];
    ptr.cast()
}

fn CFDataGetBytes(env: &mut Environment, data: CFDataRef, range: CFRange, buffer: MutPtr<u8>) {
    let range = NSRange {
        location: range.location.try_into().unwrap(),
        length: range.length.try_into().unwrap(),
    };
    msg![env; data getBytes:buffer range:range]
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(CFDataCreateMutable(_, _)),
    export_c_func!(CFDataGetMutableBytePtr(_)),
    export_c_func!(CFDataSetLength(_, _)),
    export_c_func!(CFDataCreate(_, _, _)),
    export_c_func!(CFDataCreateWithBytesNoCopy(_, _, _, _)),
    export_c_func!(CFDataGetLength(_)),
    export_c_func!(CFDataGetBytePtr(_)),
    export_c_func!(CFDataGetBytes(_, _, _)),
];
