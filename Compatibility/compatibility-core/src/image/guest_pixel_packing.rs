/* This Source Code Form is subject to the Mozilla Public License, v. 2.0. */
//! Ahead-of-time translation of three verified pixel-packing loops.
//! Only installed after the complete original executable identity check.
//! No game rules, resources, save formats, or UI behavior are substituted.
#[cfg(test)]
use crate::abi::CallFromHost;
use crate::cpu::Cpu;
use crate::dyld::{export_c_func, FunctionExports};
use crate::mem::{ConstPtr, MutPtr};
use crate::Environment;

#[derive(Clone, Copy)]
enum Packing { Rgba4444, Rgb565, Rgba5551 }
fn packed(pixel: u32, format: Packing) -> u16 {
    let [r, g, b, a] = pixel.to_le_bytes();
    match format {
        Packing::Rgba4444 => ((r as u16 >> 4) << 12) | ((g as u16 >> 4) << 8) | ((b as u16 >> 4) << 4) | (a as u16 >> 4),
        Packing::Rgb565 => ((r as u16 >> 3) << 11) | ((g as u16 >> 2) << 5) | (b as u16 >> 3),
        Packing::Rgba5551 => ((r as u16 >> 3) << 11) | ((g as u16 >> 3) << 6) | ((b as u16 >> 3) << 1) | (a as u16 >> 7),
    }
}
fn pack_loop(env: &mut Environment, format: Packing, destination_slot: u32) {
    let stack = MutPtr::<u32>::from_bits(env.cpu.regs()[Cpu::SP]);
    let count = env.mem.read(stack + 0x64 / 4) as i32;
    let source = env.mem.read(stack + 0x68 / 4);
    assert!(count <= 16_777_216, "Reject invalid original pixel count");
    let pixels = count.max(0) as u32;
    // Original source/destination overlap: copy input before compacting it.
    let input = if pixels == 0 { Vec::new() } else {
        env.mem.bytes_at(ConstPtr::<u8>::from_bits(source), pixels * 4).to_vec()
    };
    if pixels != 0 {
        let output = env.mem.bytes_at_mut(MutPtr::<u8>::from_bits(source), pixels * 2);
        for (input, output) in input.chunks_exact(4).zip(output.chunks_exact_mut(2)) {
            output.copy_from_slice(&packed(u32::from_le_bytes(input.try_into().unwrap()), format).to_le_bytes());
        }
        env.cpu.regs_mut()[2] = source + pixels * 2;
    }
    env.mem.write(stack + destination_slot / 4, source + pixels * 2);
    env.mem.write(stack + 0x68 / 4, source + pixels * 4);
    let final_count = if count > 0 { -1 } else { count.wrapping_sub(1) };
    env.mem.write(stack + 0x64 / 4, final_count as u32);
    // Match the final original CMP against zero and its live registers.
    env.cpu.regs_mut()[0] = count.min(0) as u32;
    env.cpu.regs_mut()[1] = final_count as u32;
    let flags = if count >= 0 { 0x6000_0000 } else { 0xa000_0000 };
    let cpsr = (env.cpu.cpsr() & !0xf000_0000) | flags;
    env.cpu.set_cpsr(cpsr);
}
fn pack4444(env: &mut Environment) { pack_loop(env, Packing::Rgba4444, 0x60); }
fn pack565(env: &mut Environment) { pack_loop(env, Packing::Rgb565, 0x5c); }
fn pack5551(env: &mut Environment) { pack_loop(env, Packing::Rgba5551, 0x58); }
const FUNCTIONS: FunctionExports = &[
    export_c_func!(pack4444()), export_c_func!(pack565()), export_c_func!(pack5551()),
];

pub(crate) fn install_verified(mem: &mut crate::mem::Mem, dyld: &mut crate::dyld::Dyld) {
    let blocks = [
        (0x24929a, 0x249328, 0x18, FUNCTIONS[0]),
        (0x24932a, 0x24939c, 0x17, FUNCTIONS[1]),
        (0x24939e, 0x24942c, 0x16, FUNCTIONS[2]),
    ];
    for (address, _, slot, _) in blocks {
        assert_eq!(address % 4, 2);
        assert_eq!(mem.bytes_at(ConstPtr::<u8>::from_bits(address), 10),
            [0x1a,0x98,slot,0x90,0x19,0x98,0x00,0xf1,0xff,0x31]);
    }
    for (address, continuation, _, (name, function)) in blocks {
        let target = dyld.create_guest_block(mem, name, function, continuation | 1);
        let code = MutPtr::<u16>::from_bits(address);
        // PC for this halfword-aligned Thumb instruction is aligned down:
        // LDR.W PC,[PC,#4], NOP, then an aligned ARM target literal.
        mem.write(code, 0xf8dfu16); mem.write(code + 1, 0xf004u16);
        mem.write(code + 2, 0xbf00u16);
        mem.write(MutPtr::<u32>::from_bits(address + 6), target.addr_with_thumb_bit());
    }
    log!("Verified original RGBA4444/RGB565/RGBA5551 packing loops accelerated; original files untouched");
}

