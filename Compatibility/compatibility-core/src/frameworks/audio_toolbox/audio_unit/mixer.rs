/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! RemoteIO connections and the multi-channel mixer's PCM render graph.
use super::{AudioBuffer, AudioUnit, AURenderCallbackStruct, audio_components};
use crate::abi::CallFromHost;
use crate::frameworks::audio_toolbox::audio_components::AudioUnitKind;
use crate::frameworks::carbon_core::{OSStatus, paramErr};
use crate::frameworks::core_audio_types::{AudioStreamBasicDescription, AudioTimeStamp, SMPTETime,
    kAudioFormatLinearPCM, kAudioFormatFlagIsFloat, kAudioFormatFlagIsSignedInteger};
use crate::mem::{ConstPtr, Mem, MutPtr, MutVoidPtr};
use crate::Environment;

const FORMAT_ERROR: OSStatus = -10868;
const kAudioFormatFlagIsNonInterleaved: u32 = 1 << 5;

pub(super) fn new_timestamp(mem: &mut Mem, sample_time: f64) -> MutPtr<AudioTimeStamp> {
    mem.alloc_and_write(AudioTimeStamp {
        sample_time, host_time: 0, rate_scalar: 1.0, world_clock_type: 0,
        SMPTE_time: SMPTETime { subframes: 0, subframe_divisor: 0, counter: 0,
            type_: 0, flags: 0, hours: 0, minutes: 0, seconds: 0, frames: 0 },
        flags: 1, _reserved: 0,
    })
}

pub(super) fn AudioUnitSetParameter(env: &mut Environment, unit: AudioUnit, parameter: u32,
    scope: u32, element: u32, value: f32, offset_frames: u32) -> OSStatus {
    if offset_frames != 0 || !value.is_finite() { return paramErr; }
    let Some(unit) = audio_components::State::get(&mut env.framework_state)
        .audio_component_instances.get_mut(&unit) else { return paramErr; };
    if unit.kind == AudioUnitKind::RemoteIO { return paramErr; }
    if scope == 2 && element == 0 && parameter == 0 { unit.output_volume = value; return 0; }
    if scope != 1 { return paramErr; }
    let Some(bus) = unit.input_buses.get_mut(element as usize) else { return paramErr; };
    if unit.kind == AudioUnitKind::Embedded3DMixer {
        match parameter {
            0 => bus.pan = value.to_radians().sin(),
            3 => bus.volume = 10.0f32.powf(value / 20.0),
            5 | 20 => { bus.enabled = value != 0.0; if !bus.enabled { bus.rate_state.clear(); } },
            4 => { if !crate::audio_rate::RateState::valid_rate(value as f64) { return paramErr; } },
            1 | 2 | 6..=11 => {},
            _ => return paramErr,
        }
        bus.parameters.insert(parameter, value);
        return 0;
    }
    match parameter { 0 => bus.volume = value, 1 => bus.enabled = value != 0.0,
        2 => bus.pan = value.clamp(-1.0, 1.0), _ => return paramErr }
    0
}

pub(super) fn AudioUnitGetParameter(env: &mut Environment, unit: AudioUnit, parameter: u32,
    scope: u32, element: u32, value: MutPtr<f32>) -> OSStatus {
    let Some(unit) = audio_components::State::get(&mut env.framework_state)
        .audio_component_instances.get(&unit) else { return paramErr; };
    let result = if scope == 2 && element == 0 && parameter == 0 { unit.output_volume }
    else {
        if scope != 1 { return paramErr; }
        let Some(bus) = unit.input_buses.get(element as usize) else { return paramErr; };
        if unit.kind == AudioUnitKind::Embedded3DMixer {
            let default = match parameter { 0 | 1 | 3 | 8..=11 => 0.0, 2 | 4 | 5 | 20 => 1.0,
                6 => -120.0, 7 => 20.0, _ => return paramErr };
            env.mem.write(value, bus.parameters.get(&parameter).copied().unwrap_or(default));
            return 0;
        }
        match parameter { 0 => bus.volume, 1 => if bus.enabled { 1.0 } else { 0.0 },
            2 => bus.pan, _ => return paramErr }
    };
    env.mem.write(value, result);
    0
}

