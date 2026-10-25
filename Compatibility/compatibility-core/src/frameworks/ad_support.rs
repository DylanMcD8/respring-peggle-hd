/* This Source Code Form is subject to the Mozilla Public License, v. 2.0. */
//! AdSupport boundary for an isolated guest with tracking unavailable.
use crate::dyld::HostDylib;
use crate::frameworks::foundation::ns_string::get_static_str;
use crate::objc::{autorelease, id, msg, msg_class, objc_classes, ClassExports};

#[derive(Default)]
pub struct State { manager: Option<id> }

const CLASSES: ClassExports = objc_classes! {
(env, this, _cmd);
@implementation ASIdentifierManager: NSObject
+ (id)sharedManager {
    if let Some(manager) = env.framework_state.ad_support.manager { return manager; }
    let manager: id = msg![env; this new];
    env.framework_state.ad_support.manager = Some(manager);
    manager
}
- (bool)isAdvertisingTrackingEnabled { false }
- (id)advertisingIdentifier {
    let zero = get_static_str(env, "00000000-0000-0000-0000-000000000000");
    let uuid: id = msg_class![env; NSUUID alloc];
    let uuid: id = msg![env; uuid initWithUUIDString:zero];
    autorelease(env, uuid)
}
@end
};

pub const DYLIB: HostDylib = HostDylib {
    path: "/System/Library/Frameworks/AdSupport.framework/AdSupport", aliases: &[],
    class_exports: &[CLASSES], constant_exports: &[], function_exports: &[],
};
