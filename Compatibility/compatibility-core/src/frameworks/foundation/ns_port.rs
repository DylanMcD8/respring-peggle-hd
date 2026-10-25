/* This Source Code Form is subject to the Mozilla Public License, v. 2.0. */
//! Guest-local run-loop ports. These never create a native Mach IPC port.
use crate::objc::{id, msg, msg_class, nil, objc_classes, ClassExports, HostObject, NSZonePtr};

struct PortHostObject { valid: bool, delegate: id }
impl HostObject for PortHostObject {}

pub const CLASSES: ClassExports = objc_classes! {
(env, this, _cmd);
@implementation NSPort: NSObject
+ (id)port { let port: id = msg_class![env; NSMachPort new]; crate::objc::autorelease(env, port) }
- (bool)isValid { env.objc.borrow::<PortHostObject>(this).valid }
- (())invalidate { env.objc.borrow_mut::<PortHostObject>(this).valid = false; }
- (())setDelegate:(id)delegate { env.objc.borrow_mut::<PortHostObject>(this).delegate = delegate; }
- (id)delegate { env.objc.borrow::<PortHostObject>(this).delegate }
- (())scheduleInRunLoop:(id)run_loop forMode:(id)mode { () = msg![env; run_loop addPort:this forMode:mode]; }
- (())removeFromRunLoop:(id)run_loop forMode:(id)mode { () = msg![env; run_loop removePort:this forMode:mode]; }
- (())dealloc { env.objc.dealloc_object(this, &mut env.mem); }
@end
@implementation NSMachPort: NSPort
+ (id)allocWithZone:(NSZonePtr)_zone {
    env.objc.alloc_object(this, Box::new(PortHostObject { valid: true, delegate: nil }), &mut env.mem)
}
@end
};
