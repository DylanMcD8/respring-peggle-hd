/* This Source Code Form is subject to the Mozilla Public License, v. 2.0. */
//! 32-bit Apple block ABI: guest stack/global literals, heap copies and captures.
//! References: https://clang.llvm.org/docs/Block-ABI-Apple.html
//! All storage and callbacks remain in the guest; no native block is executed.
use crate::abi::{CallFromHost, GuestFunction};
use crate::dyld::{export_c_func, ConstantExports, FunctionExports, HostConstant};
use crate::mem::{ConstVoidPtr, MutPtr};
use crate::objc::{id, msg_class, nil, objc_classes, release, retain, ClassExports, NSZonePtr};
use crate::Environment;
use std::collections::HashMap;

const GLOBAL: u32 = 1 << 28;
const COPY_DISPOSE: u32 = 1 << 25;
const NEEDS_FREE: u32 = 1 << 24;
#[derive(Default)]
pub(super) struct State {
    blocks: HashMap<id, u32>,
    byrefs: HashMap<id, u32>,
}

pub const CONSTANTS: ConstantExports = &[
    ("__NSConcreteStackBlock", HostConstant::Custom(block_class)),
    ("__NSConcreteGlobalBlock", HostConstant::Custom(block_class)),
    ("__NSConcreteMallocBlock", HostConstant::Custom(block_class)),
];
fn block_class(env: &mut Environment) -> ConstVoidPtr {
    env.objc.get_known_class("_touchHLE_NSBlock", &mut env.mem).cast().cast_const()
}
fn word(env: &Environment, p: id, offset: u32) -> u32 { env.mem.read(p.cast::<u32>() + offset) }
fn set_word(env: &mut Environment, p: id, offset: u32, value: u32) { env.mem.write(p.cast::<u32>() + offset, value); }

pub const CLASSES: ClassExports = objc_classes! {
(env, this, _cmd);
@implementation _touchHLE_NSBlock: NSObject
- (id)copyWithZone:(NSZonePtr)_zone { _Block_copy(env, this.cast().cast_const()) }
- (id)retain {
    if let Some(count) = env.objc.blocks.blocks.get_mut(&this) { *count = count.checked_add(1).unwrap(); }
    this
}
- (())release { _Block_release(env, this.cast().cast_const()); }
- (u32)retainCount { env.objc.blocks.blocks.get(&this).copied().unwrap_or(1) }
- (id)autorelease { () = msg_class![env; NSAutoreleasePool addObject:this]; this }
@end
};

fn _Block_copy(env: &mut Environment, block: ConstVoidPtr) -> id {
    let block: id = block.cast().cast_mut();
    if block == nil { return nil; }
    if word(env, block, 1) & GLOBAL != 0 { return block; }
    if let Some(count) = env.objc.blocks.blocks.get_mut(&block) {
        *count = count.checked_add(1).unwrap();
        return block;
    }
    let descriptor: id = crate::mem::Ptr::from_bits(word(env, block, 4));
    let size = word(env, descriptor, 1);
    assert!((20..=1_048_576).contains(&size), "Invalid guest block size");
    let copied: id = env.mem.alloc(size).cast();
    env.mem.memmove(copied.cast(), block.cast().cast_const(), size);
    let flags = word(env, block, 1);
    set_word(env, copied, 1, (flags & !0xffff) | NEEDS_FREE | 1);
    env.objc.blocks.blocks.insert(copied, 1);
    if flags & COPY_DISPOSE != 0 {
        let helper = GuestFunction::from_addr_with_thumb_bit(word(env, descriptor, 2));
        let _: () = helper.call_from_host(env, (copied, block));
    }
    copied
}
fn _Block_release(env: &mut Environment, block: ConstVoidPtr) {
    let block: id = block.cast().cast_mut();
    let Some(count) = env.objc.blocks.blocks.get_mut(&block) else { return; };
    *count -= 1;
    if *count != 0 { return; }
    env.objc.blocks.blocks.remove(&block);
    let flags = word(env, block, 1);
    if flags & COPY_DISPOSE != 0 {
        let descriptor: id = crate::mem::Ptr::from_bits(word(env, block, 4));
        let helper = GuestFunction::from_addr_with_thumb_bit(word(env, descriptor, 3));
        let _: () = helper.call_from_host(env, (block,));
    }
    env.mem.free(block.cast());
}

