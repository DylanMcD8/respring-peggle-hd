use std::io::Write;
use std::ffi::{c_char, CStr};
// Full Device retained for developer experiments, disabled in this original-raster comparison.
const FULL_DEVICE_TEST:bool=false;
mod nights;
mod runtime_settings;
mod save_manager;
mod save_manager_bridge;

// The original ARMv7 Peggle HD 2.0.9 game remains an unchanged data file. All guest instructions are
// interpreted except for guarded native helpers; guest networking is disabled.
#[no_mangle]
pub unsafe extern "C" fn peggle_ios_run(
    ipa: *const c_char,
    resources: *const c_char,
    documents: *const c_char,
) -> i32 {
    if ipa.is_null() || resources.is_null() || documents.is_null() {
        return 1;
    }
    let outcome = std::panic::catch_unwind(|| {
        let ipa = CStr::from_ptr(ipa).to_str().map_err(|e| e.to_string())?;
        let resources = CStr::from_ptr(resources).to_str().map_err(|e| e.to_string())?;
        let documents = CStr::from_ptr(documents).to_str().map_err(|e| e.to_string())?;
        std::fs::create_dir_all(documents).map_err(|e| e.to_string())?;
        touchHLE::diagnostics::initialize(
            &std::path::Path::new(documents).join("Peggle-C53-diagnostics.jsonl"),
            std::env::var("PEGGLE_C53_DIAGNOSTICS").as_deref() != Ok("0"),
        ).map_err(|e| format!("Diagnostic initialization: {e}"))?;
        // Resolve before creating any guest GL context or installing hooks.
        // Normal icon launches use the persisted C53 profile; explicit0/1
        // overrides update that host-only profile for later launches.
        // Preserve C51 single-buffer transfers.
        std::env::set_var("PEGGLE_HD_COPY_RING","0");
        let runtime = runtime_settings::initialize(std::path::Path::new(documents));
        touchHLE::diagnostics::event(&runtime.event_json());
        let original_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            touchHLE::diagnostics::panic_message(&info.to_string());
            original_hook(info);
        }));
        // Profile discovery and creation belong to the original game. Never
        // synthesize a User index/current-user preference in the host startup.
        std::env::set_var("TOUCHHLE_RESOURCE_DIR", resources);
        let game_documents=save_manager_bridge::approved_game_root(std::path::Path::new(documents))?;
        // Apply to the selected save slot before NSUserDefaults is loaded by the guest.
        extern "C" { fn peggle_show_nights_theme() -> i32; }
        std::env::set_var("PEGGLE_SHOW_NIGHTS_THEME", if peggle_show_nights_theme() != 0 { "1" } else { "0" });
        let nights_changed = nights::initialize(&game_documents, "com.popcap.ios.PeggleHD")?;
        touchHLE::diagnostics::event(&format!("{{\"event\":\"nights_entitlement\",\"enabled\":true,\"preferences_changed\":{nights_changed}}}"));
        std::env::set_var("TOUCHHLE_USER_DATA_DIR", &game_documents);
        std::env::set_current_dir(&game_documents).map_err(|e| e.to_string())?;
        // Original guest raster with high-DPI final output; no resolution selector.
        let wrapper_mode=if FULL_DEVICE_TEST{"device"}else{"original"};
        touchHLE::diagnostics::event(&format!("{{\"event\":\"wrapper_resolution_preference\",\"mode\":\"{wrapper_mode}\",\"source\":\"fixed_C53_test_configuration\",\"fixed\":true,\"invalid_preference\":false,\"latched_until_restart\":true}}"));
        let arguments = vec![
            "Peggle Compatibility".to_owned(),
            ipa.to_owned(),
            "--fullscreen".to_owned(),
            format!("--wrapper-resolution={wrapper_mode}"),
            "--device-family=ipad".to_owned(),
            "--landscape-right".to_owned(),
            "--scale-hack=1".to_owned(),
            "--screen-scale=2".to_owned(),
            "--compositor-scale=1".to_owned(),
            "--display-link-fps=60".to_owned(),
            "--fps-limit=60".to_owned(),
            "--pause-on-background".to_owned(),
            "--fullscreen-host-blit".to_owned(),
            "--no-error-popup".to_owned(),
        ];
        // Bounded C53 diagnostics replace verbose timer, frame-image, and PCM
        // capture diagnostics. They work on manual launches without env flags.
        touchHLE::main(arguments.into_iter())
    });
    match outcome {
        Ok(Ok(())) => {if touchHLE::progress_runtime::take_reload(){match save_manager_bridge::finish_game_for_reload(){Ok(())=>75,Err(_)=>1}}else{0}},
        Ok(Err(error)) => { let _ = writeln!(std::io::stderr().lock(), "Peggle Compatibility: {error}"); 1 }
        Err(_) => { let _ = writeln!(std::io::stderr().lock(), "Peggle Compatibility: emulator panic; see Documents/Peggle-C53-diagnostics.jsonl"); 2 }
    }
}

// Native overlay telemetry contains timing/geometry only, never screenshots or
// user-entered text. Called on visibility changes and at most once per5seconds.
#[no_mangle]
pub extern "C" fn peggle_ios_fps_diagnostic(kind:i32,fps:f64,count:u64,warming:i32,overflow:i32,x:f64,y:f64,width:f64,height:f64,interactive:i32,contained:i32){
    if kind==0 {
        touchHLE::diagnostics::event(&format!("{{\"event\":\"present_fps_overlay_visibility\",\"enabled\":{}}}",count!=0));
    }else if kind==1 {
        touchHLE::diagnostics::event(&format!("{{\"event\":\"present_fps_overlay_sample\",\"metric\":\"successful_tagged_EAGL_game_buffer_submissions\",\"rolling_window_s\":0.5,\"fps\":{fps:.2},\"presentations\":{count},\"warming\":{},\"overflow_lower_bound\":{},\"frame\":{{\"x\":{x:.1},\"y\":{y:.1},\"width\":{width:.1},\"height\":{height:.1}}},\"interactive\":{},\"inside_safe_area\":{}}}",warming!=0,overflow!=0,interactive!=0,contained!=0));
    }
}

// Counts/stages only; no native NSString contents cross this FFI.
#[no_mangle]
pub extern "C" fn peggle_ios_keyboard_native_diagnostic(stage:i32,current_units:u32,replacement_units:u32){touchHLE::keyboard_trace::native(stage,current_units,replacement_units);}
