/* This Source Code Form is subject to the Mozilla Public License, v. 2.0. */
//! Standard zlib HLE for a verified, statically linked guest library.
//! https://zlib.net/manual.html
//! Guest pointers and opaque stream tokens never contain host addresses.
use crate::dyld::{export_c_func, FunctionExports};
use crate::abi::CallFromHost;
use crate::mem::{ConstPtr, MutPtr, SafeRead};
use crate::Environment;
use std::collections::HashMap;
use std::ffi::{c_char, c_int, c_uint, c_ulong, c_void, CStr};

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct GuestStream {
    next_in: u32, avail_in: u32, total_in: u32,
    next_out: u32, avail_out: u32, total_out: u32,
    msg: u32, state: u32, zalloc: u32, zfree: u32, opaque: u32,
    data_type: i32, adler: u32, reserved: u32,
}
unsafe impl SafeRead for GuestStream {}

#[repr(C)]
struct NativeStream {
    next_in: *const u8, avail_in: c_uint, total_in: c_ulong,
    next_out: *mut u8, avail_out: c_uint, total_out: c_ulong,
    msg: *const c_char, state: *mut c_void,
    zalloc: Option<unsafe extern "C" fn(*mut c_void, c_uint, c_uint) -> *mut c_void>,
    zfree: Option<unsafe extern "C" fn(*mut c_void, *mut c_void)>,
    opaque: *mut c_void, data_type: c_int, adler: c_ulong, reserved: c_ulong,
}
#[link(name = "z")]
extern "C" {
    #[link_name = "zlibVersion"] fn native_version() -> *const c_char;
    #[link_name = "inflateInit2_"] fn native_init(s: *mut NativeStream, bits: c_int, version: *const c_char, size: c_int) -> c_int;
    #[link_name = "inflate"] fn native_inflate(s: *mut NativeStream, flush: c_int) -> c_int;
    #[link_name = "inflateEnd"] fn native_end(s: *mut NativeStream) -> c_int;
    #[link_name = "inflateReset"] fn native_reset(s: *mut NativeStream) -> c_int;
}
struct Stream {
    native: Box<NativeStream>,
    // One guest allocation: opaque state token plus a bounded error message.
    token: MutPtr<u8>,
}
impl Drop for Stream {
    fn drop(&mut self) { unsafe { native_end(&mut *self.native); } }
}
#[derive(Default)]
pub struct State {
    streams: HashMap<u32, Stream>,
    original_functions: HashMap<&'static str, crate::abi::GuestFunction>,
}

fn original_function(env: &Environment, name: &'static str) -> crate::abi::GuestFunction {
    *env.libc_state.zlib.original_functions.get(name).expect("Original zlib fallback unavailable")
}

