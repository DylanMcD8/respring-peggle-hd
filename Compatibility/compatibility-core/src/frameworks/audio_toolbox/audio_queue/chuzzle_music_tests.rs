/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
use super::*;
use crate::bundle::Bundle;
use crate::fs::BundleData;
use crate::frameworks::audio_toolbox::audio_file::{
    guest_audio_file_read_from_vec, AudioFileGetProperty,
    kAudioFilePropertyDataFormat,
};

// This executes the original ARM SongInfo::FeedAudioQueue, including its
// end-of-file branch, against the real CAF decoder and Audio Toolbox adapter.
// The proprietary IPA stays outside the source tree and is never a test fixture.
#[test]
#[ignore = "Requires CHUZZLE_ORIGINAL_IPA pointing to the preserved Chuzzle 1.0 IPA"]
fn original_classic_music_callback_crosses_two_loop_boundaries() {
    let path = std::path::PathBuf::from(std::env::var_os("CHUZZLE_ORIGINAL_IPA").unwrap());
    let data = BundleData::open_ipa(&path).unwrap();
    let (bundle, fs) = Bundle::new_bundle_and_fs_from_host_path(data, true).unwrap();
    assert_eq!(bundle.bundle_version(), "1.0");
    let music_path = bundle.bundle_path().join("music/classictheme.caf");
    let music = fs.read(&music_path).unwrap();
    let options = crate::options::Options { headless: true, popup_errors: false, ..Default::default() };
    let env = Environment::new(bundle, fs, options, Vec::new()).unwrap();
    env.run_app_picker(move |env| {
        // The game opens OpenAL for effects before Audio Toolbox starts music.
        // Exercise that combination, including iOS's single physical device.
        let effects_device = unsafe { al::alcOpenDevice(std::ptr::null()) };
        assert!(!effects_device.is_null(), "could not open effects device");
        let mut effects_context = unsafe {
            crate::audio::openal::OpenALContext::new_with_device_and_attrlist(
                &mut env.openal_manager, effects_device, std::ptr::null(),
            ).unwrap()
        };
        effects_context.make_current(&mut env.openal_manager);
        env.cpu.regs_mut()[crate::cpu::Cpu::SP] = 0xffff_fff0;
        let file = guest_audio_file_read_from_vec(env, music).unwrap();
        let desc_ptr = env.mem.alloc(40).cast::<AudioStreamBasicDescription>();
        let size_ptr = env.mem.alloc_and_write(40_u32);
        assert_eq!(AudioFileGetProperty(env, file, kAudioFilePropertyDataFormat, size_ptr, desc_ptr.cast_void()), 0);
        let count_ptr = env.mem.alloc_and_write(0_u64);
        env.mem.write(size_ptr, 8_u32);
        assert_eq!(AudioFileGetProperty(env, file, fourcc(b"pcnt"), size_ptr, count_ptr.cast_void()), 0);
        let count = env.mem.read(count_ptr);
        let desc = env.mem.read(desc_ptr);
        let duration = count as f64 / desc.sample_rate;
        assert!((47.0..49.0).contains(&duration), "decoded classic music duration: {duration}");
        let song = env.mem.alloc(104).cast::<u8>();
        env.mem.bytes_at_mut(song, 104).fill(0);
        let song_bridge = env.mem.alloc_and_write(song);
        let music_info = env.mem.alloc(52).cast::<u8>();
        env.mem.bytes_at_mut(music_info, 52).fill(0);
        env.mem.write((song + 0x64).cast(), music_info);
        let music_bridge = env.mem.alloc(16).cast::<u8>();
        env.mem.bytes_at_mut(music_bridge, 16).fill(0);
        env.mem.write((music_bridge + 12).cast(), song_bridge);

        // Execute SongBridge::Load rather than supplying the packet count.
        // The original loader skips that count if the cookie query fails,
        // which prevents its EOF equality check from looping the song.
        let name = env.mem.alloc_and_write_cstr(music_path.as_str().as_bytes());
        let load = GuestFunction::from_addr_and_thumb_flag(0x8a0f4, false);
        let loaded: bool = load.call_from_host(env, (song_bridge, name));
        assert!(loaded, "original SongBridge::Load failed");
        let loaded_count: u64 = env.mem.read((song + 0x5c).cast());
        assert_eq!(loaded_count, count, "original loader skipped the PCM packet count");
        assert_eq!(env.mem.read((song + 0x54).cast::<u32>()), 0, "decoded PCM has no priming frames");
        assert_eq!(env.mem.read((song + 0x58).cast::<u32>()), 0, "decoded PCM has no remainder frames");
        let buffer_bytes: u32 = env.mem.read((song + 0x2c).cast());
        let packets_per_buffer: u32 = env.mem.read((song + 0x30).cast());
        assert_eq!(buffer_bytes, 88_200, "original half-second buffer sizing changed");
        assert_eq!(packets_per_buffer, 22_050);

        let out_aq = env.mem.alloc_and_write(AudioQueueRef::null());
        assert_eq!(AudioQueueNewOutput(env, desc_ptr.cast_const(), GuestFunction::from_addr_and_thumb_flag(0x8a6b0, false), music_bridge.cast_void(), Ptr::null(), Ptr::null(), 0, out_aq), 0);
        let aq = env.mem.read(out_aq);
        env.mem.write((music_info + 0x28).cast(), aq);
        let start = GuestFunction::from_addr_and_thumb_flag(0x8a598, false);
        let (): () = start.call_from_host(env, (song, true));
        let buffers: Vec<AudioQueueBufferRef> = (0..3)
            .map(|index| env.mem.read((song + 0x44 + index * 4).cast()))
            .collect();
        assert_eq!(AudioQueueStart(env, aq, Ptr::null()), 0);
        let al_source = State::get(&mut env.framework_state).audio_queues[&aq].al_source.unwrap();
        let pitch = std::env::var("CHUZZLE_AUDIO_TEST_PITCH")
            .ok().and_then(|value| value.parse::<f32>().ok()).unwrap_or(32.0);
        assert!(pitch.is_finite() && (1.0..=32.0).contains(&pitch));
        {
            let context = env.framework_state.audio_toolbox.make_al_context_current(&mut env.openal_manager);
            unsafe {
                context.Sourcef(al_source, 0x1003 /* AL_PITCH */, pitch);
                assert_eq!(context.GetError(), 0);
            }
        }
        let mut previous = 0;
        let mut loops = 0;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(140);
        while std::time::Instant::now() < deadline {
            effects_context.make_current(&mut env.openal_manager);
            handle_audio_queue(env, aq);
            let position: u64 = env.mem.read((song + 0x34).cast());
            for &buffer in &buffers {
                assert!(env.mem.read(buffer).audio_data_byte_size > 0, "music callback produced no data before completing two loops");
            }
            let active: u8 = env.mem.read((song + 0x50).cast());
            assert_eq!(active, 1, "original music callback stopped at EOF");
            if position < previous { loops += 1; }
            previous = position;
            if loops == 2 { break; }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert_eq!(loops, 2, "original callback did not wrap its packet position twice");
        {
            let context = env.framework_state.audio_toolbox.make_al_context_current(&mut env.openal_manager);
            let mut state = 0;
            unsafe { context.GetSourcei(al_source, al::AL_SOURCE_STATE, &mut state); }
            assert_eq!(state, al::AL_PLAYING, "audio source stopped after its second loop");
        }
        assert_eq!(AudioQueueStop(env, aq, true), 0);
        assert_eq!(AudioQueueDispose(env, aq, true), 0);
        drop(effects_context);
        assert_eq!(unsafe { al::alcCloseDevice(effects_device) }, al::ALC_TRUE);
        eprintln!("Original classic music: decoded {duration:.3}s, streamed two complete loops through OpenAL at {pitch}x pitch, source still playing");
    });
}
