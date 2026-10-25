/* This Source Code Form is subject to the Mozilla Public License, v. 2.0. */
//! Guest Security API boundary. No native keychain access or credentials.
//! A guest with no keychain service receives Apple's documented failure status.
use crate::dyld::{export_c_func, ConstantExports, FunctionExports, HostConstant, HostDylib};
use crate::mem::{ConstVoidPtr, MutPtr};
use crate::objc::{id, nil};
use crate::Environment;
use std::io::Read;
const NOT_AVAILABLE: i32 = -25291;
const ITEM_NOT_FOUND: i32 = -25300;
fn SecItemCopyMatching(env: &mut Environment, _query: id, result: MutPtr<id>) -> i32 {
    if !result.is_null() { env.mem.write(result, nil); }
    log_once!("Guest keychain contains no items; native host keychain is not accessed");
    ITEM_NOT_FOUND
}
fn SecItemAdd(env: &mut Environment, _query: id, result: MutPtr<id>) -> i32 {
    if !result.is_null() { env.mem.write(result, nil); }
    NOT_AVAILABLE
}
fn SecItemUpdate(_env: &mut Environment, _query: id, _attributes: id) -> i32 { NOT_AVAILABLE }
fn SecItemDelete(_env: &mut Environment, _query: id) -> i32 { ITEM_NOT_FOUND }
fn SecRandomCopyBytes(env: &mut Environment, random: ConstVoidPtr, count: u32, bytes: MutPtr<u8>) -> i32 {
    if !random.is_null() { return -50; }
    if count == 0 { return 0; }
    match std::fs::File::open("/dev/urandom").and_then(|mut f| f.read_exact(env.mem.bytes_at_mut(bytes, count))) {
        Ok(()) => 0,
        Err(_) => -1,
    }
}
const CONSTANTS: ConstantExports = &[
    ("_kSecRandomDefault", HostConstant::NullPtr),
    ("_kSecAttrAccessGroup", HostConstant::NSString("agrp")),
    ("_kSecAttrGeneric", HostConstant::NSString("gena")),
    ("_kSecClass", HostConstant::NSString("class")),
    ("_kSecClassGenericPassword", HostConstant::NSString("genp")),
    ("_kSecMatchLimit", HostConstant::NSString("m_Limit")),
    ("_kSecMatchLimitOne", HostConstant::NSString("m_LimitOne")),
    ("_kSecReturnData", HostConstant::NSString("r_Data")),
    ("_kSecValueData", HostConstant::NSString("v_Data")),
];
const FUNCTIONS: FunctionExports = &[
    export_c_func!(SecItemCopyMatching(_, _)),
    export_c_func!(SecItemAdd(_, _)),
    export_c_func!(SecItemUpdate(_, _)),
    export_c_func!(SecItemDelete(_)),
    export_c_func!(SecRandomCopyBytes(_, _, _)),
];
pub const DYLIB: HostDylib = HostDylib {
    path: "/System/Library/Frameworks/Security.framework/Security", aliases: &[],
    class_exports: &[], constant_exports: &[CONSTANTS], function_exports: &[FUNCTIONS],
};