fn inflateInit_(env: &mut Environment, ptr: MutPtr<GuestStream>, version: ConstPtr<u8>, size: i32) -> i32 {
    if !ptr.is_null() {
        let guest = env.mem.read(ptr);
        if guest.zalloc != 0 || guest.zfree != 0 {
            log_once!("Custom zlib allocation callbacks: preserving original guest library behavior");
            return original_function(env, "_inflateInit_").call_from_host(env, (ptr, version, size));
        }
    }
    inflateInit2_(env, ptr, 15, version, size)
}
fn inflateInit2_(env: &mut Environment, ptr: MutPtr<GuestStream>, bits: i32, version: ConstPtr<u8>, size: i32) -> i32 {
    if version.is_null() || size != 56 || env.mem.cstr_at(version).first() != Some(&b'1') { return -6; }
    if ptr.is_null() { return -2; }
    let mut guest = env.mem.read(ptr);
    // Preserve custom allocator behavior in the guest's original library.
    if guest.zalloc != 0 || guest.zfree != 0 {
        return original_function(env, "_inflateInit2_").call_from_host(env, (ptr, bits, version, size));
    }
    if env.libc_state.zlib.streams.contains_key(&ptr.to_bits()) { return -2; }
    let mut native: Box<NativeStream> = Box::new(unsafe { std::mem::zeroed() });
    let result = unsafe { native_init(&mut *native, bits, native_version(), std::mem::size_of::<NativeStream>() as i32) };
    if result != 0 { return result; }
    let token = env.mem.alloc(260).cast::<u8>();
    guest.total_in = 0; guest.total_out = 0; guest.msg = 0;
    guest.state = token.to_bits(); guest.adler = native.adler as u32;
    guest.data_type = native.data_type;
    env.mem.write(ptr, guest);
    env.libc_state.zlib.streams.insert(ptr.to_bits(), Stream { native, token });
    0
}
fn inflate(env: &mut Environment, ptr: MutPtr<GuestStream>, flush: i32) -> i32 {
    if ptr.is_null() { return -2; }
    let mut guest = env.mem.read(ptr);
    if !env.libc_state.zlib.streams.contains_key(&ptr.to_bits()) {
        return original_function(env, "_inflate").call_from_host(env, (ptr, flush));
    }
    let stream = env.libc_state.zlib.streams.get_mut(&ptr.to_bits()).unwrap();
    if guest.state != stream.token.to_bits() || guest.next_out == 0 || (guest.next_in == 0 && guest.avail_in != 0) { return -2; }
    // Bound each host operation, leaving remaining input/output for another
    // standard streaming call. Guest and host buffers are never aliased.
    let input_len = guest.avail_in.min(1024 * 1024);
    let output_len = guest.avail_out.min(8 * 1024 * 1024);
    let input = if input_len == 0 { Vec::new() } else { env.mem.bytes_at(ConstPtr::<u8>::from_bits(guest.next_in), input_len).to_vec() };
    let mut output = vec![0u8; output_len as usize];
    let native = &mut stream.native;
    native.next_in = input.as_ptr(); native.avail_in = input_len;
    native.next_out = output.as_mut_ptr(); native.avail_out = output_len;
    let result = unsafe { native_inflate(&mut **native, flush) };
    let consumed = input_len - native.avail_in;
    let produced = output_len - native.avail_out;
    if produced != 0 {
        env.mem.bytes_at_mut(MutPtr::<u8>::from_bits(guest.next_out), produced).copy_from_slice(&output[..produced as usize]);
    }
    guest.next_in = guest.next_in.wrapping_add(consumed); guest.avail_in -= consumed;
    guest.next_out = guest.next_out.wrapping_add(produced); guest.avail_out -= produced;
    guest.total_in = native.total_in as u32; guest.total_out = native.total_out as u32;
    guest.adler = native.adler as u32; guest.data_type = native.data_type;
    guest.msg = 0;
    if !native.msg.is_null() {
        let message = unsafe { CStr::from_ptr(native.msg) }.to_bytes();
        let count = message.len().min(255);
        let target = stream.token + 4;
        env.mem.bytes_at_mut(target, count as u32 + 1)[..count].copy_from_slice(&message[..count]);
        env.mem.write(target + count as u32, 0u8);
        guest.msg = target.to_bits();
    }
    // Drop borrowed pointers before their vectors leave scope.
    native.next_in = std::ptr::null(); native.next_out = std::ptr::null_mut();
    env.mem.write(ptr, guest);
    result
}
fn inflateEnd(env: &mut Environment, ptr: MutPtr<GuestStream>) -> i32 {
    if ptr.is_null() { return -2; }
    let mut guest = env.mem.read(ptr);
    if guest.state == 0 { return -2; }
    if !env.libc_state.zlib.streams.contains_key(&ptr.to_bits()) {
        return original_function(env, "_inflateEnd").call_from_host(env, (ptr,));
    }
    let stream = env.libc_state.zlib.streams.get(&ptr.to_bits()).unwrap();
    if guest.state != stream.token.to_bits() { return -2; }
    let stream = env.libc_state.zlib.streams.remove(&ptr.to_bits()).unwrap();
    env.mem.free(stream.token.cast());
    drop(stream);
    guest.state = 0; guest.msg = 0;
    env.mem.write(ptr, guest);
    0
}
fn inflateReset(env: &mut Environment, ptr: MutPtr<GuestStream>) -> i32 {
    if ptr.is_null() { return -2; }
    let mut guest = env.mem.read(ptr);
    if guest.state == 0 { return -2; }
    if !env.libc_state.zlib.streams.contains_key(&ptr.to_bits()) {
        return original_function(env, "_inflateReset").call_from_host(env, (ptr,));
    }
    let stream = env.libc_state.zlib.streams.get_mut(&ptr.to_bits()).unwrap();
    if guest.state != stream.token.to_bits() { return -2; }
    let result = unsafe { native_reset(&mut *stream.native) };
    if result == 0 {
        guest.total_in = 0; guest.total_out = 0; guest.msg = 0;
        guest.adler = stream.native.adler as u32; guest.data_type = stream.native.data_type;
        env.mem.write(ptr, guest);
    }
    result
}