fn buffers(mem: &Mem, data: MutVoidPtr) -> Option<Vec<AudioBuffer>> {
    if data.is_null() { return None; }
    let count = mem.read(data.cast::<u32>());
    if count == 0 || count > 2 { return None; }
    let start = (data.cast::<u8>() + 4).cast::<AudioBuffer>();
    Some((0..count).map(|i| mem.read(start + i)).collect())
}

fn supported(format: &AudioStreamBasicDescription) -> bool {
    let channels = format.channels_per_frame;
    format.format_id == kAudioFormatLinearPCM && (1..=2).contains(&channels)
        && ((format.bits_per_channel == 32 && format.format_flags & kAudioFormatFlagIsFloat != 0)
            || (format.bits_per_channel == 16 && format.format_flags & kAudioFormatFlagIsSignedInteger != 0))
        // This graph currently handles native little-endian packed PCM.
        && format.format_flags & 2 == 0
}

pub(super) fn float_output_pcm(mem: &Mem, format: &AudioStreamBasicDescription,
    data: MutVoidPtr, frames: u32) -> Option<Vec<u8>> {
    let buffers = buffers(mem, data)?;
    let mut pcm = Vec::with_capacity(frames as usize * format.channels_per_frame as usize * 2);
    for frame in 0..frames {
        for channel in 0..format.channels_per_frame {
            let address = sample_address(format, &buffers, frame, channel)?;
            let value: f32 = mem.read(address.cast());
            let value = (value.clamp(-1.0, 1.0) * 32767.0) as i16;
            pcm.extend_from_slice(&value.to_le_bytes());
        }
    }
    Some(pcm)
}

fn sample_address(format: &AudioStreamBasicDescription, buffers: &[AudioBuffer], frame: u32,
    channel: u32) -> Option<MutPtr<u8>> {
    let separate = format.format_flags & kAudioFormatFlagIsNonInterleaved != 0;
    let index = if separate { channel as usize } else { 0 };
    let buffer = buffers.get(index)?;
    let channels = if separate { 1 } else { format.channels_per_frame };
    let sample = frame.checked_mul(channels)?.checked_add(if separate { 0 } else { channel })?;
    let bytes = format.bits_per_channel / 8;
    let offset = sample.checked_mul(bytes)?;
    if buffer.data.is_null() || offset.checked_add(bytes)? > buffer.data_byte_size { return None; }
    Some(buffer.data.cast::<u8>() + offset)
}

pub(super) fn AudioUnitRender(env: &mut Environment, unit: AudioUnit, flags: MutPtr<u32>,
    timestamp: ConstPtr<AudioTimeStamp>, bus: u32, frames: u32, data: MutVoidPtr) -> OSStatus {
    render(env, unit, flags, timestamp, bus, frames, data, 0)
}

