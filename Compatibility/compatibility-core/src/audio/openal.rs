/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Wrapper for OpenAL usage.
use al_sys::alc_types::{ALCcontext, ALCdevice};
use std::marker::PhantomData;
use touchHLE_openal_soft_wrapper as al_sys;
use touchHLE_openal_soft_wrapper::alc_types::ALCint;

pub use al_sys::al_defines::*;
pub use al_sys::al_types;
pub use al_sys::alc_defines::*;
pub use al_sys::alc_types;
pub use al_sys::{alcGetError, alcGetString};
#[cfg(not(target_os = "ios"))]
pub use al_sys::{alcCloseDevice, alcOpenDevice};

// Opt-in mute for automated simulator runs; device launches retain their audio.
const TEST_LISTENER_GAIN: i32 = 0x100a; // OpenAL AL_GAIN, omitted by the minimal bindings.
fn simulator_test_muted() -> bool {
    static MUTED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *MUTED.get_or_init(|| std::env::var_os("SIMULATOR_UDID").is_some()
        && std::env::var("PEGGLE_TEST_MUTE").as_deref() == Ok("1"))
}

// Apple's iOS OpenAL backend only opens the default output device once.
// It also rejects activating a second context. Chuzzle's effects and Audio
// Toolbox music therefore share the native mixer, with distinct source IDs
// and reference-counted logical ownership.
#[cfg(target_os = "ios")]
static IOS_OUTPUT_DEVICE: std::sync::Mutex<(usize, usize)> = std::sync::Mutex::new((0, 0));
#[cfg(target_os = "ios")]
static IOS_OUTPUT_CONTEXT: std::sync::Mutex<(usize, usize)> = std::sync::Mutex::new((0, 0));

#[cfg(target_os = "ios")]
static IOS_SOURCES: std::sync::Mutex<Option<std::collections::HashSet<ALuint>>> = std::sync::Mutex::new(None);

pub struct NativeAudioPause {
    context: usize,
    pub paused_sources: Vec<ALuint>,
}

pub fn pause_native_audio() -> NativeAudioPause {
    let mut token = NativeAudioPause { context: 0, paused_sources: Vec::new() };
    #[cfg(target_os = "ios")]
    unsafe {
        token.context = IOS_OUTPUT_CONTEXT.lock().unwrap().0;
        if token.context == 0 { return token; }
        let previous = al_sys::alcGetCurrentContext();
        if al_sys::alcMakeContextCurrent(token.context as *mut ALCcontext) != ALC_TRUE { return token; }
        let sources = IOS_SOURCES.lock().unwrap().clone().unwrap_or_default();
        for source in sources {
            if al_sys::alIsSource(source) == 0 { continue; }
            let mut state = 0;
            al_sys::alGetSourcei(source, AL_SOURCE_STATE, &mut state);
            if state == AL_PLAYING {
                al_sys::alSourcePause(source);
                al_sys::alGetSourcei(source, AL_SOURCE_STATE, &mut state);
                if state == AL_PAUSED { token.paused_sources.push(source); }
            }
        }
        al_sys::alcMakeContextCurrent(previous);
    }
    token
}

pub fn resume_native_audio(token: NativeAudioPause) -> usize {
    let mut resumed = 0;
    #[cfg(target_os = "ios")]
    unsafe {
        if token.context == 0 { return 0; }
        let previous = al_sys::alcGetCurrentContext();
        if al_sys::alcMakeContextCurrent(token.context as *mut ALCcontext) != ALC_TRUE { return 0; }
        for source in token.paused_sources {
            let exists = al_sys::alIsSource(source) != 0;
            let mut state = 0;
            if exists { al_sys::alGetSourcei(source, AL_SOURCE_STATE, &mut state); }
            if crate::lifecycle_pause::audio_resume_allowed(true, exists, state == AL_PAUSED) {
                al_sys::alSourcePlay(source);
                resumed += 1;
            }
        }
        al_sys::alcMakeContextCurrent(previous);
    }
    resumed
}

#[cfg(target_os = "ios")]
pub unsafe fn alcOpenDevice(name: *const alc_types::ALCchar) -> *mut ALCdevice {
    let mut output = IOS_OUTPUT_DEVICE.lock().unwrap();
    if output.0 != 0 {
        output.1 += 1;
        return output.0 as *mut ALCdevice;
    }
    let device = al_sys::alcOpenDevice(name);
    if !device.is_null() {
        *output = (device as usize, 1);
    }
    device
}