const FUNCTIONS: FunctionExports = &[
    export_c_func!(inflateInit_(_, _, _)), export_c_func!(inflateInit2_(_, _, _, _)),
    export_c_func!(inflate(_, _)), export_c_func!(inflateEnd(_)), export_c_func!(inflateReset(_)),
];

fn original_trampoline(mem: &mut crate::mem::Mem, address: u32) -> crate::abi::GuestFunction {
    let prefix = mem.bytes_at(ConstPtr::<u8>::from_bits(address), 16).to_vec();
    let original = mem.alloc(24).cast::<u32>();
    mem.bytes_at_mut(original.cast::<u8>(), 16).copy_from_slice(&prefix);
    mem.write(original + 4, 0xf000_f8dfu32);
    mem.write(original + 5, (address + 16) | 1);
    crate::abi::GuestFunction::from_addr_with_thumb_bit(original.to_bits() | 1)
}

/// Install only for the preserved original executable, after linking and
/// before any guest code executes. Original files are never modified.
pub(crate) fn install_verified_peggle_hooks(bundle: &crate::bundle::Bundle, fs: &crate::fs::Fs,
    bins: &[crate::mach_o::MachO], mem: &mut crate::mem::Mem, dyld: &mut crate::dyld::Dyld, libc_state: &mut super::State) {
    use digest::Digest;
    if bundle.bundle_identifier() != "com.popcap.Peggle" || bundle.bundle_version() != "4824.14.07.16.01" { return; }
    let bytes = fs.read(bundle.executable_path()).expect("Verified zlib executable must be readable");
    assert_eq!(format!("{:x}", sha1::Sha1::digest(&bytes)), "0c20cdf40d1925fa9f8237556b753e968be764e9", "Unexpected original executable: refuse static zlib acceleration");
    let verified = [
        ("_inflateInit_", 0x26ad18, [0x80,0xb5,0x6f,0x46,0x84,0xb0,0x0f,0x23]),
        ("_inflateInit2_", 0x26ab4c, [0x80,0xb5,0x6f,0x46,0x8a,0xb0,0x40,0xf2]),
        ("_inflate", 0x26ad40, [0x80,0xb5,0x6f,0x46,0x86,0xb0,0x00,0x22]),
        ("_inflateEnd", 0x26aac4, [0x80,0xb5,0x6f,0x46,0x85,0xb0,0x00,0x21]),
        ("_inflateReset", 0x26aa3c, [0x80,0xb5,0x6f,0x46,0x83,0xb0,0x00,0x21]),
    ];
    // Validate every address and instruction before changing guest memory.
    for (symbol, address, prologue) in verified {
        assert_eq!(bins[0].exported_symbols.get(symbol), Some(&(address | 1)));
        assert_eq!(mem.bytes_at(ConstPtr::<u8>::from_bits(address), 8), prologue);
    }
    for (symbol, address, _) in verified {
        // Every first16-byte prefix has been inspected: no PC-relative
        // instruction, whole Thumb instructions, and no branch. A permanent
        // original-code trampoline avoids temporarily unhooking shared code
        // while another emulated thread might be scheduled.
        let original = original_trampoline(mem, address);
        libc_state.zlib.original_functions.insert(symbol, original);
        let (_, function) = FUNCTIONS.iter().find(|(name, _)| *name == symbol).unwrap();
        let target = dyld.create_guest_function(mem, symbol, *function);
        let ptr = MutPtr::<u32>::from_bits(address);
        // Thumb LDR.W PC,[PC,#0], then an interworking ARM host-call stub.
        mem.write(ptr, 0xf000_f8dfu32);
        mem.write(ptr + 1, target.addr_with_thumb_bit());
    }
    log!("Verified Peggle 1.7.2 standard zlib acceleration installed in guest memory; original executable untouched");
    crate::image::guest_pixel_packing::install_verified(mem, dyld);
}

