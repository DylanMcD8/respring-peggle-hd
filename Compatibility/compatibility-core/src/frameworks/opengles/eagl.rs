/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! EAGL.

use crate::dyld::{ConstantExports, HostConstant};
use crate::frameworks::core_animation::ca_eagl_layer::{
    find_fullscreen_eagl_layer, get_pixels_vec_for_presenting, present_pixels,
};
use crate::frameworks::core_graphics::{CGRect, CGSize};
use crate::frameworks::foundation::ns_string::get_static_str;
use crate::frameworks::foundation::NSUInteger;
use crate::gles::gles11_raw as gles11; // constants only
use crate::gles::gles11_raw::types::*;
use crate::gles::present::{present_frame, FpsCounter};
use crate::gles::{create_gles1_ctx, gles1_on_gl2, GLESContext, GLES};
use crate::mem::MutPtr;
use crate::objc::{id, msg, nil, objc_classes, release, retain, ClassExports, HostObject};
use crate::options::Options;
use crate::Environment;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

pub(crate) fn reset_after_pause(env: &mut crate::Environment) {
    let contexts: std::collections::HashSet<_> = env.framework_state.opengles.current_ctxs
        .values().filter_map(|id| *id).collect();
    for context in contexts {
        let state = env.objc.borrow_mut::<EAGLContextHostObject>(context);
        state.next_frame_due = None;
        state.fps_counter = None;
    }
}

// These are used by the EAGLDrawable protocol implemented by CAEAGLayer.
// Since these have the ABI of constant symbols rather than literal constants,
// the values shouldn't matter, and haven't been checked against real iPhone OS.
pub const kEAGLDrawablePropertyColorFormat: &str = "ColorFormat";
pub const kEAGLDrawablePropertyRetainedBacking: &str = "RetainedBacking";
pub const kEAGLColorFormatRGBA8: &str = "RGBA8";
pub const kEAGLColorFormatRGB565: &str = "RGB565";

pub const CONSTANTS: ConstantExports = &[
    (
        "_kEAGLDrawablePropertyColorFormat",
        HostConstant::NSString(kEAGLDrawablePropertyColorFormat),
    ),
    (
        "_kEAGLDrawablePropertyRetainedBacking",
        HostConstant::NSString(kEAGLDrawablePropertyRetainedBacking),
    ),
    (
        "_kEAGLColorFormatRGBA8",
        HostConstant::NSString(kEAGLColorFormatRGBA8),
    ),
    (
        "_kEAGLColorFormatRGB565",
        HostConstant::NSString(kEAGLColorFormatRGB565),
    ),
];

type EAGLRenderingAPI = u32;
const kEAGLRenderingAPIOpenGLES1: EAGLRenderingAPI = 1;
#[allow(dead_code)]
const kEAGLRenderingAPIOpenGLES2: EAGLRenderingAPI = 2;
#[allow(dead_code)]
const kEAGLRenderingAPIOpenGLES3: EAGLRenderingAPI = 3;

pub(super) struct EAGLContextHostObject {
    pub(super) gles_ctx: Option<Box<dyn GLESContext>>,
    /// Mapping of OpenGL ES renderbuffer names to `EAGLDrawable` instances
    /// (always `CAEAGLLayer*`). Retains the instance so it won't dangle.
    renderbuffer_drawable_bindings: Rc<RefCell<HashMap<GLuint, id>>>,
    pub(super)wrapper_sizes:super::wrapper_raster::Sizes,
    pub(super)wrapper_raster:super::wrapper_raster::Raster,
    fps_counter: Option<FpsCounter>,
    next_frame_due: Option<Instant>,
    pacing_target:Option<u32>,
    pub(super) pending_errors: std::collections::VecDeque<GLenum>,
    pub(super) shares_host: bool,
    gpu_disabled: bool,
    transfers: HashMap<id, crate::gles::gpu_transfer::Transfer>,
    pub mapped_buffers: HashMap<GLuint, (MutPtr<GLvoid>, *mut GLvoid)>,
}
impl HostObject for EAGLContextHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation EAGLContext: NSObject

