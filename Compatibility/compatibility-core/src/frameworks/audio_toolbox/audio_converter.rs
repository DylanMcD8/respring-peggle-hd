/* This Source Code Form is subject to the Mozilla Public License, v. 2.0. */
//! Streaming PCM Audio Converter Services. Compressed input is handled by
//! Audio File Services before it reaches this converter.
use super::audio_unit::AudioBuffer;
use crate::abi::{CallFromHost, GuestFunction};
use crate::dyld::{export_c_func, FunctionExports};
use crate::frameworks::carbon_core::{OSStatus, paramErr};
use crate::frameworks::core_audio_types::{AudioStreamBasicDescription as Format,
    fourcc, kAudioFormatLinearPCM, kAudioFormatFlagIsFloat, kAudioFormatFlagIsBigEndian,
    kAudioFormatFlagIsSignedInteger, kAudioFormatFlagIsPacked};
use crate::mem::{ConstPtr, Mem, MutPtr, MutVoidPtr, SafeRead};
use crate::Environment;
use std::collections::HashMap;

const NONINTERLEAVED: u32 = 1 << 5;
const UNSUPPORTED: OSStatus = fourcc(b"fmt?") as i32;
#[repr(C, packed)]
pub struct OpaqueConverter { _filler: u8 }
unsafe impl SafeRead for OpaqueConverter {}
type ConverterRef = MutPtr<OpaqueConverter>;
#[derive(Clone)]
struct Converter { source: Format, destination: Format, pending: Vec<[f32; 2]>, position: f64 }
#[derive(Default)]
pub struct State { converters: HashMap<ConverterRef, Converter> }

fn supported(f: &Format) -> bool {
    let sample_rate = f.sample_rate;
    f.format_id == kAudioFormatLinearPCM && f.sample_rate.is_finite()
        && (1.0..=192000.0).contains(&sample_rate) && matches!(f.channels_per_frame, 1 | 2)
        && f.frames_per_packet == 1 && f.format_flags & kAudioFormatFlagIsPacked != 0
        && ((f.bits_per_channel == 32 && f.format_flags & kAudioFormatFlagIsFloat != 0)
            || (f.bits_per_channel == 16 && f.format_flags & kAudioFormatFlagIsSignedInteger != 0))
        && f.bytes_per_frame == (f.bits_per_channel / 8)
            * if f.format_flags & NONINTERLEAVED != 0 { 1 } else { f.channels_per_frame }
}

fn AudioConverterNew(env: &mut Environment, source: ConstPtr<Format>, destination: ConstPtr<Format>,
    output: MutPtr<ConverterRef>) -> OSStatus {
    if source.is_null() || destination.is_null() || output.is_null() { return paramErr; }
    env.mem.write(output, ConverterRef::null());
    let source = env.mem.read(source); let destination = env.mem.read(destination);
    log!("AudioConverterNew source={:?}, destination={:?}", source, destination);
    if !supported(&source) || !supported(&destination) { return UNSUPPORTED; }
    let handle = env.mem.alloc_and_write(OpaqueConverter { _filler: 0 });
    env.framework_state.audio_toolbox.audio_converter.converters.insert(handle,
        Converter { source, destination, pending: Vec::new(), position: 0.0 });
    env.mem.write(output, handle); 0
}

fn AudioConverterDispose(env: &mut Environment, handle: ConverterRef) -> OSStatus {
    if env.framework_state.audio_toolbox.audio_converter.converters.remove(&handle).is_none() { return paramErr; }
    env.mem.free(handle.cast()); 0
}

fn buffers(mem: &Mem, list: MutVoidPtr) -> Option<Vec<AudioBuffer>> {
    if list.is_null() { return None; }
    let count = mem.read(list.cast::<u32>());
    if !(1..=2).contains(&count) { return None; }
    Some((0..count).map(|i| mem.read((list.cast::<u8>() + 4 + i * 12).cast())).collect())
}

fn address(format: &Format, buffers: &[AudioBuffer], frame: u32, channel: u32) -> Option<MutPtr<u8>> {
    let (buffer, channel) = if format.format_flags & NONINTERLEAVED != 0 {
        (buffers.get(channel as usize)?, 0)
    } else { (buffers.first()?, channel) };
    let width = format.bits_per_channel / 8;
    let offset = frame.checked_mul(format.bytes_per_frame)?.checked_add(channel * width)?;
    if buffer.data.is_null() || offset.checked_add(width)? > buffer.data_byte_size { return None; }
    Some(buffer.data.cast::<u8>() + offset)
}

fn decode(bytes: &[u8], format: &Format) -> f32 {
    let big = format.format_flags & kAudioFormatFlagIsBigEndian != 0;
    if format.bits_per_channel == 32 {
        let data: [u8; 4] = bytes.try_into().unwrap();
        if big { f32::from_be_bytes(data) } else { f32::from_le_bytes(data) }
    } else {
        let data: [u8; 2] = bytes.try_into().unwrap();
        f32::from(if big { i16::from_be_bytes(data) } else { i16::from_le_bytes(data) }) / 32768.0
    }
}

fn interpolate(samples: &[[f32; 2]], position: f64) -> [f32; 2] {
    let index = position.floor() as usize;
    let a = samples[index]; let b = samples.get(index + 1).copied().unwrap_or(a);
    let fraction = (position - index as f64) as f32;
    [a[0] + (b[0] - a[0]) * fraction, a[1] + (b[1] - a[1]) * fraction]
}

