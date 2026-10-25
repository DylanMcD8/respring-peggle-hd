/* This Source Code Form is subject to the Mozilla Public License, v. 2.0. */
//! Guest NSUUID values. No native device identifiers are accessed.
use super::ns_string::{from_rust_string, to_rust_string};
use crate::mem::{ConstPtr, MutPtr};
use crate::objc::{autorelease, id, msg, nil, objc_classes, release, retain, ClassExports, HostObject, NSZonePtr};
use uuid::Uuid;

struct UUIDHostObject { value: Uuid }
impl HostObject for UUIDHostObject {}

pub const CLASSES: ClassExports = objc_classes! {
(env, this, _cmd);
@implementation NSUUID: NSObject
+ (id)allocWithZone:(NSZonePtr)_zone {
    env.objc.alloc_object(this, Box::new(UUIDHostObject { value: Uuid::nil() }), &mut env.mem)
}
+ (id)UUID {
    let object: id = msg![env; this new];
    autorelease(env, object)
}
- (id)init {
    env.objc.borrow_mut::<UUIDHostObject>(this).value = Uuid::new_v4();
    this
}
- (id)initWithUUIDString:(id)string {
    let value = Uuid::parse_str(&to_rust_string(env, string));
    match value {
        Ok(value) => { env.objc.borrow_mut::<UUIDHostObject>(this).value = value; this },
        Err(_) => { release(env, this); nil },
    }
}
- (id)initWithUUIDBytes:(ConstPtr<u8>)bytes {
    let value: [u8; 16] = env.mem.bytes_at(bytes, 16).try_into().unwrap();
    env.objc.borrow_mut::<UUIDHostObject>(this).value = Uuid::from_bytes(value);
    this
}
- (())getUUIDBytes:(MutPtr<u8>)bytes {
    let value = env.objc.borrow::<UUIDHostObject>(this).value;
    env.mem.bytes_at_mut(bytes, 16).copy_from_slice(value.as_bytes());
}
- (id)UUIDString {
    let value = env.objc.borrow::<UUIDHostObject>(this).value.hyphenated().to_string().to_uppercase();
    let string = from_rust_string(env, value);
    autorelease(env, string)
}
- (id)description { msg![env; this UUIDString] }
- (id)copyWithZone:(NSZonePtr)_zone { retain(env, this) }
- (bool)isEqual:(id)other {
    if other == nil { return false; }
    let class = env.objc.get_known_class("NSUUID", &mut env.mem);
    if !msg![env; other isKindOfClass:class] { return false; }
    env.objc.borrow::<UUIDHostObject>(this).value == env.objc.borrow::<UUIDHostObject>(other).value
}
- (u32)hash {
    env.objc.borrow::<UUIDHostObject>(this).value.as_bytes().chunks_exact(4)
        .fold(0u32, |hash, word| hash ^ u32::from_le_bytes(word.try_into().unwrap()))
}
@end
};