fn render(env: &mut Environment, handle: AudioUnit, flags: MutPtr<u32>,
    timestamp: ConstPtr<AudioTimeStamp>, bus_number: u32, frames: u32,
    data: MutVoidPtr, depth: usize) -> OSStatus {
    if depth > 8 || frames > 16384 || flags.is_null() { return paramErr; }
    let Some(unit) = audio_components::State::get(&mut env.framework_state)
        .audio_component_instances.get(&handle).cloned() else { return paramErr; };
    let original_flags = env.mem.read(flags);
    env.mem.write(flags, original_flags | (1 << 2));
    for notification in &unit.render_notifications {
        let AURenderCallbackStruct { input_proc, input_proc_ref_con } = *notification;
        let _: OSStatus = input_proc.call_from_host(env,
            (input_proc_ref_con, flags, timestamp, bus_number, frames, data));
    }
    env.mem.write(flags, original_flags);
    // Pre-render callbacks apply the original game's pending pitch and enable
    // updates. Read the current graph after those callbacks have returned.
    let Some(unit)=audio_components::State::get(&mut env.framework_state)
        .audio_component_instances.get(&handle).cloned() else { return paramErr; };
    let status = match unit.kind {
        AudioUnitKind::RemoteIO => {
            if let Some(connection) = unit.connection {
                render(env, connection.source_unit, flags, timestamp, connection.source_output, frames, data, depth + 1)
            } else if let Some(callback) = unit.render_callback {
                let AURenderCallbackStruct { input_proc, input_proc_ref_con } = callback;
                input_proc.call_from_host(env,
                    (input_proc_ref_con, flags, timestamp, bus_number, frames, data))
            } else { paramErr }
        },
        AudioUnitKind::MultiChannelMixer | AudioUnitKind::Embedded3DMixer => {
            let output = unit.output_stream_format.unwrap_or(unit.global_stream_format);
            if !supported(&output) { return FORMAT_ERROR; }
            let Some(output_buffers) = buffers(&env.mem, data) else { return paramErr; };
            let mut mixed = vec![0.0f32; frames as usize * output.channels_per_frame as usize];
            for (index, input) in unit.input_buses.iter().enumerate() {
                let Some(callback) = input.callback else { continue; };
                if !input.enabled || input.volume == 0.0 { continue; }
                let format = input.format.unwrap_or(unit.global_stream_format);
                if !supported(&format) || format.sample_rate != output.sample_rate {
                    log!("Unsupported mixer input format/rate on bus {}: {:?}, output {:?}", index, format, output);
                    return FORMAT_ERROR;
                }
                let rate = if unit.kind == AudioUnitKind::Embedded3DMixer {
                    f64::from(input.parameters.get(&4).copied().unwrap_or(1.0))
                } else { 1.0 };
                let mut rate_state=input.rate_state.clone();
                let Some(required)=rate_state.additional_frames(frames as usize,rate,format.channels_per_frame as usize) else { return paramErr; };
                if required>0 {
                    let input_frames=required as u32;
                    let separate = format.format_flags & kAudioFormatFlagIsNonInterleaved != 0;
                    let count = if separate { format.channels_per_frame } else { 1 };
                    let channels = if separate { 1 } else { format.channels_per_frame };
                    let size = input_frames * channels * (format.bits_per_channel / 8);
                    let list = env.mem.alloc(4 + count * 12);
                    env.mem.write(list.cast::<u32>(), count);
                    let mut allocations = Vec::new();
                    for i in 0..count {
                        let buffer = env.mem.alloc(size);
                        env.mem.bytes_at_mut(buffer.cast(), size).fill(0);
                        env.mem.write((list.cast::<u8>() + 4 + i * 12).cast(), AudioBuffer {
                            number_channels: channels, data_byte_size: size, data: buffer });
                        allocations.push(buffer);
                    }
                    let input_flags = env.mem.alloc_and_write(0u32);
                    let AURenderCallbackStruct { input_proc, input_proc_ref_con } = callback;
                    let result: OSStatus = input_proc.call_from_host(env,
                        (input_proc_ref_con, input_flags, timestamp, index as u32, input_frames, list));
                    let mut pcm=vec![0.0;required*format.channels_per_frame as usize];
                    if result == 0 && env.mem.read(input_flags) & (1 << 4) == 0 {
                        let input_buffers = buffers(&env.mem, list).unwrap();
                        for frame in 0..input_frames {
                            for channel in 0..format.channels_per_frame {
                                let Some(address)=sample_address(&format,&input_buffers,frame,channel) else { continue; };
                                pcm[(frame*format.channels_per_frame+channel) as usize]=if format.format_flags & kAudioFormatFlagIsFloat != 0 {
                                    env.mem.read(address.cast::<f32>())
                                } else { f32::from(env.mem.read(address.cast::<i16>()))/32768.0 };
                            }
                        }
                    }
                    for allocation in allocations { env.mem.free(allocation); }
                    env.mem.free(input_flags.cast()); env.mem.free(list);
                    if result!=0 { return result; }
                    if !rate_state.append(&pcm) { return paramErr; }
                }
                let Some(samples)=rate_state.render(frames as usize,rate) else { return paramErr; };
                for frame in 0..frames {
                    for channel in 0..output.channels_per_frame {
                        let source_channel=channel.min(format.channels_per_frame-1);
                        let sample=samples[(frame*format.channels_per_frame+source_channel) as usize];
                        let pan=if output.channels_per_frame==1 {1.0}
                            else if channel==0 {1.0-input.pan.max(0.0)} else {1.0+input.pan.min(0.0)};
                        mixed[(frame*output.channels_per_frame+channel) as usize]+=sample*input.volume*pan;
                    }
                }
                if let Some(current)=audio_components::State::get(&mut env.framework_state)
                    .audio_component_instances.get_mut(&handle).and_then(|u|u.input_buses.get_mut(index)) {
                    if current.enabled { current.rate_state=rate_state; }
                }

            }
            for frame in 0..frames {
                for channel in 0..output.channels_per_frame {
                    let Some(address) = sample_address(&output, &output_buffers, frame, channel) else { return paramErr; };
                    let sample = mixed[(frame * output.channels_per_frame + channel) as usize] * unit.output_volume;
                    if output.format_flags & kAudioFormatFlagIsFloat != 0 { env.mem.write(address.cast::<f32>(), sample); }
                    else { env.mem.write(address.cast::<i16>(), (sample.clamp(-1.0, 1.0) * 32767.0) as i16); }
                }
            }
            0
        },
    };
    env.mem.write(flags, original_flags | (1 << 3) | if status == 0 { 0 } else { 1 << 8 });
    for notification in &unit.render_notifications {
        let AURenderCallbackStruct { input_proc, input_proc_ref_con } = *notification;
        let _: OSStatus = input_proc.call_from_host(env,
            (input_proc_ref_con, flags, timestamp, bus_number, frames, data));
    }
    env.mem.write(flags, original_flags);
    if status == 0 {
        if let Some(unit) = audio_components::State::get(&mut env.framework_state)
            .audio_component_instances.get_mut(&handle) {
            unit.rendered_sample_time += f64::from(frames);
        }
    }
    status
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn float_pcm_interleaves_channels_clips_and_rejects_short_buffers() {
        let mut mem = Mem::new();
        let _null_page = mem.alloc(4096);
        let left = mem.alloc(8); let right = mem.alloc(8);
        mem.write(left.cast::<f32>(), 2.0); mem.write(left.cast::<f32>() + 1, -0.5);
        mem.write(right.cast::<f32>(), -2.0); mem.write(right.cast::<f32>() + 1, 0.5);
        let list = mem.alloc(28); mem.write(list.cast::<u32>(), 2);
        mem.write((list.cast::<u8>() + 4).cast(), AudioBuffer { number_channels: 1, data_byte_size: 8, data: left });
        mem.write((list.cast::<u8>() + 16).cast(), AudioBuffer { number_channels: 1, data_byte_size: 8, data: right });
        let format = AudioStreamBasicDescription {
            sample_rate: 44100.0, format_id: kAudioFormatLinearPCM,
            format_flags: kAudioFormatFlagIsFloat | kAudioFormatFlagIsNonInterleaved,
            bytes_per_packet: 4, frames_per_packet: 1, bytes_per_frame: 4,
            channels_per_frame: 2, bits_per_channel: 32, _reserved: 0,
        };
        let pcm = float_output_pcm(&mem, &format, list, 2).unwrap();
        let samples: Vec<i16> = pcm.chunks_exact(2).map(|b| i16::from_le_bytes(b.try_into().unwrap())).collect();
        assert_eq!(samples, vec![32767, -32767, -16383, 16383]);
        assert!(float_output_pcm(&mem, &format, list, 3).is_none());
    }
}