#[cfg(target_os = "ios")]
pub unsafe fn alcCloseDevice(device: *mut ALCdevice) -> alc_types::ALCboolean {
    let mut output = IOS_OUTPUT_DEVICE.lock().unwrap();
    if device.is_null() || device as usize != output.0 {
        return ALC_FALSE;
    }
    if output.1 > 1 {
        output.1 -= 1;
        return ALC_TRUE;
    }
    let result = al_sys::alcCloseDevice(device);
    if result == ALC_TRUE {
        *output = (0, 0);
    }
    result
}

use al_types::*;

static OPENALMANAGER_INSTANCE_EXISTS: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
pub struct OpenALManager {}
impl OpenALManager {
    pub fn new() -> Result<Self, String> {
        if OPENALMANAGER_INSTANCE_EXISTS.swap(true, std::sync::atomic::Ordering::SeqCst) {
            return Err("Only one OpenALManager can exist at a time!".to_string());
        }
        Ok(Self {})
    }
}

impl Drop for OpenALManager {
    fn drop(&mut self) {
        OPENALMANAGER_INSTANCE_EXISTS.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

#[derive(Debug)]
pub struct OpenALContext {
    context: *mut ALCcontext,
    // We don't need to guarantee the lifetime or thread-safety of the device -
    // OpenAL (should) do that for us.
    device: *mut ALCdevice,
    owns_device: bool,
}

impl OpenALContext {
    pub fn new(_manager: &mut OpenALManager) -> Result<Self, String> {
        let device = unsafe { alcOpenDevice(std::ptr::null()) };
        if device.is_null() {
            return Err("Could not open OpenAL device".to_string());
        }
        match unsafe { Self::new_with_device_and_attrlist(_manager, device, std::ptr::null()) } {
            Ok(mut context) => {
                context.owns_device = true;
                Ok(context)
            }
            Err(error) => {
                unsafe { alcCloseDevice(device) };
                Err(error)
            }
        }
    }

    pub unsafe fn new_with_device_and_attrlist(
        _manager: &mut OpenALManager,
        device: *mut ALCdevice,
        attrlist: *const ALCint,
    ) -> Result<Self, String> {
        #[cfg(not(target_os = "ios"))]
        let context = al_sys::alcCreateContext(device, attrlist);
        #[cfg(target_os = "ios")]
        let context = {
            let mut shared = IOS_OUTPUT_CONTEXT.lock().unwrap();
            if shared.0 != 0 {
                shared.1 += 1;
                shared.0 as *mut ALCcontext
            } else {
                let context = al_sys::alcCreateContext(device, attrlist);
                if !context.is_null() {
                    *shared = (context as usize, 1);
                }
                context
            }
        };
        if context.is_null() {
            return Err("Could not open OpenAL context".to_string());
        }
        log_dbg!(
            "New OpenAL device ({:?}) and context ({:?})",
            device,
            context
        );
        Ok(Self { context, device, owns_device: false })
    }

    pub fn make_current<'al_ctx, 'manager: 'al_ctx>(
        &'al_ctx mut self,
        _manager: &'manager mut OpenALManager,
    ) -> OpenAL<'al_ctx> {
        // OpenALsoft already caches the current context, no need to
        // check ourselves.
        let ret = unsafe { al_sys::alcMakeContextCurrent(self.context) };
        assert!(ret == ALC_TRUE);
        if simulator_test_muted() { unsafe { al_sys::alListenerf(TEST_LISTENER_GAIN, 0.0); } }
        OpenAL {
            _al_lifetime: PhantomData,
        }
    }

    pub fn SuspendContext(&mut self) {
        unsafe { al_sys::alcSuspendContext(self.context) };
    }

    pub fn ProcessContext(&mut self) {
        unsafe { al_sys::alcProcessContext(self.context) };
    }

    pub fn GetContextsDevice(&self) -> *mut ALCdevice {
        self.device
    }

    // According to both OpenAL docs and OpenALsoft source code, this is
    // context independent, and can be called without one active.
    pub unsafe fn GetEnumValue(enumName: *const ALchar) -> ALenum {
        al_sys::alGetEnumValue(enumName)
    }
}

impl Drop for OpenALContext {
    fn drop(&mut self) {
        unsafe {
            #[cfg(not(target_os = "ios"))]
            let destroy = true;
            #[cfg(target_os = "ios")]
            let destroy = {
                let mut shared = IOS_OUTPUT_CONTEXT.lock().unwrap();
                shared.1 -= 1;
                if shared.1 == 0 {
                    *shared = (0, 0);
                    true
                } else {
                    false
                }
            };
            if destroy {
                if al_sys::alcGetCurrentContext() == self.context {
                    al_sys::alcMakeContextCurrent(std::ptr::null_mut());
                }
                al_sys::alcDestroyContext(self.context);
            }
            if self.owns_device {
                alcCloseDevice(self.device);
            }
        }
    }
}

pub struct OpenAL<'al_ctx> {
    _al_lifetime: PhantomData<&'al_ctx ()>,
}