+ (id)alloc {
    let host_object = Box::new(EAGLContextHostObject {
        gles_ctx: None,
        renderbuffer_drawable_bindings: Rc::new(RefCell::new(HashMap::new())),
        wrapper_sizes:Default::default(),wrapper_raster:Default::default(),
        fps_counter: None,
        next_frame_due: None,
        pacing_target:None,
        mapped_buffers: HashMap::new(),
        pending_errors: Default::default(), shares_host: false, gpu_disabled: false, transfers: HashMap::new(),
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

+ (id)currentContext {
    env.framework_state.opengles.current_ctx_for_thread(env.current_thread).unwrap_or(nil)
}
+ (bool)setCurrentContext:(id)context { // EAGLContext*
    retain(env, context);

    let current_ctx = env.framework_state.opengles.current_ctx_for_thread(env.current_thread);

    if let Some(old_ctx) = std::mem::take(current_ctx) {
        release(env, old_ctx);
    }

    // reborrow
    let current_ctx = env.framework_state.opengles.current_ctx_for_thread(env.current_thread);

    if context != nil {
        *current_ctx = Some(context);
        if env.options.wrapper_resolution.is_some(){super::wrapper_raster::refresh(env);}
    }

    true
}

- (id)initWithAPI:(EAGLRenderingAPI)api sharegroup:(id)group {
    if api != kEAGLRenderingAPIOpenGLES1 {
        log!(
            "TODO: App requested EAGL initWithAPI:{} sharegroup:{:?}, returning nil as we only support API 1 for now",
            api,
            group
        );
        return nil;
    }

    if group == nil {
        return msg![env; this initWithAPI:api];
    }

    let window = env.window.as_mut().expect("OpenGL ES is not supported in headless mode");
    let prev_context = env.objc.borrow_mut::<EAGLContextHostObject>(group).gles_ctx.as_mut().unwrap();

    // This is sort of a hack - we set the "current" context, then immediately
    // drop it. Since we know all the code between here and creating the new
    // context, we know that there won't be any context switches, so it's fine
    // to do this.
    {
        let mut prev_ctx = prev_context.make_current(window);
        if crate::gles::gpu_transfer::enabled() { unsafe { prev_ctx.Flush(); } }
    }
    env.window.as_mut().unwrap().set_share_with_current_context(true);

    let mut gles1_ins = create_gles1_ctx(env);

    let window = env.window.as_mut().expect("OpenGL ES is not supported in headless mode");
    {
        let gles1_ctx = gles1_ins.make_current(window);
        log!("Driver info: {}", unsafe { gles1_ctx.driver_description() });
    }

    env.objc.borrow_mut::<EAGLContextHostObject>(this).gles_ctx = Some(gles1_ins);

    env.window.as_mut().unwrap().set_share_with_current_context(false);

    env.objc.borrow_mut::<EAGLContextHostObject>(this).shares_host = env.objc.borrow::<EAGLContextHostObject>(group).shares_host
        && env.objc.borrow::<EAGLContextHostObject>(this).gles_ctx.as_ref().unwrap().is_native_gles1();
    env.objc.borrow_mut::<EAGLContextHostObject>(this).renderbuffer_drawable_bindings = env.objc.borrow::<EAGLContextHostObject>(group).renderbuffer_drawable_bindings.clone();
    env.objc.borrow_mut::<EAGLContextHostObject>(this).wrapper_sizes=env.objc.borrow::<EAGLContextHostObject>(group).wrapper_sizes.clone();
    this
}

- (id)initWithAPI:(EAGLRenderingAPI)api {
    if api != kEAGLRenderingAPIOpenGLES1 {
        log!(
            "TODO: App requested EAGL initWithAPI:{}, returning nil as we only support API 1 for now",
            api
        );
        return nil;
    }

    let can_share = crate::gles::gpu_transfer::enabled()
        && !env.framework_state.opengles.host_share_attempted
        && env.window.as_ref().is_some_and(|w|w.internal_is_native_gles1());
    env.framework_state.opengles.host_share_attempted = true;
    let shared = if can_share {
        // Only the first independent guest group joins the internal group.
        // Keep the existing SDL views, drawable and compositor context.
        { let mut g = env.window.as_mut().unwrap().make_internal_gl_ctx_current(); unsafe { g.Flush(); } }
        env.window.as_mut().unwrap().set_share_with_current_context(true);
        let result = env.on_parent_stack_in_coroutine(|window, _| {
            crate::gles::GLESImplementation::GLES1Native.construct(window)
        });
        env.window.as_mut().unwrap().set_share_with_current_context(false);
        result.ok()
    } else { None };
    let shares_host = shared.is_some();
    let mut gles1_ins = shared.unwrap_or_else(||create_gles1_ctx(env));
    env.objc.borrow_mut::<EAGLContextHostObject>(this).shares_host = shares_host;
    crate::diagnostics::event(&format!("{{\"event\":\"gpu_transfer_context\",\"shared\":{shares_host}}}"));

    let window = env.window.as_mut().expect("OpenGL ES is not supported in headless mode");
    {
        let gles1_ctx = gles1_ins.make_current(window);
        log!("Driver info: {}", unsafe { gles1_ctx.driver_description() });
    }

    env.objc.borrow_mut::<EAGLContextHostObject>(this).gles_ctx = Some(gles1_ins);

    this
}

- (EAGLRenderingAPI)API {
    // TODO: support later API versions
    kEAGLRenderingAPIOpenGLES1
}

- (id)sharegroup {
    // We use object itself as the sharegroup.
    // Check initWithAPI:sharegroup: for more info
    this
}

- (())dealloc {
    super::direct_texture::context_will_drop(env,this);
    let host_obj = env.objc.borrow_mut::<EAGLContextHostObject>(this);
    for &(guest_buf, _host_buf) in host_obj.mapped_buffers.values() {
        env.mem.free(guest_buf);
    }
    if Rc::strong_count(&host_obj.renderbuffer_drawable_bindings) == 1 {
        let bindings = std::mem::take(&mut host_obj.renderbuffer_drawable_bindings);
        for (_renderbuffer, drawable) in bindings.take() {
            release(env, drawable);
        }
    }
    env.objc.dealloc_object(this, &mut env.mem);
}

- (bool)renderbufferStorage:(NSUInteger)target
               fromDrawable:(id)drawable { // EAGLDrawable (always CAEAGLayer*)
    super::direct_texture::before_storage(env,target,"drawable_storage");
    assert!(drawable != nil); // TODO: handle unbinding

    assert!(target == gles11::RENDERBUFFER_OES);

    let props: id = msg![env; drawable drawableProperties];

    let format_key = get_static_str(env, kEAGLDrawablePropertyColorFormat);
    let format_rgba8 = get_static_str(env, kEAGLColorFormatRGBA8);
    let format_rgb565 = get_static_str(env, kEAGLColorFormatRGB565);

    let format: id = msg![env; props objectForKey:format_key];
    // Theoretically this should map formats like:
    // - kColorFormatRGBA8 => RGBA8_OES
    // - kColorFormatRGB565 => RGB565_OES
    // However, the specification of EXT_framebuffer_object allows the
    // implementation to arbitrarily restrict which formats can be rendered to,
    // and it seems like RGB565 isn't supported, at least on a machine with
    // Intel HD Graphics 615 running macOS Monterey. I don't think RGBA8 is
    // guaranteed either, but it at least seems to work.
    if !msg![env; format isEqual:format_rgba8] && !msg![env; format isEqual:format_rgb565] {
        log!("[renderbufferStorage:{:?} fromDrawable:{:?}] Warning: unhandled format {:?}, using RGBA8", target, drawable, format);
    }
    let internalformat = gles11::RGBA8_OES;

    let (guest_width,guest_height) = {
        let bounds: CGRect = msg![env; drawable bounds];
        let CGSize { width, height } = bounds.size;
        assert!((0.0..(u32::MAX as f32)).contains(&width));
        assert!((0.0..(u32::MAX as f32)).contains(&height));
        let scale_hack = env.options.scale_hack.get();
        let contents_scale: f32 = msg![env; drawable contentsScale];
        let size = ((width * contents_scale).round() as u32 * scale_hack,
                    (height * contents_scale).round() as u32 * scale_hack);
        log!("Guest Retina drawable: points={}x{}, contentsScale={}, pixels={}x{}", width, height, contents_scale, size.0, size.1);
        if size.0 > 0 && size.1 > 0 {
            env.framework_state.opengles.guest_drawable_created = true;
        }
        size
    };

    let surface=env.window().wrapper_render_target().and_then(|target|crate::wrapper_resolution::Surface::hd(crate::wrapper_resolution::Size::new(guest_width,guest_height),target));
    let (width,height)=surface.map(|s|(s.actual.width,s.actual.height)).unwrap_or((guest_width,guest_height));
    let window = env.window.as_mut().expect("OpenGL ES is not supported in headless mode");

    let renderbuffer = {
        // Unclear from documentation if this method requires an appropriate
        // context to already be active, but that seems to be the case
        // in practice?
        let mut gles = super::sync_context(&mut env.framework_state.opengles, &mut env.objc, window, env.current_thread);
        unsafe {
            if let Err(reason)=crate::gles::full_device_budget::drawable(gles.as_mut(),width.try_into().unwrap(),height.try_into().unwrap()){
                crate::diagnostics::event(&format!("{{\"event\":\"full_device_storage_rejected\",\"reason\":\"{reason}\"}}"));
                panic!("Full Device test stopped: {reason}. Use C50 for Retina.");
            }
            gles.RenderbufferStorageOES(target, internalformat, width.try_into().unwrap(), height.try_into().unwrap());
            let mut renderbuffer = 0;
            gles.GetIntegerv(gles11::RENDERBUFFER_BINDING_OES, &mut renderbuffer);
            if env.options.wrapper_resolution.is_some(){let(mut aw,mut ah)=(0,0);gles.GetRenderbufferParameterivOES(target,gles11::RENDERBUFFER_WIDTH_OES,&mut aw);gles.GetRenderbufferParameterivOES(target,gles11::RENDERBUFFER_HEIGHT_OES,&mut ah);if aw!=width as i32||ah!=height as i32{crate::diagnostics::event(&format!("{{\"event\":\"wrapper_drawable_storage_failed\",\"requested\":[{width},{height}],\"actual\":[{aw},{ah}]}}"));return false;}}
            renderbuffer as _
        }
    };

    if let Some(surface)=surface{
        super::wrapper_raster::storage(env,renderbuffer,surface.guest,surface.actual);
        super::wrapper_raster::refresh(env);

    }
    if let Some(mode)=env.options.wrapper_resolution {
        crate::diagnostics::event(&format!("{{\"event\":\"wrapper_drawable_storage\",\"mode\":\"{}\",\"guest\":[{guest_width},{guest_height}],\"actual\":[{width},{height}],\"renderbuffer\":{renderbuffer},\"allocation_queried\":true,\"guest_requested_storage\":{},\"internal_raster_changed\":{}}}",mode.name(),mode==crate::wrapper_resolution::Mode::Original,(width,height)!=(guest_width,guest_height)));
    }
    retain(env, drawable);
    let host_obj = env.objc.borrow_mut::<EAGLContextHostObject>(this);
    let maybe_old_drawable = host_obj.renderbuffer_drawable_bindings.borrow_mut().insert(
        renderbuffer,
        drawable
    );
    if let Some(old_drawable) = maybe_old_drawable {
        release(env, old_drawable);
    }

    true
}

- (bool)presentRenderbuffer:(NSUInteger)target {
    assert!(target == gles11::RENDERBUFFER_OES);

    // The presented frame should be displayed ASAP, but the next one must be
    // delayed, so this needs to be checked before returning.
    let adaptive=crate::frame_pacing::target(env.peggle_hd_is_loading());
    let context=env.objc.borrow_mut::<EAGLContextHostObject>(this);
    if context.pacing_target!=adaptive{context.next_frame_due=None;context.pacing_target=adaptive;}
    let sleep_for = limit_framerate(&mut context.next_frame_due, adaptive.map(|v|v as f64).or(env.options.fps_limit));

    if env.options.print_fps {
        env
            .objc
            .borrow_mut::<EAGLContextHostObject>(this)
            .fps_counter
            .get_or_insert_with(FpsCounter::start)
            .count_frame(format_args!("EAGLContext {this:?}"));
    }

    // UIKit text fields can require composition over the game's layer. Keep
    // iOS on one host presentation context throughout, so creating/editing a
    // field never switches the native drawable or keyboard owner.
    let fullscreen_layer = if cfg!(target_os = "ios") { nil } else { find_fullscreen_eagl_layer(env) };

    let present_context_timing = crate::diagnostics::scope(crate::diagnostics::Phase::PresentContext);
    // Unclear from documentation if this method requires the context to be
    // current, but it would be weird if it didn't?
    let window = env.window.as_mut().expect("OpenGL ES is not supported in headless mode");
    let mut gles = super::sync_context(&mut env.framework_state.opengles, &mut env.objc, window, env.current_thread);

    let renderbuffer: GLuint = unsafe {
        let mut renderbuffer = 0;
        gles.GetIntegerv(gles11::RENDERBUFFER_BINDING_OES, &mut renderbuffer);
        renderbuffer as _
    };

    std::mem::drop(gles);
    drop(present_context_timing);

    let Some(&drawable) = env
        .objc
        .borrow::<EAGLContextHostObject>(this)
        .renderbuffer_drawable_bindings
        .borrow()
        .get(&renderbuffer) else {
        log_dbg!("Can't present a renderbuffer {:?} not bound to a drawable!", renderbuffer);
        return false;
    };

    // We're presenting to the opaque CAEAGLLayer that covers the screen.
    // We can use the fast path where we skip composition and present directly.
    if super::direct_texture::try_present(env,this,drawable,renderbuffer) || try_gpu_transfer(env, this, drawable, renderbuffer) {
        // Composition still runs through the retained internal context.
    } else if crate::gles::full_device_budget::enabled() {
        crate::diagnostics::event("{\"event\":\"full_device_gpu_path_unavailable\",\"continuous_cpu_fallback\":false}");
        panic!("Full Device GPU verification failed. Use C50 for Retina.");
    } else if drawable == fullscreen_layer {
        log_dbg!(
            "Layer {:?} is the fullscreen layer, presenting renderbuffer {:?} directly (fast path).",
            drawable,
            renderbuffer,
        );
        // re-borrow
        unsafe {
            present_renderbuffer(env);
        }
    } else {
        if fullscreen_layer != nil {
            // If there's a single layer that covers the screen, and this isn't
            // it, there's no point in presenting the output because it won't be
            // seen. Using a noisy log because it's a weird scenario and might
            // indicate a bug.
            log!(
                "Layer {:?} is not the fullscreen layer {:?}, skipping presentation of renderbuffer {:?}!",
                drawable,
                fullscreen_layer,
                renderbuffer,
            );
            if let Some(sleep_for) = sleep_for {
                crate::frame_pacing::deliberate_wait(env,sleep_for,adaptive.is_some());
            }
            return true;
        }

        // The very slow and inefficient path: not only does glReadPixels()
        // block the thread until rendering finishes, but the result has to be
        // copied back to system RAM, and then will have to be copied to VRAM
        // again during composition. find_fullscreen_eagl_layer() exists to
        // avoid this.
        log_dbg!(
            "There is no fullscreen layer, presenting renderbuffer {:?} to layer {:?} by copying to RAM (slow path).",
            renderbuffer,
            drawable,
        );
        let pixels_vec = get_pixels_vec_for_presenting(env, drawable);
        // re-borrow
        let (pixels_vec, width, height) = {
            let mut gles = super::sync_context(&mut env.framework_state.opengles, &mut env.objc, env.window.as_mut().unwrap(), env.current_thread);
            unsafe {
                read_renderbuffer(gles.as_mut(), pixels_vec)
            }
        };
        present_pixels(env, drawable, pixels_vec, width, height);
    }

    crate::diagnostics::frame(false);
    if let Some(sleep_for) = sleep_for {
        crate::frame_pacing::deliberate_wait(env,sleep_for,adaptive.is_some());
    }

    if cfg!(target_os = "ios") && env.peggle_hd_is_loading() {
        // Yield to the asset loader without adding a fixed 210 ms delay on
        // physical iOS devices. The existing EAGL limiter and loading-only
        // compositor cap remain active. Simulator retains the fixed delay:
        // its software renderer regresses with the cooperative route.
        static COOPERATIVE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        let cooperative = *COOPERATIVE.get_or_init(|| {
            match std::env::var("PEGGLE_STARTUP_COOPERATIVE_YIELD").as_deref() {
                Ok("0") => false,
                Ok("1") => true,
                _ => cfg!(all(target_os = "ios", not(target_abi = "sim"))),
            }
        });
        env.sleep(if cooperative { Duration::ZERO } else { Duration::from_millis(210) });
    }

    true
}

@end

};

/// Implement framerate limiting.
///
/// The real iPhone OS seems to force 60Hz v-sync in `presentRenderbuffer:`.
/// touchHLE does not force v-sync, and its users might not have 60Hz monitors
/// in any case, so to avoid excessive FPS or games running too fast, we need
/// to simulate it.
///
/// V-sync is essentially a limiter with no "slop", or allowance for frames
/// arriving late: if the frame misses a 60Hz interval, it must wait until the
/// next one. This is quite harsh: if frames consistently arrive very slightly
/// late, the framerate is halved!
///
/// Most games already use NSTimer, which is itself a v-sync-like limiter.
/// For the remainder, let's do something a bit kinder, for the benefit of users
/// with slow systems or which are using high scale hack settings: allow at most
/// an interval's worth of accumulated slop. Allowing infinite accumulation of
/// slop is not desirable, because if the game is running slowly for a long time
/// and suddenly speeds back up, it will then run too fast for a long time.
fn limit_framerate(next_frame_due: &mut Option<Instant>, fps_limit: Option<f64>) -> Option<Duration> {
    let interval = {
        let fps = fps_limit?;
        1.0 / fps
    };
    let interval_rust = Duration::from_secs_f64(interval);

    let &mut Some(current_frame_due) = next_frame_due else {
        // First frame presented: no delay yet.
        *next_frame_due = Some(Instant::now() + interval_rust);
        return None;
    };

    let now = Instant::now();
    *next_frame_due = if now > current_frame_due + interval_rust {
        // Too much slop has accumulated. Make the next frame wait for the next
        // interval.
        log_dbg!("Too much slop accumulated, skipping an interval.");
        Some(
            current_frame_due
                + Duration::from_secs_f64(
                    interval * (((now - current_frame_due).as_secs_f64() / interval).ceil()),
                ),
        )
    } else {
        // Time next frame based on when the current frame was due, not
        // the current time, so as to allow some slop.
        Some(current_frame_due + interval_rust)
    };

    if now < current_frame_due {
        // Frame was presented early, delay it to maintain framerate limit.
        Some(current_frame_due.saturating_duration_since(now))
    } else {
        // Frame was presented on time or late, don't delay.
        None
    }
}

// These helper functions make the state backup code easier to read, but
// more importantly, they make it free of mutable variables that wouldn't
// get caught by Rust's unused variable warnings, which are useful to check
// we actually restore the stuff we back up.

unsafe fn get_ptr(gles: &mut dyn GLES, pname: GLenum) -> *const GLvoid {
    let mut ptr = std::ptr::null();
    gles.GetPointerv(pname, &mut ptr);
    ptr
}
// Safety: caller's responsibility to use appropriate N.
unsafe fn get_ints<const N: usize>(gles: &mut dyn GLES, pname: GLenum) -> [GLint; N] {
    let mut res = [0; N];
    gles.GetIntegerv(pname, res.as_mut_ptr());
    res
}
// Safety: caller's responsibility to only use this for scalars.
unsafe fn get_int(gles: &mut dyn GLES, pname: GLenum) -> GLint {
    get_ints::<1>(gles, pname)[0]
}
// Safety: caller's responsibility to use appropriate N.
unsafe fn get_tex_env_ints<const N: usize>(
    gles: &mut dyn GLES,
    target: GLenum,
    pname: GLenum,
) -> [GLint; N] {
    let mut res = [0; N];
    gles.GetTexEnviv(target, pname, res.as_mut_ptr());
    res
}
// Safety: caller's responsibility to only use this for scalars.
unsafe fn get_tex_env_int(gles: &mut dyn GLES, target: GLenum, pname: GLenum) -> GLint {
    get_tex_env_ints::<1>(gles, target, pname)[0]
}
// Safety: caller's responsibility to use appropriate N.
unsafe fn get_floats<const N: usize>(gles: &mut dyn GLES, pname: GLenum) -> [GLfloat; N] {
    let mut res = [0.0; N];
    gles.GetFloatv(pname, res.as_mut_ptr());
    res
}
unsafe fn get_renderbuffer_size(gles: &mut dyn GLES) -> (GLsizei, GLsizei) {
    let mut width: GLint = 0;
    let mut height: GLint = 0;
    gles.GetRenderbufferParameterivOES(
        gles11::RENDERBUFFER_OES,
        gles11::RENDERBUFFER_WIDTH_OES,
        &mut width,
    );
    gles.GetRenderbufferParameterivOES(
        gles11::RENDERBUFFER_OES,
        gles11::RENDERBUFFER_HEIGHT_OES,
        &mut height,
    );
    (width, height)
}

#[cfg(target_os = "ios")]
fn save_diagnostic_frame(pixels: &[u8], width: u32, height: u32, frame: usize, target: &str) {
    use std::io::Write;
    let Some(data_dir) = std::env::var_os("TOUCHHLE_USER_DATA_DIR") else { return; };
    let directory = std::path::PathBuf::from(data_dir).join("peggle-frame-diagnostics");
    let result = (|| -> std::io::Result<()> {
        std::fs::create_dir_all(&directory)?;
        let mut file = std::fs::File::create(directory.join(format!("{target}-{frame}.ppm")))?;
        write!(file, "P6\n{width} {height}\n255\n")?;
        for row in pixels.chunks_exact(width as usize * 4).rev() {
            let rgb: Vec<u8> = row.chunks_exact(4).flat_map(|p| p[..3].iter().copied()).collect();
            file.write_all(&rgb)?;
        }
        Ok(())
    })();
    let colored = pixels.chunks_exact(4).filter(|p| p[..3].iter().any(|c| *c != 0)).count();
    log!("iOS diagnostic {target} frame {frame}: {width}x{height}, nonblack pixels={colored}, saved={:?}", result);
}

/// Copies the pixels in a renderbuffer bound to `GL_RENDERBUFFER_BINDING_OES`
/// (which should be provided by the app) to a provided [Vec], trying to avoid
/// noticeably modifying OpenGL ES state while doing so.
///
/// This uses `glReadPixels()`, with all the associated performance risks. Any
/// existing content in the [Vec] will bereplaced. The format is RGBA8.
/// The returned values are the [Vec], the width and height.
///
/// The provided context must be current.
unsafe fn read_renderbuffer(gles: &mut dyn GLES, mut pixel_buffer: Vec<u8>) -> (Vec<u8>, u32, u32) {
    let renderbuffer: GLuint = get_int(gles, gles11::RENDERBUFFER_BINDING_OES) as _;
    let (width, height) = get_renderbuffer_size(gles);
    let width_u32: u32 = width.try_into().unwrap();
    let height_u32: u32 = height.try_into().unwrap();

    // To avoid confusing the guest app, we need to be able to undo any
    // state changes we make.
    let old_framebuffer: GLuint = get_int(gles, gles11::FRAMEBUFFER_BINDING_OES) as _;
    let old_pack = get_int(gles, gles11::PACK_ALIGNMENT);
    gles.PixelStorei(gles11::PACK_ALIGNMENT, 1);

    // Create a framebuffer we can use to read from the renderbuffer
    let mut src_framebuffer = 0;
    gles.GenFramebuffersOES(1, &mut src_framebuffer);
    gles.BindFramebufferOES(gles11::FRAMEBUFFER_OES, src_framebuffer);
    gles.FramebufferRenderbufferOES(
        gles11::FRAMEBUFFER_OES,
        gles11::COLOR_ATTACHMENT0_OES,
        gles11::RENDERBUFFER_OES,
        renderbuffer,
    );

    // Read the pixels
    let size = (width_u32 as usize)
        .checked_mul(height_u32 as usize)
        .unwrap()
        .checked_mul(4)
        .unwrap();
    pixel_buffer.clear();
    pixel_buffer.reserve_exact(size);
    let before = Instant::now();
    let readback_timing = crate::diagnostics::scope(crate::diagnostics::Phase::Readback);
    gles.ReadPixels(
        0,
        0,
        width,
        height,
        gles11::RGBA,
        gles11::UNSIGNED_BYTE,
        pixel_buffer.as_mut_ptr() as *mut _,
    );
    drop(readback_timing);
    log_dbg!(
        "glReadPixels(0, 0, {}, {}, …) took {:?}",
        width,
        height,
        Instant::now().saturating_duration_since(before)
    );
    pixel_buffer.set_len(size);

    // Clean up the framebuffer object since we no longer need it.
    gles.DeleteFramebuffersOES(1, &src_framebuffer);

    gles.PixelStorei(gles11::PACK_ALIGNMENT, old_pack);
    // Restore the framebuffer binding
    gles.BindFramebufferOES(gles11::FRAMEBUFFER_OES, old_framebuffer);

    (pixel_buffer, width_u32, height_u32)
}

/// Copies the pixels in a renderbuffer bound to `GL_RENDERBUFFER_BINDING_OES`
/// (which should be provided by the app) to a texture and presents it with
/// [present_frame], trying to avoid noticeably modifying OpenGL ES state while
/// doing so. The front and back buffers are then swapped.
unsafe fn present_renderbuffer(env: &mut Environment) {
    #[cfg(target_os = "ios")]
    static DIAGNOSTICS_REPORTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    #[cfg(target_os = "ios")]
    let report_diagnostics = env.options.print_fps &&
        !DIAGNOSTICS_REPORTED.swap(true, std::sync::atomic::Ordering::Relaxed);
    #[cfg(target_os = "ios")]
    static DIAGNOSTIC_FRAME: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    #[cfg(target_os = "ios")]
    let diagnostic_frame = if env.options.print_fps {
        DIAGNOSTIC_FRAME.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1
    } else { 0 };
    #[cfg(target_os = "ios")]
    let capture_diagnostics = matches!(diagnostic_frame, 1 | 60 | 300 | 600);
    // Save these for when we need to draw the frame
    let viewport = env.window.as_mut().unwrap().viewport();
    let rotation_matrix = env.window.as_mut().unwrap().rotation_matrix();
    let virtual_cursor_visible_at = env.window.as_mut().unwrap().virtual_cursor_visible_at();

    let gles_ctx = super::get_thread_context(
        &mut env.framework_state.opengles,
        &mut env.objc,
        env.current_thread,
    );

    let mut gles_boxed = gles_ctx.make_current(env.window.as_mut().unwrap());
    let gles = gles_boxed.as_mut();

    // A guest can leave different server/client texture units selected.
    // Presentation always uses unit zero, and temporarily disables secondary
    // units so their texture combiners cannot alter the presentation quad.
    let old_active_texture = get_int(gles, gles11::ACTIVE_TEXTURE) as GLenum;
    let old_client_active_texture = get_int(gles, gles11::CLIENT_ACTIVE_TEXTURE) as GLenum;
    let texture_units = get_int(gles, gles11::MAX_TEXTURE_UNITS).clamp(1, 32) as u32;
    let mut secondary_texture_units = Vec::new();
    for unit in 1..texture_units {
        gles.ActiveTexture(gles11::TEXTURE0 + unit);
        secondary_texture_units.push(gles.IsEnabled(gles11::TEXTURE_2D));
        gles.Disable(gles11::TEXTURE_2D);
    }
    gles.ActiveTexture(gles11::TEXTURE0);
    gles.ClientActiveTexture(gles11::TEXTURE0);
    #[cfg(target_os = "ios")]
    if report_diagnostics {
        log!("iOS presentation texture units: server={:#x}, client={:#x}, maximum={}",
            old_active_texture, old_client_active_texture, texture_units);
    }

    // We can't directly copy the content of the renderbuffer to the default
    // framebuffer (the window), but if we attach it to a framebuffer object, we
    // can use glCopyTexImage2D() to copy it to a texture, which we can then
    // draw to the default framebuffer via a textured quad, which can be
    // rotated, scaled or letterboxed as appropriate.

    let renderbuffer: GLuint = get_int(gles, gles11::RENDERBUFFER_BINDING_OES) as _;
    let (width, height) = get_renderbuffer_size(gles);

    // To avoid confusing the guest app, we need to be able to undo any
    // state changes we make.
    let old_framebuffer: GLuint = get_int(gles, gles11::FRAMEBUFFER_BINDING_OES) as _;
    let old_texture_2d: GLuint = get_int(gles, gles11::TEXTURE_BINDING_2D) as _;

    // Create a framebuffer we can use to read from the renderbuffer
    let mut src_framebuffer = 0;
    gles.GenFramebuffersOES(1, &mut src_framebuffer);
    gles.BindFramebufferOES(gles11::FRAMEBUFFER_OES, src_framebuffer);
    gles.FramebufferRenderbufferOES(
        gles11::FRAMEBUFFER_OES,
        gles11::COLOR_ATTACHMENT0_OES,
        gles11::RENDERBUFFER_OES,
        renderbuffer,
    );

    #[cfg(target_os = "ios")]
    if report_diagnostics {
        let mut prior_errors = Vec::new();
        for _ in 0..16 {
            let error = gles.GetError();
            if error == gles11::NO_ERROR { break; }
            prior_errors.push(error);
        }
        log!("iOS presentation source: {}x{}, FBO status={:#x}, pre-existing errors={:x?}",
            width, height, gles.CheckFramebufferStatusOES(gles11::FRAMEBUFFER_OES), prior_errors);
    }
    #[cfg(target_os = "ios")]
    if capture_diagnostics {
        let (pixels, w, h) = read_renderbuffer(gles, Vec::new());
        save_diagnostic_frame(&pixels, w, h, diagnostic_frame, "guest");
    }

    // Create a texture with a copy of the pixels in the framebuffer
    let mut texture: GLuint = 0;
    gles.GenTextures(1, &mut texture);
    gles.BindTexture(gles11::TEXTURE_2D, texture);
    gles.CopyTexImage2D(
        gles11::TEXTURE_2D,
        0,
        gles11::RGB as _,
        0,
        0,
        width,
        height,
        0,
    );
    #[cfg(target_os = "ios")]
    if report_diagnostics {
        log!("iOS presentation texture-copy error={:#x}", gles.GetError());
    }
    // The texture will not have any mip levels so we must ensure the filter
    // does not use them, else rendering will fail.
    gles.TexParameteri(
        gles11::TEXTURE_2D,
        gles11::TEXTURE_MIN_FILTER,
        gles11::LINEAR as _,
    );

    // Clean up the framebuffer object. present_frame explicitly binds the
    // host drawable; framebuffer zero is not a window draw target on UIKit.
    gles.DeleteFramebuffersOES(1, &src_framebuffer);

    // Reset various things that could affect the quad or virtual cursor we're
    // going to draw. Back up the old state while doing so, so it can be
    // restored later. The app's subsequent drawing will be messed up if we
    // don't restore it.
    let old_arrays = {
        let mut old_arrays = [gles11::FALSE; gles1_on_gl2::ARRAYS.len()];
        for (is_enabled, info) in old_arrays.iter_mut().zip(gles1_on_gl2::ARRAYS.iter()) {
            gles.GetBooleanv(info.name, is_enabled);
            gles.DisableClientState(info.name);
        }
        old_arrays
    };
    let old_capabilities = {
        let mut old_capabilities = [gles11::FALSE; gles1_on_gl2::CAPABILITIES.len()];
        for (is_enabled, &name) in old_capabilities
            .iter_mut()
            .zip(gles1_on_gl2::CAPABILITIES.iter())
        {
            gles.GetBooleanv(name, is_enabled);
            gles.Disable(name);
        }
        old_capabilities
    };
    let old_matrix_mode: GLenum = get_int(gles, gles11::MATRIX_MODE) as _;
    // Native GLES 1 projection/texture stacks can have only two entries. A
    // guest may already use both; pushing another for host presentation would
    // overflow, then the unmatched pop would corrupt the guest's stack.
    let old_matrices = [
        (gles11::MODELVIEW, gles11::MODELVIEW_MATRIX),
        (gles11::PROJECTION, gles11::PROJECTION_MATRIX),
        (gles11::TEXTURE, gles11::TEXTURE_MATRIX),
    ].map(|(mode, pname)| {
        let matrix = get_floats::<16>(gles, pname);
        gles.MatrixMode(mode);
        gles.LoadIdentity();
        (mode, matrix)
    });
    let old_color: [GLfloat; 4] = get_floats(gles, gles11::CURRENT_COLOR);
    gles.Color4f(1.0, 1.0, 1.0, 1.0);

    // Back up other things that will be modified while drawing.
    let old_viewport: (GLint, GLint, GLsizei, GLsizei) = {
        let [x, y, width, height] = get_ints(gles, gles11::VIEWPORT);
        (x, y, width as _, height as _)
    };
    let old_clear_color: [GLfloat; 4] = get_floats(gles, gles11::COLOR_CLEAR_VALUE);
    let old_array_buffer: GLuint = get_int(gles, gles11::ARRAY_BUFFER_BINDING) as _;
    let old_vertex_array_binding: GLuint = get_int(gles, gles11::VERTEX_ARRAY_BUFFER_BINDING) as _;
    let old_vertex_array_size: GLint = get_int(gles, gles11::VERTEX_ARRAY_SIZE);
    let old_vertex_array_type: GLenum = get_int(gles, gles11::VERTEX_ARRAY_TYPE) as _;
    let old_vertex_array_stride: GLsizei = get_int(gles, gles11::VERTEX_ARRAY_STRIDE) as _;
    let old_vertex_array_pointer = get_ptr(gles, gles11::VERTEX_ARRAY_POINTER);
    let old_tex_coord_array_binding: GLuint =
        get_int(gles, gles11::TEXTURE_COORD_ARRAY_BUFFER_BINDING) as _;
    let old_tex_coord_array_size: GLint = get_int(gles, gles11::TEXTURE_COORD_ARRAY_SIZE);
    let old_tex_coord_array_type: GLenum = get_int(gles, gles11::TEXTURE_COORD_ARRAY_TYPE) as _;
    let old_tex_coord_array_stride: GLsizei =
        get_int(gles, gles11::TEXTURE_COORD_ARRAY_STRIDE) as _;
    let old_tex_coord_array_pointer = get_ptr(gles, gles11::TEXTURE_COORD_ARRAY_POINTER);
    let old_blend_sfactor: GLenum = get_int(gles, gles11::BLEND_SRC) as _;
    let old_blend_dfactor: GLenum = get_int(gles, gles11::BLEND_DST) as _;

    let old_tex_env_mode = get_tex_env_int(gles, gles11::TEXTURE_ENV, gles11::TEXTURE_ENV_MODE);
    // if the mode is REPLACE, we don't have to reset the other texture
    // environment values
    let tex_env_mode_arr = [gles11::REPLACE; 1];
    gles.TexEnviv(
        gles11::TEXTURE_ENV,
        gles11::TEXTURE_ENV_MODE,
        tex_env_mode_arr.as_ptr().cast(),
    );

    // Draw the quad
    present_frame(gles, viewport, rotation_matrix, virtual_cursor_visible_at);
    #[cfg(target_os = "ios")]
    if capture_diagnostics {
        let mut pixels = vec![0u8; viewport.2 as usize * viewport.3 as usize * 4];
        gles.ReadPixels(viewport.0 as _, viewport.1 as _, viewport.2 as _, viewport.3 as _,
            gles11::RGBA, gles11::UNSIGNED_BYTE, pixels.as_mut_ptr().cast());
        log!("iOS diagnostic readback viewport={:?}, error={:#x}", viewport, gles.GetError());
        save_diagnostic_frame(&pixels, viewport.2, viewport.3, diagnostic_frame, "host");
    }
    #[cfg(target_os = "ios")]
    if report_diagnostics {
        log!("iOS presentation drawable: FBO={}, colorbuffer={}, status={:#x}, draw error={:#x}",
            get_int(gles, gles11::FRAMEBUFFER_BINDING_OES), get_int(gles, gles11::RENDERBUFFER_BINDING_OES),
            gles.CheckFramebufferStatusOES(gles11::FRAMEBUFFER_OES), gles.GetError());
    }

    // Clean up the texture
    gles.DeleteTextures(1, &texture);

    // Restore all the state saved before rendering
    for (&is_enabled, info) in old_arrays.iter().zip(gles1_on_gl2::ARRAYS.iter()) {
        match is_enabled {
            gles11::TRUE => gles.EnableClientState(info.name),
            gles11::FALSE => gles.DisableClientState(info.name),
            _ => unreachable!(),
        }
    }
    for (&is_enabled, &name) in old_capabilities
        .iter()
        .zip(gles1_on_gl2::CAPABILITIES.iter())
    {
        match is_enabled {
            gles11::TRUE => gles.Enable(name),
            gles11::FALSE => gles.Disable(name),
            _ => unreachable!(),
        }
    }
    for (mode, matrix) in old_matrices {
        gles.MatrixMode(mode);
        gles.LoadMatrixf(matrix.as_ptr());
    }
    gles.MatrixMode(old_matrix_mode);
    gles.Color4f(old_color[0], old_color[1], old_color[2], old_color[3]);
    gles.Viewport(
        old_viewport.0,
        old_viewport.1,
        old_viewport.2,
        old_viewport.3,
    );
    gles.ClearColor(
        old_clear_color[0],
        old_clear_color[1],
        old_clear_color[2],
        old_clear_color[3],
    );
    // GL_ARRAY_BUFFER is implicitly used by the Pointer functions but is also
    // an independent binding.
    gles.BindBuffer(gles11::ARRAY_BUFFER, old_vertex_array_binding);
    gles.VertexPointer(
        old_vertex_array_size,
        old_vertex_array_type,
        old_vertex_array_stride,
        old_vertex_array_pointer,
    );
    gles.BindBuffer(gles11::ARRAY_BUFFER, old_tex_coord_array_binding);
    gles.TexCoordPointer(
        old_tex_coord_array_size,
        old_tex_coord_array_type,
        old_tex_coord_array_stride,
        old_tex_coord_array_pointer,
    );
    gles.BindBuffer(gles11::ARRAY_BUFFER, old_array_buffer);
    gles.BlendFunc(old_blend_sfactor, old_blend_dfactor);

    let old_tex_env_mode_arr = [old_tex_env_mode; 1];
    gles.TexEnviv(
        gles11::TEXTURE_ENV,
        gles11::TEXTURE_ENV_MODE,
        old_tex_env_mode_arr.as_ptr().cast(),
    );

    std::mem::drop(gles_boxed);

    // present_frame left the host drawable framebuffer/colorbuffer bound.
    env.window.as_ref().unwrap().swap_window();

    let mut gles_boxed = gles_ctx.make_current(env.window.as_mut().unwrap());
    let gles = gles_boxed.as_mut();

    // Restore the other bindings
    gles.BindTexture(gles11::TEXTURE_2D, old_texture_2d);
    gles.BindFramebufferOES(gles11::FRAMEBUFFER_OES, old_framebuffer);
    gles.BindRenderbufferOES(gles11::RENDERBUFFER_OES, renderbuffer);

    for (unit, enabled) in (1..texture_units).zip(secondary_texture_units) {
        gles.ActiveTexture(gles11::TEXTURE0 + unit);
        if enabled == gles11::TRUE { gles.Enable(gles11::TEXTURE_2D); }
        else { gles.Disable(gles11::TEXTURE_2D); }
    }
    gles.ActiveTexture(old_active_texture);
    gles.ClientActiveTexture(old_client_active_texture);

    // { let err = gles.GetError(); if err != 0 { panic!("{:#x}", err); } }
}

/// No guest callbacks or interpreter execution in this GPU transfer. A failed
/// guard leaves the normal readback route responsible for this frame.
fn try_gpu_transfer(env:&mut Environment, context:id, drawable:id, renderbuffer:u32)->bool {
    use crate::gles::gpu_transfer as transfer;
    use crate::frameworks::core_animation::ca_eagl_layer as layer;
    if !transfer::enabled() {return false;}
    let host=env.objc.borrow::<EAGLContextHostObject>(context);
    if !host.shares_host || host.gpu_disabled {return false;}
    if !layer::is_opaque(env,drawable) {return false;}
    let host=env.objc.borrow_mut::<EAGLContextHostObject>(context);
    if !host.transfers.contains_key(&drawable) {
        if env.framework_state.opengles.gpu_transfer_slots>=4 {return false;}
        env.framework_state.opengles.gpu_transfer_slots+=1;
    }
    let mut state=host.transfers.remove(&drawable).unwrap_or_default();
    let _timing=crate::diagnostics::scope(crate::diagnostics::Phase::GpuTransfer);
    let wait_probe=state.wait_probe_due(env.peggle_hd_is_loading());
    let (copied,prior)={
        let mut g={
            let _context=crate::diagnostics::scope(crate::diagnostics::Phase::GpuProducerContext);
            super::sync_context(&mut env.framework_state.opengles,&mut env.objc,env.window.as_mut().unwrap(),env.current_thread)
        };
        unsafe {transfer::copy_with_probe(g.as_mut(),&mut state,renderbuffer,wait_probe)}
    };
    let queue=&mut env.objc.borrow_mut::<EAGLContextHostObject>(context).pending_errors;
    transfer::enqueue_errors(queue,prior);
    let mut failure=None;
    let mut published=false;
    match copied {
        Err(reason)=>failure=Some(reason),
        Ok(frame)=>{
            if state.requires_oracle() {
                let pixels=get_pixels_vec_for_presenting(env,drawable);
                let (pixels,w,h)={
                    let mut g=super::sync_context(&mut env.framework_state.opengles,&mut env.objc,env.window.as_mut().unwrap(),env.current_thread);
                    unsafe {read_renderbuffer(g.as_mut(),pixels)}
                };
                let actual={
                    let mut g=env.window.as_mut().unwrap().make_internal_gl_ctx_current();
                    unsafe {transfer::read_shared(g.as_mut(),frame)}
                };
                let matched=actual.as_ref().is_ok_and(|v|transfer::matches_opaque(&pixels,v));
                state.oracle_attempts+=1;
                let detailed=transfer::has_color_detail(&pixels);
                if matched && detailed {state.record_verified_frame();}
                crate::diagnostics::event(&format!("{{\"event\":\"gpu_transfer_oracle\",\"width\":{w},\"height\":{h},\"verified_frames\":{},\"matched\":{matched},\"color_detail\":{detailed},\"bytes\":{},\"buffer\":{},\"buffers\":{}}}",state.verified_frames(),pixels.len(),state.slot(),state.buffers()));
                if !matched {failure=Some(actual.err().unwrap_or("pixel_mismatch"));}
                else if state.oracle_attempts>=120 && state.requires_oracle() {failure=Some("oracle_no_color_detail");}
                // Oracle frames always present the trusted CPU result.
                present_pixels(env,drawable,pixels,w,h);
                published=true;
            } else {
                layer::present_gpu_frame(env,drawable,frame);
                // Keep an old Vec only as reusable fallback storage, never upload
                // it while the new shared frame is selected by composition.
                published=true;state.frames+=1;
                if state.frames==1 || state.frames%600==0 {
                    crate::diagnostics::event(&format!("{{\"event\":\"gpu_transfer_active\",\"frames\":{},\"width\":{},\"height\":{},\"readback\":false,\"buffer\":{},\"buffers\":{}}}",state.frames,frame.width,frame.height,state.slot(),state.buffers()));
                }
            }
        }
    }
    let host=env.objc.borrow_mut::<EAGLContextHostObject>(context);
    host.transfers.insert(drawable,state);
    if let Some(reason)=failure {
        host.gpu_disabled=true;
        // Existing GPU frames remain valid snapshots; the caller's CPU fallback
        // immediately replaces the current layer's frame.
        crate::diagnostics::event(&format!("{{\"event\":\"gpu_transfer_disabled\",\"reason\":\"{reason}\"}}"));
        return false;
    }
    published
}