#[cfg(test)]
pub(crate) fn regression_fixture(env: &mut Environment) {
    assert_eq!(std::mem::size_of::<GuestStream>(), 56);
    // Python/system-zlib generated compressed input for a repetitive Unicode
    // text fixture, with a known exact checksum and output. No app execution.
    let plain = "original assets stay original — ".repeat(300).into_bytes();
    let compressed = include_bytes!("zlib-stream-fixture.bin");
    let input = env.mem.alloc(compressed.len() as u32).cast::<u8>();
    env.mem.bytes_at_mut(input, compressed.len() as u32).copy_from_slice(compressed);
    let output = env.mem.alloc(plain.len() as u32 + 16).cast::<u8>();
    let ptr = env.mem.alloc_and_write(GuestStream::default());
    let version = env.mem.alloc_and_write_cstr(b"1.1.3");
    assert_eq!(inflateInit_(env, ptr, version.cast_const(), 55), -6);
    assert_eq!(inflateInit_(env, ptr, version.cast_const(), 56), 0);
    let mut fed = 0usize;
    let mut written = 0usize;
    for _ in 0..2000 {
        let mut guest = env.mem.read(ptr);
        if guest.avail_in == 0 && fed < compressed.len() {
            let count = 7.min(compressed.len() - fed);
            guest.next_in = (input + fed as u32).to_bits(); guest.avail_in = count as u32; fed += count;
        }
        guest.next_out = (output + written as u32).to_bits(); guest.avail_out = 113;
        env.mem.write(ptr, guest);
        let result = inflate(env, ptr, 0);
        let guest = env.mem.read(ptr);
        written = guest.total_out as usize;
        assert!(matches!(result, 0 | 1 | -5));
        if result == 1 { break; }
    }
    assert_eq!(written, plain.len());
    assert_eq!(env.mem.bytes_at(output.cast_const(), written as u32), plain);
    assert_eq!(env.mem.read(ptr).total_in as usize, compressed.len());
    assert_eq!(inflateReset(env, ptr), 0);
    let bad = env.mem.alloc(16).cast::<u8>();
    env.mem.bytes_at_mut(bad, 16).fill(0xff);
    let mut guest = env.mem.read(ptr);
    guest.next_in = bad.to_bits(); guest.avail_in = 16;
    guest.next_out = output.to_bits(); guest.avail_out = 16;
    env.mem.write(ptr, guest);
    assert_eq!(inflate(env, ptr, 0), -3);
    assert_ne!(env.mem.read(ptr).msg, 0);
    assert_eq!(inflateEnd(env, ptr), 0);
    assert_eq!(env.mem.read(ptr).state, 0);
    assert_eq!(inflateEnd(env, ptr), -2);
    assert!(env.libc_state.zlib.streams.is_empty());
    // Exercise the real Thumb trampoline and custom-stream routing with our
    // own tiny fixture code: MOVS R0,#123, seven NOPs, then BX LR.
    let fixture = env.mem.alloc(20).cast::<u16>();
    env.mem.write(fixture, 0x207bu16);
    for i in 1..8 { env.mem.write(fixture + i, 0xbf00u16); }
    env.mem.write(fixture + 8, 0x4770u16);
    env.mem.write(fixture + 9, 0xbf00u16);
    let trampoline = original_trampoline(&mut env.mem, fixture.to_bits());
    env.cpu.invalidate_cache_range(fixture.to_bits(), 20);
    env.cpu.invalidate_cache_range(trampoline.addr_without_thumb_bit(), 24);
    for (name, _) in FUNCTIONS { env.libc_state.zlib.original_functions.insert(name, trampoline); }
    let mut guest = env.mem.read(ptr);
    guest.zalloc = 0x1234; guest.zfree = 0x5678; guest.state = 0xabcd;
    env.mem.write(ptr, guest);
    assert_eq!(inflateInit_(env, ptr, version.cast_const(), 56), 123);
    assert_eq!(inflateInit2_(env, ptr, 15, version.cast_const(), 56), 123);
    assert_eq!(inflate(env, ptr, 0), 123);
    assert_eq!(inflateReset(env, ptr), 123);
    assert_eq!(inflateEnd(env, ptr), 123);
    assert!(env.libc_state.zlib.streams.is_empty());
    env.libc_state.zlib.original_functions.clear();
    env.mem.free(MutPtr::<u8>::from_bits(trampoline.addr_without_thumb_bit()).cast());
    env.mem.free(fixture.cast());
    env.mem.free(input.cast()); env.mem.free(output.cast()); env.mem.free(ptr.cast());
    env.mem.free(version.cast()); env.mem.free(bad.cast());
}
