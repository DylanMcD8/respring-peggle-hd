/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `AudioComponent.h` (Audio Component Services)

use std::collections::HashMap;
use std::time::Instant;

use crate::abi::GuestFunction;
use crate::audio::openal::al_types::ALuint;
use crate::dyld::FunctionExports;
use crate::environment::Environment;
use crate::export_c_func;
use crate::frameworks::carbon_core::{paramErr, OSStatus};
use crate::frameworks::core_audio_types::{
    fourcc, kAudioFormatFlagIsAlignedHigh, kAudioFormatFlagIsFloat, kAudioFormatFlagIsPacked,
    kAudioFormatFlagIsSignedInteger, kAudioFormatLinearPCM, AudioStreamBasicDescription,
};
use crate::mem::{ConstPtr, ConstVoidPtr, MutPtr, SafeRead};

const kAudioUnitType_Output: u32 = fourcc(b"auou");
const kAudioUnitSubType_RemoteIO: u32 = fourcc(b"rioc");
const kAudioUnitManufacturer_Apple: u32 = fourcc(b"appl");
const kAudioUnitType_Mixer: u32 = fourcc(b"aumx");
const kAudioUnitSubType_MultiChannelMixer: u32 = fourcc(b"mcmx");
const kAudioUnitSubType_AU3DMixerEmbedded: u32 = fourcc(b"3dem");

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AudioUnitKind { RemoteIO, MultiChannelMixer, Embedded3DMixer }

#[derive(Clone)]
pub struct AudioUnitInputBus {
    pub format: Option<AudioStreamBasicDescription>,
    pub callback: Option<AURenderCallbackStruct>,
    pub enabled: bool,
    pub volume: f32,
    pub pan: f32,
    pub parameters: HashMap<u32, f32>,
    pub rate_state: crate::audio_rate::RateState,
}
impl Default for AudioUnitInputBus {
    fn default() -> Self { Self { format: None, callback: None, enabled: true, volume: 1.0, pan: 0.0, parameters: HashMap::new(), rate_state: Default::default() } }
}

#[derive(Copy, Clone)]
#[repr(C, packed)]
pub struct AudioUnitConnection {
    pub source_unit: AudioComponentInstance,
    pub source_output: u32,
    pub destination_input: u32,
}
unsafe impl SafeRead for AudioUnitConnection {}

#[derive(Default)]
pub struct State {
    pub audio_component: AudioComponent,
    pub mixer_component: AudioComponent,
    pub embedded_mixer_component: AudioComponent,
    pub audio_component_instances:
        HashMap<AudioComponentInstance, AudioComponentInstanceHostObject>,
}
impl State {
    pub fn get(framework_state: &mut crate::frameworks::State) -> &mut Self {
        &mut framework_state.audio_toolbox.audio_components
    }
}

#[derive(Clone)]
pub struct AudioComponentInstanceHostObject {
    pub kind: AudioUnitKind,
    pub input_buses: Vec<AudioUnitInputBus>,
    pub connection: Option<AudioUnitConnection>,
    pub output_volume: f32,
    pub started: bool,
    pub maximum_frames_per_slice: u32,
    pub global_stream_format: AudioStreamBasicDescription,
    pub input_stream_format: Option<AudioStreamBasicDescription>,
    pub output_stream_format: Option<AudioStreamBasicDescription>,
    pub render_callback: Option<AURenderCallbackStruct>,
    pub render_notifications: Vec<AURenderCallbackStruct>,
    pub rendered_sample_time: f64,
    pub last_render_time: Option<Instant>,
    pub al_source: Option<ALuint>,
    pub is_running_handler: bool,
}
impl Default for AudioComponentInstanceHostObject {
    fn default() -> Self {
        // Default values obtained from an iPod Touch 4 running iOS 6.1.6
        // through a test app built targetting iOS 2.0
        AudioComponentInstanceHostObject {
            kind: AudioUnitKind::RemoteIO,
            input_buses: vec![AudioUnitInputBus::default()],
            connection: None,
            output_volume: 1.0,
            started: false,
            // returning 1024 based on https://developer.apple.com/documentation/audiotoolbox/kaudiounitproperty_maximumframesperslice
            maximum_frames_per_slice: 1024,
            global_stream_format: AudioStreamBasicDescription {
                sample_rate: 44100.0,
                format_id: kAudioFormatLinearPCM,
                format_flags: kAudioFormatFlagIsFloat
                    | kAudioFormatFlagIsSignedInteger
                    | kAudioFormatFlagIsPacked
                    | kAudioFormatFlagIsAlignedHigh,
                bytes_per_packet: 4,
                frames_per_packet: 1,
                bytes_per_frame: 4,
                channels_per_frame: 2,
                bits_per_channel: 32,
                _reserved: 0,
            },
            input_stream_format: None,
            output_stream_format: None,
            render_callback: None,
            render_notifications: Vec::new(),
            rendered_sample_time: 0.0,
            last_render_time: None,
            al_source: None,
            is_running_handler: false,
        }
    }
}