impl OpenAL<'_> {
    pub unsafe fn GetError(&self) -> ALenum {
        al_sys::alGetError()
    }

    pub unsafe fn DistanceModel(&self, value: ALenum) {
        al_sys::alDistanceModel(value)
    }

    pub unsafe fn IsBuffer(&self, buffer: ALuint) -> ALboolean {
        al_sys::alIsBuffer(buffer)
    }
    pub unsafe fn IsSource(&self, source: ALuint) -> ALboolean {
        al_sys::alIsSource(source)
    }
    pub unsafe fn IsExtensionPresent(&self, extName: *const ALchar) -> ALboolean {
        al_sys::alIsExtensionPresent(extName)
    }

    pub unsafe fn Enable(&self, capability: ALenum) {
        al_sys::alEnable(capability)
    }

    pub unsafe fn GetBufferi(&self, buffer: ALuint, param: ALenum, value: *const ALint) {
        al_sys::alGetBufferi(buffer, param, value)
    }

    pub unsafe fn Listenerf(&self, param: ALenum, value: ALfloat) {
        al_sys::alListenerf(param, if param==TEST_LISTENER_GAIN && simulator_test_muted() {0.0}else{value})
    }
    pub unsafe fn Listener3f(
        &self,
        param: ALenum,
        value1: ALfloat,
        value2: ALfloat,
        value3: ALfloat,
    ) {
        al_sys::alListener3f(param, value1, value2, value3)
    }
    pub unsafe fn Listenerfv(&self, param: ALenum, values: *const ALfloat) {
        if param==TEST_LISTENER_GAIN && simulator_test_muted() {al_sys::alListenerf(param,0.0)}else{al_sys::alListenerfv(param, values)}
    }
    pub unsafe fn Listeneri(&self, param: ALenum, value: ALint) {
        al_sys::alListeneri(param, value)
    }
    pub unsafe fn Listener3i(&self, param: ALenum, value1: ALint, value2: ALint, value3: ALint) {
        al_sys::alListener3i(param, value1, value2, value3)
    }
    pub unsafe fn Listeneriv(&self, param: ALenum, values: *const ALint) {
        al_sys::alListeneriv(param, values)
    }

    pub unsafe fn GetListenerf(&self, param: ALenum, value: *mut ALfloat) {
        al_sys::alGetListenerf(param, value)
    }
    pub unsafe fn GetListener3f(
        &self,
        param: ALenum,
        value1: *mut ALfloat,
        value2: *mut ALfloat,
        value3: *mut ALfloat,
    ) {
        al_sys::alGetListener3f(param, value1, value2, value3)
    }
    pub unsafe fn GetListenerfv(&self, param: ALenum, values: *mut ALfloat) {
        al_sys::alGetListenerfv(param, values)
    }
    pub unsafe fn GetListeneri(&self, param: ALenum, value: *mut ALint) {
        al_sys::alGetListeneri(param, value)
    }
    pub unsafe fn GetListener3i(
        &self,
        param: ALenum,
        value1: *mut ALint,
        value2: *mut ALint,
        value3: *mut ALint,
    ) {
        al_sys::alGetListener3i(param, value1, value2, value3)
    }
    pub unsafe fn GetListeneriv(&self, param: ALenum, values: *mut ALint) {
        al_sys::alGetListeneriv(param, values)
    }

    pub unsafe fn GenSources(&self, n: ALsizei, sources: *mut ALuint) {
        al_sys::alGenSources(n, sources);
        #[cfg(target_os = "ios")]
        if n > 0 && !sources.is_null() {
            let mut tracked = IOS_SOURCES.lock().unwrap();
            let tracked = tracked.get_or_insert_with(std::collections::HashSet::new);
            for &source in std::slice::from_raw_parts(sources, n as usize) {
                if al_sys::alIsSource(source) != 0 { tracked.insert(source); }
            }
        }
    }
    pub unsafe fn DeleteSources(&self, n: ALsizei, sources: *const ALuint) {
        al_sys::alDeleteSources(n, sources);
        #[cfg(target_os = "ios")]
        if n > 0 && !sources.is_null() {
            if let Some(tracked) = IOS_SOURCES.lock().unwrap().as_mut() {
                for &source in std::slice::from_raw_parts(sources, n as usize) {
                    if al_sys::alIsSource(source) == 0 { tracked.remove(&source); }
                }
            }
        }
    }

    pub unsafe fn Sourcef(&self, source: ALuint, param: ALenum, value: ALfloat) {
        al_sys::alSourcef(source, param, value)
    }
    pub unsafe fn Source3f(
        &self,
        source: ALuint,
        param: ALenum,
        value1: ALfloat,
        value2: ALfloat,
        value3: ALfloat,
    ) {
        al_sys::alSource3f(source, param, value1, value2, value3)
    }
    pub unsafe fn Sourcefv(&self, source: ALuint, param: ALenum, values: *const ALfloat) {
        al_sys::alSourcefv(source, param, values)
    }
    pub unsafe fn Sourcei(&self, source: ALuint, param: ALenum, value: ALint) {
        al_sys::alSourcei(source, param, value)
    }
    pub unsafe fn Source3i(
        &self,
        source: ALuint,
        param: ALenum,
        value1: ALint,
        value2: ALint,
        value3: ALint,
    ) {
        al_sys::alSource3i(source, param, value1, value2, value3)
    }
    pub unsafe fn Sourceiv(&self, source: ALuint, param: ALenum, values: *const ALint) {
        al_sys::alSourceiv(source, param, values)
    }

    pub unsafe fn GetSourcef(&self, source: ALuint, param: ALenum, value: *mut ALfloat) {
        al_sys::alGetSourcef(source, param, value)
    }
    pub unsafe fn GetSource3f(
        &self,
        source: ALuint,
        param: ALenum,
        value1: *mut ALfloat,
        value2: *mut ALfloat,
        value3: *mut ALfloat,
    ) {
        al_sys::alGetSource3f(source, param, value1, value2, value3)
    }
    pub unsafe fn GetSourcefv(&self, source: ALuint, param: ALenum, values: *mut ALfloat) {
        al_sys::alGetSourcefv(source, param, values)
    }
    pub unsafe fn GetSourcei(&self, source: ALuint, param: ALenum, value: *mut ALint) {
        al_sys::alGetSourcei(source, param, value)
    }
    pub unsafe fn GetSource3i(
        &self,
        source: ALuint,
        param: ALenum,
        value1: *mut ALint,
        value2: *mut ALint,
        value3: *mut ALint,
    ) {
        al_sys::alGetSource3i(source, param, value1, value2, value3)
    }
    pub unsafe fn GetSourceiv(&self, source: ALuint, param: ALenum, values: *mut ALint) {
        al_sys::alGetSourceiv(source, param, values)
    }

    pub unsafe fn SourcePlay(&self, source: ALuint) {
        al_sys::alSourcePlay(source)
    }
    pub unsafe fn SourcePause(&self, source: ALuint) {
        al_sys::alSourcePause(source)
    }
    pub unsafe fn SourceStop(&self, source: ALuint) {
        al_sys::alSourceStop(source)
    }
    pub unsafe fn SourceRewind(&self, source: ALuint) {
        al_sys::alSourceRewind(source)
    }

    pub unsafe fn SourcePlayv(&self, n: ALsizei, sources: *const ALuint) {
        al_sys::alSourcePlayv(n, sources)
    }
    pub unsafe fn SourcePausev(&self, n: ALsizei, sources: *const ALuint) {
        al_sys::alSourcePausev(n, sources)
    }
    pub unsafe fn SourceStopv(&self, n: ALsizei, sources: *const ALuint) {
        al_sys::alSourceStopv(n, sources)
    }
    pub unsafe fn SourceRewindv(&self, n: ALsizei, sources: *const ALuint) {
        al_sys::alSourceRewindv(n, sources)
    }

    pub unsafe fn SourceQueueBuffers(&self, source: ALuint, nb: ALsizei, buffers: *const ALuint) {
        al_sys::alSourceQueueBuffers(source, nb, buffers)
    }
    pub unsafe fn SourceUnqueueBuffers(&self, source: ALuint, nb: ALsizei, buffers: *mut ALuint) {
        al_sys::alSourceUnqueueBuffers(source, nb, buffers)
    }

    pub unsafe fn GenBuffers(&self, n: ALsizei, buffers: *mut ALuint) {
        al_sys::alGenBuffers(n, buffers)
    }
    pub unsafe fn DeleteBuffers(&self, n: ALsizei, buffers: *const ALuint) {
        al_sys::alDeleteBuffers(n, buffers)
    }

    pub unsafe fn BufferData(
        &self,
        buffer: ALuint,
        format: ALenum,
        data: *const ALvoid,
        size: ALsizei,
        samplerate: ALsizei,
    ) {
        al_sys::alBufferData(buffer, format, data, size, samplerate)
    }

    pub unsafe fn DopplerFactor(&self, dopplerFactor: ALfloat) {
        al_sys::alDopplerFactor(dopplerFactor)
    }
    pub unsafe fn DopplerVelocity(&self, dopplerVelocity: ALfloat) {
        al_sys::alDopplerVelocity(dopplerVelocity)
    }
    pub unsafe fn SpeedOfSound(&self, speed: ALfloat) {
        al_sys::alSpeedOfSound(speed)
    }
}
