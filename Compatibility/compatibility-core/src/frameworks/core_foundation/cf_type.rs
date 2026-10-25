/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `CFType` (type-generic functions etc).

use super::{CFHashCode, CFIndex};
use crate::dyld::{export_c_func, FunctionExports};
use crate::frameworks::foundation::NSUInteger;
use crate::objc::Class;
use crate::{msg, objc};
use crate::{msg_class, Environment};

pub type CFTypeRef = objc::id;
pub type CFTypeID = u32;

// Type IDs are opaque values. Use one stable ID per toll-free bridged public
// type, so immutable and mutable class-cluster instances compare consistently.
const BRIDGED_TYPES: &[(&str, CFTypeID)] = &[
    ("NSString", 1), ("NSNumber", 2), ("NSData", 3),
    ("NSArray", 4), ("NSDictionary", 5), ("NSURL", 6), ("NSDate", 7),
];

fn CFGetTypeID(env: &mut Environment, object: CFTypeRef) -> CFTypeID {
    assert!(!object.is_null());
    let class: Class = msg![env; object class];
    for &(name, type_id) in BRIDGED_TYPES {
        let public_class = env.objc.get_known_class(name, &mut env.mem);
        if env.objc.class_is_subclass_of(class, public_class) {
            return type_id;
        }
    }
    panic!("CFGetTypeID for an unsupported bridged object {:?}", object);
}

fn CFStringGetTypeID(_env: &mut Environment) -> CFTypeID { 1 }
fn CFNumberGetTypeID(_env: &mut Environment) -> CFTypeID { 2 }
fn CFDataGetTypeID(_env: &mut Environment) -> CFTypeID { 3 }

pub fn CFRetain(env: &mut Environment, object: CFTypeRef) -> CFTypeRef {
    assert!(!object.is_null()); // not allowed, unlike for normal objc objects
    objc::retain(env, object)
}
pub fn CFRelease(env: &mut Environment, object: CFTypeRef) {
    objc::release(env, object);
}

pub fn CFGetRetainCount(env: &mut Environment, object: CFTypeRef) -> CFIndex {
    let count: NSUInteger = msg![env; object retainCount];
    count as CFIndex
}

pub fn CFEqual(env: &mut Environment, object1: CFTypeRef, object2: CFTypeRef) -> bool {
    if object1 == object2 {
        return true;
    }
    // TODO: other classes
    let str_class: Class = msg_class![env; NSString class];
    let object1_class: Class = msg![env; object1 class];
    assert!(msg![env; object1_class isKindOfClass:str_class]);
    let object2_class: Class = msg![env; object2 class];
    assert!(msg![env; object2_class isKindOfClass:str_class]);
    // TODO: use isEqual: once it is fixed
    msg![env; object1 isEqualToString:object2]
}

pub fn CFHash(env: &mut Environment, object: CFTypeRef) -> CFHashCode {
    msg![env; object hash]
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(CFGetTypeID(_)),
    export_c_func!(CFStringGetTypeID()),
    export_c_func!(CFNumberGetTypeID()),
    export_c_func!(CFDataGetTypeID()),
    export_c_func!(CFRetain(_)),
    export_c_func!(CFRelease(_)),
    export_c_func!(CFGetRetainCount(_)),
    export_c_func!(CFEqual(_, _)),
    export_c_func!(CFHash(_)),
];