#[derive(Copy, Clone, Debug)]
#[repr(C, packed)]
#[allow(dead_code)]
pub struct AURenderCallbackStruct {
    pub input_proc: AURenderCallback,
    pub input_proc_ref_con: ConstVoidPtr,
}
unsafe impl SafeRead for AURenderCallbackStruct {}

#[repr(C, packed)]
pub struct OpaqueAudioComponent {
    _pad: u8,
}
unsafe impl SafeRead for OpaqueAudioComponent {}

type AudioComponent = MutPtr<OpaqueAudioComponent>;

pub type AURenderCallback = GuestFunction;

#[repr(C, packed)]
pub struct OpaqueAudioComponentInstance {
    _pad: u8,
}
unsafe impl SafeRead for OpaqueAudioComponentInstance {}

pub type AudioComponentInstance = MutPtr<OpaqueAudioComponentInstance>;

#[repr(C, packed)]
struct AudioComponentDescription {
    component_type: u32,
    component_sub_type: u32,
    component_manufacturer: u32,
    component_flags: u32,
    component_flags_mask: u32,
}
unsafe impl SafeRead for AudioComponentDescription {}

fn AudioComponentFindNext(
    env: &mut Environment,
    in_component: AudioComponent,
    in_desc: ConstPtr<AudioComponentDescription>,
) -> AudioComponent {
    if !in_component.is_null() { return MutPtr::null(); }

    let audio_comp_descr = env.mem.read(in_desc);
    if audio_comp_descr.component_manufacturer != kAudioUnitManufacturer_Apple { return MutPtr::null(); }
    let kind = match (audio_comp_descr.component_type, audio_comp_descr.component_sub_type) {
        (kAudioUnitType_Output, kAudioUnitSubType_RemoteIO) => AudioUnitKind::RemoteIO,
        (kAudioUnitType_Mixer, kAudioUnitSubType_MultiChannelMixer) => AudioUnitKind::MultiChannelMixer,
        (kAudioUnitType_Mixer, kAudioUnitSubType_AU3DMixerEmbedded) => AudioUnitKind::Embedded3DMixer,
        _ => return MutPtr::null(),
    };

    let state = State::get(&mut env.framework_state);
    let component = match kind { AudioUnitKind::RemoteIO => &mut state.audio_component,
        AudioUnitKind::MultiChannelMixer => &mut state.mixer_component,
        AudioUnitKind::Embedded3DMixer => &mut state.embedded_mixer_component };
    if component.is_null() {
        *component = env.mem.alloc_and_write(OpaqueAudioComponent { _pad: 0 });
    }

    let out_component: AudioComponent = *component;

    log!(
        "TODO: AudioComponentFindNext({:?}, {:?}) -> {:?}",
        in_component,
        in_desc,
        out_component
    );
    out_component
}

fn AudioComponentInstanceNew(
    env: &mut Environment,
    in_component: AudioComponent,
    out_instance: MutPtr<AudioComponentInstance>,
) -> OSStatus {
    let state = State::get(&mut env.framework_state);
    if in_component.is_null() || (in_component != state.audio_component && in_component != state.mixer_component && in_component != state.embedded_mixer_component) { return paramErr; }
    let mut host_object = AudioComponentInstanceHostObject::default();
    if in_component == state.mixer_component || in_component == state.embedded_mixer_component {
        host_object.kind = if in_component == state.embedded_mixer_component { AudioUnitKind::Embedded3DMixer } else { AudioUnitKind::MultiChannelMixer };
        host_object.global_stream_format.format_flags = kAudioFormatFlagIsFloat | kAudioFormatFlagIsPacked;
        host_object.global_stream_format.bytes_per_packet = 8;
        host_object.global_stream_format.bytes_per_frame = 8;
        host_object.output_stream_format = Some(host_object.global_stream_format);
    }

    let guest_instance: AudioComponentInstance = env
        .mem
        .alloc_and_write(OpaqueAudioComponentInstance { _pad: 0 });
    State::get(&mut env.framework_state)
        .audio_component_instances
        .insert(guest_instance, host_object);

    env.mem.write(out_instance, guest_instance);

    let result = 0; // success
    log_dbg!(
        "AudioComponentInstanceNew({:?}, {:?}) -> {:?}",
        in_component,
        out_instance,
        result
    );
    result
}

fn AudioComponentInstanceDispose(
    env: &mut Environment,
    in_instance: AudioComponentInstance,
) -> OSStatus {
    let result = if in_instance.is_null() {
        paramErr
    } else {
        State::get(&mut env.framework_state)
            .audio_component_instances
            .remove(&in_instance);
        env.mem.free(in_instance.cast());
        0
    };
    log_dbg!(
        "AudioComponentInstanceDispose({:?}) -> {:?}",
        in_instance,
        result
    );
    result
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(AudioComponentFindNext(_, _)),
    export_c_func!(AudioComponentInstanceNew(_, _)),
    export_c_func!(AudioComponentInstanceDispose(_)),
];