#[cfg(test)]
pub(crate) fn regression_fixture(env: &mut Environment) {
    assert_eq!(packed(0xdeadbeef, Packing::Rgba4444), 0xebad);
    assert_eq!(packed(0xffffffff, Packing::Rgb565), 0xffff);
    assert_eq!(packed(0xffffffff, Packing::Rgba5551), 0xffff);
    assert_eq!(packed(0x000000ff, Packing::Rgba5551), 0xf800);
    let old_sp = env.cpu.regs()[Cpu::SP];
    let stack = env.mem.alloc(256).cast::<u32>();
    let buffer = env.mem.alloc(32).cast::<u32>();
    let continuation = env.mem.alloc_and_write(0xbf004770u32);
    let original = env.mem.alloc(16).cast::<u16>();
    // Own fixture Thumb entry, using the same halfword-aligned dispatch as
    // the verified blocks; native helper resumes at an own BX LR fixture.
    let address = original.to_bits() + 2;
    let input = [0xdeadbeef, 0xffffffff, 0x00000000, 0x000000ff];
    for (index, (name, function)) in FUNCTIONS.iter().enumerate() {
        let format = [Packing::Rgba4444, Packing::Rgb565, Packing::Rgba5551][index];
        let target = env.dyld.create_guest_block(&mut env.mem, name, *function, continuation.to_bits() | 1);
        env.mem.write(MutPtr::<u16>::from_bits(address), 0xf8dfu16);
        env.mem.write(MutPtr::<u16>::from_bits(address + 2), 0xf004u16);
        env.mem.write(MutPtr::<u16>::from_bits(address + 4), 0xbf00u16);
        env.mem.write(MutPtr::<u32>::from_bits(address + 6), target.addr_with_thumb_bit());
        for count in [4i32, 0, -1] {
            for (i, pixel) in input.iter().enumerate() { env.mem.write(buffer + i as u32, *pixel); }
            env.mem.write(stack + 0x64 / 4, count as u32);
            env.mem.write(stack + 0x68 / 4, buffer.to_bits());
            // call_from_host adds its own8-byte diagnostic frame. The real
            // in-function dispatch has no such extra frame.
            env.cpu.regs_mut()[Cpu::SP] = stack.to_bits() + 8;
            env.cpu.invalidate_cache_range(address, 10);
            env.cpu.invalidate_cache_range(target.addr_without_thumb_bit(), 16);
            env.cpu.invalidate_cache_range(continuation.to_bits(), 4);
            let entry = crate::abi::GuestFunction::from_addr_with_thumb_bit(address | 1);
            let (): () = entry.call_from_host(env, ());
            let pixels = count.max(0) as u32;
            assert_eq!(env.mem.read(stack + 0x64 / 4), (if count > 0 { -1i32 } else { count - 1 }) as u32);
            assert_eq!(env.mem.read(stack + 0x68 / 4), buffer.to_bits() + pixels * 4);
            assert_eq!(env.cpu.regs()[0], count.min(0) as u32);
            assert_eq!(env.cpu.regs()[1], (if count > 0 { -1i32 } else { count - 1 }) as u32);
            assert_eq!(env.cpu.cpsr() & 0xf000_0000, if count >= 0 { 0x6000_0000 } else { 0xa000_0000 });
            let slot = [0x60,0x5c,0x58][index];
            assert_eq!(env.mem.read(stack + slot / 4), buffer.to_bits() + pixels * 2);
            if pixels != 0 {
                for (i, pixel) in input.iter().enumerate() { assert_eq!(env.mem.read(buffer.cast::<u16>() + i as u32), packed(*pixel, format)); }
                assert_eq!(env.mem.read(buffer + 2), input[2]);
            } else { assert_eq!(env.mem.read(buffer), input[0]); }
        }
        env.mem.free(MutPtr::<u8>::from_bits(target.addr_without_thumb_bit()).cast());
    }
    env.cpu.regs_mut()[Cpu::SP] = old_sp;
    env.mem.free(stack.cast()); env.mem.free(buffer.cast());
    env.mem.free(continuation.cast()); env.mem.free(original.cast());
}