fn byref_copy(env: &mut Environment, original: id) -> id {
    let forwarded: id = crate::mem::Ptr::from_bits(word(env, original, 1));
    assert!(forwarded != nil);
    if let Some(count) = env.objc.blocks.byrefs.get_mut(&forwarded) {
        *count = count.checked_add(1).unwrap();
        return forwarded;
    }
    let size = word(env, forwarded, 3);
    assert!((16..=1_048_576).contains(&size), "Invalid guest byref size");
    let copied: id = env.mem.alloc(size).cast();
    env.mem.memmove(copied.cast(), forwarded.cast().cast_const(), size);
    let flags = word(env, forwarded, 2);
    set_word(env, copied, 1, copied.to_bits());
    set_word(env, forwarded, 1, copied.to_bits());
    set_word(env, copied, 2, (flags & !0xffff) | NEEDS_FREE | 2);
    // The stack scope and first copied block each own one reference.
    env.objc.blocks.byrefs.insert(copied, 2);
    if flags & COPY_DISPOSE != 0 {
        let helper = GuestFunction::from_addr_with_thumb_bit(word(env, copied, 4));
        let _: () = helper.call_from_host(env, (copied, forwarded));
    }
    copied
}
fn byref_release(env: &mut Environment, original: id) {
    let forwarded: id = crate::mem::Ptr::from_bits(word(env, original, 1));
    let Some(count) = env.objc.blocks.byrefs.get_mut(&forwarded) else { return; };
    *count -= 1;
    if *count != 0 { return; }
    env.objc.blocks.byrefs.remove(&forwarded);
    if word(env, forwarded, 2) & COPY_DISPOSE != 0 {
        let helper = GuestFunction::from_addr_with_thumb_bit(word(env, forwarded, 5));
        let _: () = helper.call_from_host(env, (forwarded,));
    }
    env.mem.free(forwarded.cast());
}
fn _Block_object_assign(env: &mut Environment, destination: MutPtr<id>, object: ConstVoidPtr, flags: i32) {
    let object: id = object.cast().cast_mut();
    let value = match flags & 0x9f {
        3 => retain(env, object),
        7 => _Block_copy(env, object.cast().cast_const()),
        8 | 24 => byref_copy(env, object),
        131 | 135 | 147 | 151 => object,
        _ => panic!("Unsupported guest block capture flags {flags:#x}"),
    };
    env.mem.write(destination, value);
}
fn _Block_object_dispose(env: &mut Environment, object: ConstVoidPtr, flags: i32) {
    match flags & 0x9f {
        3 => release(env, object.cast().cast_mut()),
        7 => _Block_release(env, object),
        8 | 24 => byref_release(env, object.cast().cast_mut()),
        131 | 135 | 147 | 151 => (),
        _ => panic!("Unsupported guest block dispose flags {flags:#x}"),
    }
}
fn objc_retainBlock(env: &mut Environment, object: id) -> id { _Block_copy(env, object.cast().cast_const()) }

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(_Block_copy(_)),
    export_c_func!(_Block_release(_)),
    export_c_func!(_Block_object_assign(_, _, _)),
    export_c_func!(_Block_object_dispose(_, _)),
    export_c_func!(objc_retainBlock(_)),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frameworks::foundation::{ns_data::to_rust_slice, ns_string::{from_rust_string, get_static_str, to_rust_string}};
    use crate::objc::msg;

    fn retain_count(env: &mut Environment, object: id) -> u32 { msg![env; object retainCount] }
    fn copy_object(env: &mut Environment, dst: id, src: id) {
        let object: id = crate::mem::Ptr::from_bits(word(env, src, 5));
        _Block_object_assign(env, dst.cast::<id>() + 5, object.cast().cast_const(), 3);
    }
    fn dispose_object(env: &mut Environment, block: id) {
        let object: id = crate::mem::Ptr::from_bits(word(env, block, 5));
        _Block_object_dispose(env, object.cast().cast_const(), 3);
    }
    fn copy_byref(env: &mut Environment, dst: id, src: id) {
        let object: id = crate::mem::Ptr::from_bits(word(env, src, 5));
        _Block_object_assign(env, dst.cast::<id>() + 5, object.cast().cast_const(), 8);
    }
    fn dispose_byref(env: &mut Environment, block: id) {
        let object: id = crate::mem::Ptr::from_bits(word(env, block, 5));
        _Block_object_dispose(env, object.cast().cast_const(), 8);
    }
    fn literal(env: &mut Environment, flags: u32, capture: id, byref: bool) -> (id, id) {
        let descriptor: id = env.mem.alloc(16).cast();
        let block: id = env.mem.alloc(24).cast();
        let copy = if byref { export_c_func!(copy_byref(_, _)).1 } else { export_c_func!(copy_object(_, _)).1 };
        let dispose = if byref { export_c_func!(dispose_byref(_)).1 } else { export_c_func!(dispose_object(_)).1 };
        let copy = env.dyld.create_guest_function(&mut env.mem, "test_block_copy", copy);
        let dispose = env.dyld.create_guest_function(&mut env.mem, "test_block_dispose", dispose);
        for (i, value) in [0, 24, copy.addr_with_thumb_bit(), dispose.addr_with_thumb_bit()].into_iter().enumerate() {
            set_word(env, descriptor, i as u32, value);
        }
        let isa = block_class(env).to_bits();
        for (i, value) in [isa, flags, 0, 0, descriptor.to_bits(), capture.to_bits()].into_iter().enumerate() {
            set_word(env, block, i as u32, value);
        }
        (block, descriptor)
    }

    #[test]
    fn arc_blocks_and_keyed_archive_lifetimes() {
        let options = crate::options::Options { headless: true, popup_errors: false, ..Default::default() };
        let icon = crate::image::Image::from_pixel_vec(vec![0; 4], (1, 1));
        Environment::new_without_app(options, icon).unwrap().run_app_picker(|env| {
            crate::libc::zlib::regression_fixture(env);
            crate::frameworks::audio_toolbox::audio_file::regression_fixture(env);
            crate::image::guest_pixel_packing::regression_fixture(env);
            // No app binaries, GUI window, input, networking or host keychain.
            assert!(env.bins.is_empty());
            assert!(env.window.is_none());
            let uname = env.dyld.create_proc_address(&mut env.mem, &mut env.cpu, "_uname").unwrap();
            let record = env.mem.alloc(5 * 256).cast::<u8>();
            env.mem.bytes_at_mut(record, 5 * 256).fill(255);
            let result: i32 = uname.call_from_host(env, (record,));
            assert_eq!(result, 0);
            for (index, expected) in ["Darwin", "touchHLE", "10.0.0d3", "7A341", "iPhone1,1"].into_iter().enumerate() {
                let field = record + (index as u32 * 256);
                assert_eq!(env.mem.cstr_at_utf8(field.cast_const()).unwrap(), expected);
                assert!(env.mem.bytes_at(field.cast_const() + expected.len() as u32, 256 - expected.len() as u32).iter().all(|&byte| byte == 0));
            }
            env.mem.free(record.cast());
            let fork = env.dyld.create_proc_address(&mut env.mem, &mut env.cpu, "_fork").unwrap();
            let child: i32 = fork.call_from_host(env, ());
            assert_eq!(child, -1);
            let errno = env.dyld.create_proc_address(&mut env.mem, &mut env.cpu, "___error").unwrap();
            let errno: MutPtr<i32> = errno.call_from_host(env, ());
            assert_eq!(env.mem.read(errno), crate::libc::errno::EPERM);
            let raw = env.mem.calloc(4).cast::<super::super::objects::objc_object>();
            let root = env.objc.get_known_class("NSObject", &mut env.mem);
            let assign = env.dyld.create_proc_address(&mut env.mem, &mut env.cpu, "_object_setClass").unwrap();
            let previous: id = assign.call_from_host(env, (raw, root));
            assert_eq!(previous, nil);
            assert_eq!(super::super::ObjC::read_isa(raw, &env.mem), root);
            assert_eq!(retain_count(env, raw), 1);
            let previous: id = assign.call_from_host(env, (raw, root));
            assert_eq!(previous, root);
            assert_eq!(retain_count(env, raw), 1);
            let previous: id = assign.call_from_host(env, (nil, root));
            assert_eq!(previous, nil);
            release(env, raw);
            let div = env.dyld.create_proc_address(&mut env.mem, &mut env.cpu, "___divdi3").unwrap();
            let quotient: i64 = div.call_from_host(env, (-0x1_0000_0001i64, 7i64));
            assert_eq!(quotient, -613_566_756);
            let quotient: i64 = div.call_from_host(env, (i64::MIN, -1i64));
            assert_eq!(quotient, i64::MIN);
            let modulo = env.dyld.create_proc_address(&mut env.mem, &mut env.cpu, "___modsi3").unwrap();
            let remainder: i32 = modulo.call_from_host(env, (-9i32, 4i32));
            assert_eq!(remainder, -1);
            let convert = env.dyld.create_proc_address(&mut env.mem, &mut env.cpu, "___fixdfdi").unwrap();
            let integer: i64 = convert.call_from_host(env, (-4294967297.75f64,));
            assert_eq!(integer, -4_294_967_297);
            let create = env.dyld.create_proc_address(&mut env.mem, &mut env.cpu, "_CFDataCreateMutable").unwrap();
            let mutable: id = create.call_from_host(env, (crate::mem::ConstVoidPtr::null(), 0i32));
            assert!(to_rust_slice(env, mutable).is_empty());
            let resize = env.dyld.create_proc_address(&mut env.mem, &mut env.cpu, "_CFDataSetLength").unwrap();
            let _: () = resize.call_from_host(env, (mutable, 3i32));
            assert_eq!(to_rust_slice(env, mutable), &[0, 0, 0]);
            let bytes = env.dyld.create_proc_address(&mut env.mem, &mut env.cpu, "_CFDataGetMutableBytePtr").unwrap();
            let pointer: MutPtr<u8> = bytes.call_from_host(env, (mutable,));
            env.mem.write(pointer + 1, 17u8);
            assert_eq!(to_rust_slice(env, mutable), &[0, 17, 0]);
            let _: () = resize.call_from_host(env, (mutable, 0i32));
            assert!(to_rust_slice(env, mutable).is_empty());
            release(env, mutable);
            let reserved: id = create.call_from_host(env, (crate::mem::ConstVoidPtr::null(), 44i32));
            let reserved_bytes: MutPtr<u8> = bytes.call_from_host(env, (reserved,));
            assert!(!reserved_bytes.is_null());
            assert!(to_rust_slice(env, reserved).is_empty());
            env.mem.write(reserved_bytes + 43, 255u8);
            assert_eq!(env.mem.read(reserved_bytes + 43), 255u8);
            release(env, reserved);
            let pool = super::super::objc_autoreleasePoolPush(env);
            let board_name = get_static_str(env, "isolated-guest-board");
            let missing: id = msg_class![env; UIPasteboard pasteboardWithName:board_name create:false];
            assert_eq!(missing, nil);
            let board: id = msg_class![env; UIPasteboard pasteboardWithName:board_name create:true];
            let stored = from_rust_string(env, "guest-only".to_string());
            () = msg![env; board setString:stored];
            release(env, stored);
            let found: id = msg_class![env; UIPasteboard pasteboardWithName:board_name create:false];
            assert_eq!(found, board);
            let contents: id = msg![env; found string];
            assert_eq!(to_rust_string(env, contents), "guest-only");
            () = msg_class![env; UIPasteboard removePasteboardWithName:board_name];
            assert!(env.objc.get_host_object(stored).is_none());
            let value = from_rust_string(env, "owned-value".to_string());
            let key = get_static_str(env, "copied-key");
            let dictionary: id = msg_class![env; NSMutableDictionary new];
            () = msg![env; dictionary setObject:value forKey:key];
            let enumerator: id = msg![env; dictionary keyEnumerator];
            let first: id = msg![env; enumerator nextObject];
            let last: id = msg![env; enumerator nextObject];
            assert_eq!(first, key); assert_eq!(last, nil);
            let copied: id = msg_class![env; NSDictionary alloc];
            let copied: id = msg![env; copied initWithDictionary:dictionary];
            release(env, dictionary); release(env, value);
            let copied_value: id = msg![env; copied objectForKey:key];
            assert_eq!(to_rust_string(env, copied_value), "owned-value");
            release(env, copied);
            assert!(env.objc.get_host_object(value).is_none());
            let manager: id = msg_class![env; ASIdentifierManager sharedManager];
            let again: id = msg_class![env; ASIdentifierManager sharedManager];
            assert_eq!(manager, again);
            assert!(!msg![env; manager isAdvertisingTrackingEnabled]);
            let identifier: id = msg![env; manager advertisingIdentifier];
            let uuid_string: id = msg![env; identifier UUIDString];
            assert_eq!(to_rust_string(env, uuid_string), "00000000-0000-0000-0000-000000000000");
            let uuid_bytes = env.mem.alloc(16).cast::<u8>();
            () = msg![env; identifier getUUIDBytes:uuid_bytes];
            assert_eq!(env.mem.bytes_at(uuid_bytes.cast_const(), 16), &[0; 16]);
            env.mem.free(uuid_bytes.cast());
            assert!(!env.options.network_access);
            let application: id = msg_class![env; UIApplication new];
            let scheme = get_static_str(env, "cydia://packages/probe");
            let external_url: id = msg_class![env; NSURL URLWithString:scheme];
            assert!(!msg![env; application canOpenURL:external_url]);
            assert!(!msg![env; application openURL:external_url]);
            let protected_path = get_static_str(env, "/private/peggle-runtime-probe.txt");
            let probe = get_static_str(env, "sandbox-probe");
            let error = env.mem.alloc_and_write(nil);
            let wrote: bool = msg![env; probe writeToFile:protected_path atomically:true encoding:(4u32) error:error];
            assert!(!wrote);
            let failure = env.mem.read(error);
            assert_ne!(failure, nil);
            let code: i32 = msg![env; failure code];
            assert_eq!(code, 512);
            assert!(!env.fs.exists(crate::fs::GuestPath::new("/private/peggle-runtime-probe.txt")));
            env.mem.free(error.cast());
            let utf8 = from_rust_string(env, "é😀".to_owned());
            let convert = env.dyld.create_proc_address(&mut env.mem, &mut env.cpu, "_CFStringGetBytes").unwrap();
            let bytes = env.mem.alloc(16).cast::<u8>();
            let used = env.mem.alloc_and_write(0i32);
            let range = crate::frameworks::core_foundation::CFRange { location: 0, length: 3 };
            let converted: i32 = convert.call_from_host(env, (utf8, range, 0x08000100u32, b'?', false, bytes, 16i32, used));
            assert_eq!(converted, 3);
            assert_eq!(env.mem.read(used), 6);
            assert_eq!(env.mem.bytes_at(bytes.cast_const(), 6), "é😀".as_bytes());
            let converted: i32 = convert.call_from_host(env, (utf8, range, 0x08000100u32, b'?', false, MutPtr::<u8>::null(), 0i32, used));
            assert_eq!(converted, 3); assert_eq!(env.mem.read(used), 6);
            let converted: i32 = convert.call_from_host(env, (utf8, range, 0x08000100u32, b'?', false, bytes, 5i32, used));
            assert_eq!(converted, 1); assert_eq!(env.mem.read(used), 2);
            release(env, utf8); env.mem.free(bytes.cast()); env.mem.free(used.cast());
            let object = from_rust_string(env, "captured".to_owned());
            let (stack, descriptor) = literal(env, COPY_DISPOSE, object, false);
            let copied: id = msg![env; stack copy];
            assert_ne!(copied, stack);
            assert_eq!(word(env, copied, 5), object.to_bits());
            assert_eq!(retain_count(env, object), 2u32);
            set_word(env, stack, 5, 0);
            assert_eq!(word(env, copied, 5), object.to_bits());
            let alias = _Block_copy(env, copied.cast().cast_const());
            assert_eq!(alias, copied);
            assert_eq!(env.objc.blocks.blocks[&copied], 2);
            _Block_release(env, alias.cast().cast_const());
            assert_eq!(env.objc.blocks.blocks[&copied], 1);
            _Block_release(env, copied.cast().cast_const());
            assert_eq!(retain_count(env, object), 1u32);
            assert!(env.objc.blocks.blocks.is_empty());
            env.mem.free(stack.cast()); env.mem.free(descriptor.cast());

            let (global, descriptor) = literal(env, GLOBAL, nil, false);
            assert_eq!(_Block_copy(env, global.cast().cast_const()), global);
            _Block_release(env, global.cast().cast_const());
            assert!(env.objc.blocks.blocks.is_empty());
            env.mem.free(global.cast()); env.mem.free(descriptor.cast());

            let byref: id = env.mem.alloc(20).cast();
            for (i, value) in [0, byref.to_bits(), 0, 20, 42].into_iter().enumerate() { set_word(env, byref, i as u32, value); }
            let (stack, descriptor) = literal(env, COPY_DISPOSE, byref, true);
            let a = _Block_copy(env, stack.cast().cast_const());
            let b = _Block_copy(env, stack.cast().cast_const());
            let shared: id = crate::mem::Ptr::from_bits(word(env, a, 5));
            assert_eq!(word(env, b, 5), shared.to_bits());
            assert_eq!(word(env, byref, 1), shared.to_bits());
            assert_eq!(env.objc.blocks.byrefs[&shared], 3);
            set_word(env, shared, 4, 99);
            assert_eq!(word(env, crate::mem::Ptr::from_bits(word(env, b, 5)), 4), 99);
            byref_release(env, byref);
            assert_eq!(env.objc.blocks.byrefs[&shared], 2);
            _Block_release(env, a.cast().cast_const());
            assert_eq!(env.objc.blocks.byrefs[&shared], 1);
            _Block_release(env, b.cast().cast_const());
            assert!(env.objc.blocks.byrefs.is_empty());
            assert!(env.objc.blocks.blocks.is_empty());
            env.mem.free(byref.cast()); env.mem.free(stack.cast()); env.mem.free(descriptor.cast());

            let slot = env.mem.alloc(4).cast::<id>();
            env.mem.write(slot, nil);
            super::super::objc_storeStrong(env, slot, object);
            assert_eq!(retain_count(env, object), 2u32);
            super::super::objc_storeStrong(env, slot, object);
            assert_eq!(retain_count(env, object), 2u32);
            super::super::objc_storeStrong(env, slot, nil);
            assert_eq!(retain_count(env, object), 1u32);
            env.mem.free(slot.cast());
            let inner = super::super::objc_autoreleasePoolPush(env);
            super::super::objc_autoreleaseReturnValue(env, object);
            super::super::objc_retainAutoreleasedReturnValue(env, object);
            super::super::objc_autoreleasePoolPop(env, inner);
            assert_eq!(retain_count(env, object), 1u32);

            let data: id = msg_class![env; NSMutableData new];
            let coder: id = msg_class![env; NSKeyedArchiver alloc];
            let coder: id = msg![env; coder initForWritingWithMutableData:data];
            () = msg![env; coder encodeObject:object];
            let int_key = get_static_str(env, "integer");
            let bool_key = get_static_str(env, "boolean");
            let data_key = get_static_str(env, "payload");
            let date_key = get_static_str(env, "date");
            () = msg![env; coder encodeInteger:(-42i32) forKey:int_key];
            () = msg![env; coder encodeBool:true forKey:bool_key];
            let payload = env.mem.alloc(3);
            env.mem.bytes_at_mut(payload.cast(), 3).copy_from_slice(&[1, 2, 255]);
            let blob: id = msg_class![env; NSData alloc];
            let blob: id = msg![env; blob initWithBytes:(payload.cast_const()) length:(3u32)];
            () = msg![env; coder encodeObject:blob forKey:data_key];
            let date: id = msg_class![env; NSDate alloc];
            let date: id = msg![env; date initWithTimeIntervalSinceReferenceDate:(123.25f64)];
            let adjusted: id = msg![env; date dateByAddingTimeInterval:(-60.0f64)];
            let original_interval: f64 = msg![env; date timeIntervalSinceReferenceDate];
            let adjusted_interval: f64 = msg![env; adjusted timeIntervalSinceReferenceDate];
            assert_eq!(original_interval, 123.25); assert_eq!(adjusted_interval, 63.25);
            let identifier = get_static_str(env, "en_US_POSIX");
            let locale: id = msg_class![env; NSLocale alloc];
            let locale: id = msg![env; locale initWithLocaleIdentifier:identifier];
            let reported_identifier: id = msg![env; locale localeIdentifier];
            assert_eq!(to_rust_string(env, reported_identifier), "en_US_POSIX");
            let country_key = get_static_str(env, "NSLocaleCountryCode");
            let language_key = get_static_str(env, "NSLocaleLanguageCode");
            let country: id = msg![env; locale objectForKey:country_key];
            let language: id = msg![env; locale objectForKey:language_key];
            assert_eq!(to_rust_string(env, country), "US");
            assert_eq!(to_rust_string(env, language), "en");
            let formatter: id = msg_class![env; NSDateFormatter new];
            () = msg![env; formatter setLocale:locale];
            release(env, locale);
            let format = get_static_str(env, "[yyyy-MM-dd HH:mm:ss.SSS]");
            () = msg![env; formatter setDateFormat:format];
            let formatted: id = msg![env; formatter stringFromDate:date];
            assert_eq!(to_rust_string(env, formatted), "[2001-01-01 00:02:03.250]");
            release(env, formatter);
            () = msg![env; coder encodeObject:date forKey:date_key];
            () = msg![env; coder finishEncoding];
            let length: u32 = msg![env; data length];
            assert!(length > 0);
            () = msg![env; coder finishEncoding];
            let second_length: u32 = msg![env; data length];
            assert_eq!(second_length, length);
            let decoder: id = msg_class![env; NSKeyedUnarchiver alloc];
            let decoder: id = msg![env; decoder initForReadingWithData:data];
            let decoded: id = msg![env; decoder decodeObject];
            assert_eq!(to_rust_string(env, decoded), "captured");
            let decoded_int: i32 = msg![env; decoder decodeIntegerForKey:int_key];
            assert_eq!(decoded_int, -42);
            assert!(msg![env; decoder decodeBoolForKey:bool_key]);
            let decoded_blob: id = msg![env; decoder decodeObjectForKey:data_key];
            assert_eq!(to_rust_slice(env, decoded_blob), &[1, 2, 255]);
            let decoded_date: id = msg![env; decoder decodeObjectForKey:date_key];
            let interval: f64 = msg![env; decoded_date timeIntervalSinceReferenceDate];
            assert_eq!(interval, 123.25);
            () = msg![env; decoder finishDecoding];
            release(env, decoder); release(env, coder); release(env, data);
            release(env, blob); release(env, date); release(env, object);
            env.mem.free(payload);
            let port: id = msg_class![env; NSMachPort new];
            let run_loop: id = msg_class![env; NSRunLoop currentRunLoop];
            let mode = get_static_str(env, crate::frameworks::foundation::ns_run_loop::NSDefaultRunLoopMode);
            assert!(msg![env; port isValid]);
            () = msg![env; run_loop addPort:port forMode:mode];
            () = msg![env; run_loop addPort:port forMode:mode];
            assert_eq!(retain_count(env, port), 2);
            () = msg![env; run_loop removePort:port forMode:mode];
            assert_eq!(retain_count(env, port), 1);
            () = msg![env; port invalidate];
            assert!(!msg![env; port isValid]);
            release(env, port);
            let frame = crate::frameworks::core_graphics::CGRect {
                origin: Default::default(), size: crate::frameworks::core_graphics::CGSize { width: 320.0, height: 480.0 }
            };
            let window: id = msg_class![env; UIWindow alloc];
            let window: id = msg![env; window initWithFrame:frame];
            let controller: id = msg_class![env; UIViewController new];
            let view: id = msg_class![env; UIView alloc];
            let view: id = msg![env; view initWithFrame:frame];
            () = msg![env; controller setView:view];
            () = msg![env; window setRootViewController:controller];
            assert_eq!(retain_count(env, controller), 2);
            () = msg![env; window setRootViewController:controller];
            assert_eq!(retain_count(env, controller), 2);
            let root: id = msg![env; window rootViewController];
            let parent: id = msg![env; view superview];
            assert_eq!(root, controller); assert_eq!(parent, window);
            () = msg![env; window setRootViewController:nil];
            assert_eq!(retain_count(env, controller), 1);
            let parent: id = msg![env; view superview];
            assert_eq!(parent, nil);
            release(env, view); release(env, controller); release(env, window);
            super::super::objc_autoreleasePoolPop(env, pool);
        });
    }
}
