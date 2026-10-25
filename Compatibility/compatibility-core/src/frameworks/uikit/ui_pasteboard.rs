/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UIPasteboard`.

use crate::frameworks::foundation::ns_string::{get_static_str, to_rust_string};
use crate::objc::{id, msg, nil, objc_classes, release, retain, ClassExports, HostObject, NSZonePtr};
use std::collections::HashMap;

#[derive(Default)]
pub struct State { named: HashMap<String, id> }
#[derive(Default)]
struct PasteboardHostObject { values: HashMap<String, id>, persistent: bool }
impl HostObject for PasteboardHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation UIPasteboard: NSObject
+ (id)allocWithZone:(NSZonePtr)_zone {
    env.objc.alloc_object(this, Box::<PasteboardHostObject>::default(), &mut env.mem)
}
+ (id)pasteboardWithName:(id)name create:(bool)create {
    let name = to_rust_string(env, name).into_owned();
    if let Some(&board) = env.framework_state.uikit.ui_pasteboard.named.get(&name) { return board; }
    if !create { return nil; }
    let board: id = msg![env; this new];
    env.framework_state.uikit.ui_pasteboard.named.insert(name, board);
    board
}
+ (())removePasteboardWithName:(id)name {
    let name = to_rust_string(env, name).into_owned();
    if let Some(board) = env.framework_state.uikit.ui_pasteboard.named.remove(&name) { release(env, board); }
}
- (id)dataForPasteboardType:(id)type_name {
    msg![env; this valueForPasteboardType:type_name]
}
- (id)valueForPasteboardType:(id)type_name {
    let name = to_rust_string(env, type_name).into_owned();
    env.objc.borrow::<PasteboardHostObject>(this).values.get(&name).copied().unwrap_or(nil)
}
- (())setData:(id)data forPasteboardType:(id)type_name {
    () = msg![env; this setValue:data forPasteboardType:type_name];
}
- (())setValue:(id)value forPasteboardType:(id)type_name {
    let name = to_rust_string(env, type_name).into_owned();
    let value = retain(env, value);
    let old = if value == nil {
        env.objc.borrow_mut::<PasteboardHostObject>(this).values.remove(&name)
    } else {
        env.objc.borrow_mut::<PasteboardHostObject>(this).values.insert(name, value)
    };
    if let Some(old) = old { release(env, old); }
}
- (())setPersistent:(bool)persistent {
    // Guest process-local state only; never a native shared pasteboard.
    env.objc.borrow_mut::<PasteboardHostObject>(this).persistent = persistent;
}
- (bool)isPersistent { env.objc.borrow::<PasteboardHostObject>(this).persistent }
- (id)string {
    let type_name = get_static_str(env, "public.utf8-plain-text");
    msg![env; this valueForPasteboardType:type_name]
}
- (())setString:(id)string {
    let type_name = get_static_str(env, "public.utf8-plain-text");
    () = msg![env; this setValue:string forPasteboardType:type_name];
}
- (())dealloc {
    let values = std::mem::take(&mut env.objc.borrow_mut::<PasteboardHostObject>(this).values);
    for value in values.into_values() { release(env, value); }
    env.objc.dealloc_object(this, &mut env.mem);
}
@end

};