fn AudioConverterFillComplexBuffer(env: &mut Environment, handle: ConverterRef,
    callback: GuestFunction, userdata: MutVoidPtr, packets: MutPtr<u32>, output: MutVoidPtr,
    _packet_descriptions: MutVoidPtr) -> OSStatus {
    if packets.is_null() || callback.to_ptr().is_null() { return paramErr; }
    static CALL_COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let diagnostic = env.options.print_fps && CALL_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed) < 8;
    if diagnostic { log!("AudioConverterFillComplexBuffer handle={:?}, callback={:?}, userdata={:?}, packets={:?}, output={:?}",
        handle, callback, userdata, packets, output); }
    let requested = env.mem.read(packets);
    env.mem.write(packets, 0);
    if requested > 32768 { return paramErr; }
    let Some(mut converter) = env.framework_state.audio_toolbox.audio_converter.converters.get(&handle).cloned()
        else { return paramErr; };
    let Some(mut output_buffers) = buffers(&env.mem, output) else { return paramErr; };
    let format = converter.destination;
    let channels = format.channels_per_frame;
    let width = format.bits_per_channel / 8;
    if requested > 0 && (0..channels).any(|ch| address(&format, &output_buffers, requested - 1, ch).is_none()) {
        return paramErr;
    }
    let ratio = converter.source.sample_rate / format.sample_rate;
    let mut produced = 0; let mut input_status = 0; let mut eof = false;
    while produced < requested {
        if !eof && converter.position + 1.0 >= converter.pending.len() as f64 {
            let needed = (((requested - produced) as f64 * ratio).ceil() as u32 + 2).min(32768);
            let count = if converter.source.format_flags & NONINTERLEAVED != 0 {
                converter.source.channels_per_frame } else { 1 };
            let list = env.mem.alloc(28);
            env.mem.bytes_at_mut(list.cast(), 28).fill(0);
            env.mem.write(list.cast::<u32>(), count);
            let input_packets = env.mem.alloc_and_write(needed);
            input_status = callback.call_from_host(env, (handle, input_packets, list, MutVoidPtr::null(), userdata));
            if diagnostic { log!("AudioConverter input returned {}, packets={}", input_status, env.mem.read(input_packets)); }
            let available = env.mem.read(input_packets).min(32768);
            let input_buffers = buffers(&env.mem, list);
            if input_status == 0 && available != 0 {
                if let Some(input_buffers) = input_buffers {
                    for frame in 0..available {
                        let mut samples = [0.0; 2];
                        for channel in 0..2 {
                            let source_channel = channel.min(converter.source.channels_per_frame - 1);
                            let Some(pointer) = address(&converter.source, &input_buffers, frame, source_channel) else {
                                input_status = paramErr; break;
                            };
                            samples[channel as usize] = decode(env.mem.bytes_at(pointer.cast(),
                                converter.source.bits_per_channel / 8), &converter.source);
                        }
                        if input_status != 0 { break; }
                        converter.pending.push(samples);
                    }
                } else { input_status = paramErr; }
            }
            env.mem.free(input_packets.cast()); env.mem.free(list);
            eof = available == 0 || input_status != 0;
            if input_status != 0 { break; }
        }
        if converter.position >= converter.pending.len() as f64 { break; }
        let sample = interpolate(&converter.pending, converter.position);
        for channel in 0..channels {
            let sample = if channels == 1 { (sample[0] + sample[1]) * 0.5 } else { sample[channel as usize] };
            let pointer = address(&format, &output_buffers, produced, channel).unwrap();
            let big = format.format_flags & kAudioFormatFlagIsBigEndian != 0;
            let bytes = if width == 4 {
                if big { sample.to_be_bytes().to_vec() } else { sample.to_le_bytes().to_vec() }
            } else {
                let integer = (sample.clamp(-1.0, 1.0) * 32768.0).round().clamp(-32768.0, 32767.0) as i16;
                if big { integer.to_be_bytes().to_vec() } else { integer.to_le_bytes().to_vec() }
            };
            env.mem.bytes_at_mut(pointer.cast(), width).copy_from_slice(&bytes);
        }
        produced += 1; converter.position += ratio;
    }
    let consumed = (converter.position.floor() as usize).min(converter.pending.len());
    converter.pending.drain(..consumed);
    converter.position -= consumed as f64;
    if eof && converter.pending.is_empty() { converter.position = 0.0; }
    for (index, buffer) in output_buffers.iter_mut().enumerate() {
        buffer.data_byte_size = produced * format.bytes_per_frame;
        env.mem.write((output.cast::<u8>() + 4 + index as u32 * 12).cast(), *buffer);
    }
    env.mem.write(packets, produced);
    env.framework_state.audio_toolbox.audio_converter.converters.insert(handle, converter);
    if diagnostic { log!("AudioConverter output packets={}, status={}", produced, input_status); }
    input_status
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(AudioConverterNew(_, _, _)), export_c_func!(AudioConverterDispose(_)),
    export_c_func!(AudioConverterFillComplexBuffer(_, _, _, _, _, _)),
];

#[cfg(test)]
mod tests {
    #[test]
    fn pcm_interpolation_keeps_channels_separate_and_holds_final_frame() {
        let samples = [[-1.0, 1.0], [1.0, -1.0]];
        assert_eq!(super::interpolate(&samples, 0.5), [0.0, 0.0]);
        assert_eq!(super::interpolate(&samples, 0.25), [-0.5, 0.5]);
        assert_eq!(super::interpolate(&samples, 1.75), [1.0, -1.0]);
    }
}
