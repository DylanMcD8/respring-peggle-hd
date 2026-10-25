/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. https://mozilla.org/MPL/2.0/ */
//! Opt-in transfer for the exact verified HD image, not a replacement drawable.
//! Producer and internal compositor share API1 objects. Both flush before
//! switching, and the consumer rebinds after each producer update. No Finish.
use super::{gles11_raw as gl, GLES};
use std::sync::atomic::{AtomicBool,AtomicUsize,Ordering};
static ENABLED:AtomicBool=AtomicBool::new(false);
static COPY_BUFFERS:AtomicUsize=AtomicUsize::new(1);
static WAIT_PROBE:AtomicBool=AtomicBool::new(false);
pub fn configure(verified:bool) {
    let requested=std::env::var("PEGGLE_HD_GPU_TRANSFER").as_deref()==Ok("1");
    let enabled=cfg!(target_os="ios")&&verified&&requested;
    let buffers=if enabled&&std::env::var("PEGGLE_HD_COPY_RING").as_deref()==Ok("1"){3}else{1};
    let probe=enabled&&std::env::var("PEGGLE_HD_COPY_WAIT_PROBE").as_deref()==Ok("1");
    ENABLED.store(enabled,Ordering::Relaxed);COPY_BUFFERS.store(buffers,Ordering::Relaxed);WAIT_PROBE.store(probe,Ordering::Relaxed);
    crate::diagnostics::event(&format!("{{\"event\":\"gpu_transfer_configuration\",\"requested\":{requested},\"verified_image\":{verified},\"platform_ios\":{},\"enabled\":{enabled},\"copy_buffers\":{buffers},\"bounded_wait_probe\":{probe}}}",cfg!(target_os="ios")));
}
pub fn enabled()->bool {ENABLED.load(Ordering::Relaxed)}
#[derive(Clone,Copy,Debug)]
pub struct Frame {pub texture:u32,pub width:u32,pub height:u32,pub force_opaque_alpha:bool}
#[derive(Clone,Copy,Default)]
struct Slot {texture:u32,size:(i32,i32),verified_frames:u8}
#[derive(Clone,Copy,Debug)]
pub struct WaitProbe {pub producer_finish_ms:f64,pub copy_after_finish_ms:f64}
pub struct Transfer {
    pub framebuffer:u32,pub size:(i32,i32),pub oracle_attempts:u32,pub frames:u64,pub last_probe:Option<WaitProbe>,
    slots:[Slot;3],buffers:usize,current:usize,next:usize,postload_frames:u64,
}
impl Default for Transfer {fn default()->Self {Self::with_buffers(COPY_BUFFERS.load(Ordering::Relaxed))}}
impl Transfer {
    pub fn with_buffers(buffers:usize)->Self {Self{framebuffer:0,size:(0,0),oracle_attempts:0,frames:0,last_probe:None,slots:[Slot::default();3],buffers:if buffers==3 {3}else{1},current:0,next:0,postload_frames:0}}
    pub fn buffers(&self)->usize {self.buffers}
    pub fn slot(&self)->usize {self.current}
    pub fn verified_frames(&self)->u8 {self.slots[self.current].verified_frames}
    pub fn requires_oracle(&self)->bool {self.slots[..self.buffers].iter().any(|s|s.size!=self.size||s.verified_frames<4)}
    pub fn record_verified_frame(&mut self) {let n=&mut self.slots[self.current].verified_frames;*n=(*n+1).min(4);}
    // Diagnostic only. At most three producer Finish probes per transfer,
    // on postload frames60/180/300. Normal launches never execute Finish here.
    pub fn wait_probe_due(&mut self,loading:bool)->bool {
        self.wait_probe_due_if(loading,WAIT_PROBE.load(Ordering::Relaxed))
    }
    pub fn wait_probe_due_if(&mut self,loading:bool,requested:bool)->bool {
        if !requested||loading||self.requires_oracle() {return false;}
        self.postload_frames+=1;
        matches!(self.postload_frames,60|180|300)
    }
}

pub fn rgba_size(width:i32,height:i32,max:i32)->Option<usize> {
    super::full_device_budget::rgba_bytes(width,height,max)
}
pub fn has_color_detail(source:&[u8])->bool {
    let Some(first)=source.get(..3) else {return false;};
    source.chunks_exact(4).any(|p| &p[..3]!=first)
}
pub fn select_frame(frame:Option<Frame>,opaque:bool,alternate:bool)->Option<Frame> {
    frame.filter(|_|opaque && !alternate)
}
pub fn enqueue_errors(queue:&mut std::collections::VecDeque<u32>,errors:impl IntoIterator<Item=u32>) {
    for e in errors {if e!=gl::NO_ERROR && !queue.contains(&e) {queue.push_back(e);}}
}
pub fn matches_opaque(source:&[u8],texture:&[u8])->bool {
    source.len()==texture.len() && source.len()%4==0 && !source.is_empty()
        && source.chunks_exact(4).zip(texture.chunks_exact(4))
            .all(|(a,b)| a[..3]==b[..3] && b[3]==255)
}
unsafe fn int(g:&mut dyn GLES,p:u32)->i32 {let mut v=0;g.GetIntegerv(p,&mut v);v}
// Preserve pre-existing guest flags. The native API has a finite set of error
// flags; cap collection so a broken driver cannot hang the emulator.
unsafe fn errors(g:&mut dyn GLES)->Vec<u32> {
    let mut e=Vec::new();
    for _ in 0..16 {let x=g.GetError();if x==gl::NO_ERROR {break;}e.push(x);}
    e
}

/// Caller must have made the native, shared producer context current. Only
/// this helper's framebuffer/texture bindings change; restoration is unconditional.
pub unsafe fn copy(g:&mut dyn GLES,t:&mut Transfer,renderbuffer:u32)->(Result<Frame,&'static str>,Vec<u32>) {
    copy_with_probe(g,t,renderbuffer,false)
}
pub unsafe fn copy_with_probe(g:&mut dyn GLES,t:&mut Transfer,renderbuffer:u32,probe:bool)->(Result<Frame,&'static str>,Vec<u32>) {
    let prior={let _s=crate::diagnostics::scope(crate::diagnostics::Phase::GpuCopyErrorsBefore);errors(g)};
    if prior.len()==16 {return(Err("error_queue_saturated"),prior);}
    let query_timing=crate::diagnostics::scope(crate::diagnostics::Phase::GpuCopyQueries);
    let old_fbo=int(g,gl::FRAMEBUFFER_BINDING_OES)as u32;
    let old_active=int(g,gl::ACTIVE_TEXTURE)as u32;g.ActiveTexture(gl::TEXTURE0);
    let old_texture=int(g,gl::TEXTURE_BINDING_2D)as u32;
    let(mut width,mut height,mut format)=(0,0,0);
    g.GetRenderbufferParameterivOES(gl::RENDERBUFFER_OES,gl::RENDERBUFFER_WIDTH_OES,&mut width);
    g.GetRenderbufferParameterivOES(gl::RENDERBUFFER_OES,gl::RENDERBUFFER_HEIGHT_OES,&mut height);
    g.GetRenderbufferParameterivOES(gl::RENDERBUFFER_OES,gl::RENDERBUFFER_INTERNAL_FORMAT_OES,&mut format);
    let max=int(g,gl::MAX_TEXTURE_SIZE);drop(query_timing);
    let result=(|| {
        if format!=gl::RGBA8_OES as i32||rgba_size(width,height,max).is_none(){return Err("unsupported_renderbuffer");}
        if t.size!=(width,height) {
            t.size=(width,height);t.oracle_attempts=0;
            for slot in &mut t.slots {slot.verified_frames=0;}
        }
        t.current=t.next;let slot=&mut t.slots[t.current];
        let setup_timing=crate::diagnostics::scope(crate::diagnostics::Phase::GpuCopySetup);
        if t.framebuffer==0 {g.GenFramebuffersOES(1,&mut t.framebuffer);}
        g.BindFramebufferOES(gl::FRAMEBUFFER_OES,t.framebuffer);
        g.FramebufferRenderbufferOES(gl::FRAMEBUFFER_OES,gl::COLOR_ATTACHMENT0_OES,gl::RENDERBUFFER_OES,renderbuffer);
        if g.CheckFramebufferStatusOES(gl::FRAMEBUFFER_OES)!=gl::FRAMEBUFFER_COMPLETE_OES {return Err("source_incomplete");}
        if slot.texture==0 {g.GenTextures(1,&mut slot.texture);}
        g.BindTexture(gl::TEXTURE_2D,slot.texture);
        let resized=slot.size!=(width,height);
        if resized {
            super::full_device_budget::texture(g,width,height)?;
            for p in [gl::TEXTURE_WRAP_S,gl::TEXTURE_WRAP_T] {g.TexParameteri(gl::TEXTURE_2D,p,gl::CLAMP_TO_EDGE as i32);}
            for p in [gl::TEXTURE_MIN_FILTER,gl::TEXTURE_MAG_FILTER] {g.TexParameteri(gl::TEXTURE_2D,p,gl::LINEAR as i32);}
        }
        drop(setup_timing);
        let before_finish=probe.then(std::time::Instant::now);
        if probe {let _s=crate::diagnostics::scope(crate::diagnostics::Phase::GpuCopyUpstreamWait);g.Finish();}
        let waited=before_finish.map_or(0.0,|start|start.elapsed().as_secs_f64()*1000.0);
        let before_copy=probe.then(std::time::Instant::now);
        {
            let _s=crate::diagnostics::scope(crate::diagnostics::Phase::GpuCopyCommand);
            if resized {g.CopyTexImage2D(gl::TEXTURE_2D,0,gl::RGB,0,0,width,height,0);slot.size=(width,height);}
            else {g.CopyTexSubImage2D(gl::TEXTURE_2D,0,0,0,0,0,width,height);}
        }
        if let Some(start)=before_copy {
            let copied=start.elapsed().as_secs_f64()*1000.0;
            t.last_probe=Some(WaitProbe{producer_finish_ms:waited,copy_after_finish_ms:copied});
            crate::diagnostics::event(&format!("{{\"event\":\"copy_wait_probe\",\"postload_frame\":{},\"buffer\":{},\"buffers\":{},\"producer_finish_ms\":{waited:.6},\"copy_after_finish_ms\":{:.6},\"ordinary_frame\":false}}",t.postload_frames,t.current,t.buffers,copied));
        }
        {let _s=crate::diagnostics::scope(crate::diagnostics::Phase::GpuCopyFlush);g.Flush();}
        Ok(Frame{texture:slot.texture,width:width as u32,height:height as u32,force_opaque_alpha:false})
    })();
    let _restore=crate::diagnostics::scope(crate::diagnostics::Phase::GpuCopyRestoreErrors);
    g.BindTexture(gl::TEXTURE_2D,old_texture);g.ActiveTexture(old_active);g.BindFramebufferOES(gl::FRAMEBUFFER_OES,old_fbo);
    let own=errors(g);
    if !own.is_empty(){return(Err("copy_gl_error"),prior);}
    if result.is_ok(){t.next=(t.current+1)%t.buffers;}
    (result,prior)
}

/// Four startup/resize oracle frames only. Runs in the retained internal
/// context; readback of steady gameplay is never part of this path.
pub unsafe fn read_shared(g:&mut dyn GLES,frame:Frame)->Result<Vec<u8>,&'static str> {
    if !errors(g).is_empty() {return Err("consumer_preexisting_gl_error");}
    let n=rgba_size(frame.width as i32,frame.height as i32,int(g,gl::MAX_TEXTURE_SIZE)).ok_or("consumer_size")?;
    if super::full_device_budget::enabled(){super::full_device_budget::validate(frame.width as i32,frame.height as i32,super::full_device_budget::caps(g))?;}
    let old_fbo=int(g,gl::FRAMEBUFFER_BINDING_OES) as u32;
    let old_pack=int(g,gl::PACK_ALIGNMENT);
    let old_active=int(g,gl::ACTIVE_TEXTURE) as u32;
    g.ActiveTexture(gl::TEXTURE0);
    let old_texture=int(g,gl::TEXTURE_BINDING_2D) as u32;
    g.BindTexture(gl::TEXTURE_2D,frame.texture);
    let mut fbo=0;
    let result=(|| {
        if g.IsTexture(frame.texture)==0 {return Err("texture_not_shared");}
        g.GenFramebuffersOES(1,&mut fbo);
        g.BindFramebufferOES(gl::FRAMEBUFFER_OES,fbo);
        g.FramebufferTexture2DOES(gl::FRAMEBUFFER_OES,gl::COLOR_ATTACHMENT0_OES,gl::TEXTURE_2D,frame.texture,0);
        if g.CheckFramebufferStatusOES(gl::FRAMEBUFFER_OES)!=gl::FRAMEBUFFER_COMPLETE_OES {return Err("shared_texture_incomplete");}
        g.PixelStorei(gl::PACK_ALIGNMENT,1);
        let mut pixels=vec![0;n];
        g.ReadPixels(0,0,frame.width as i32,frame.height as i32,gl::RGBA,gl::UNSIGNED_BYTE,pixels.as_mut_ptr() as *mut _);
        g.Flush();
        Ok(pixels)
    })();
    g.PixelStorei(gl::PACK_ALIGNMENT,old_pack);
    g.BindFramebufferOES(gl::FRAMEBUFFER_OES,old_fbo);
    if fbo!=0 {g.DeleteFramebuffersOES(1,&fbo);}
    g.BindTexture(gl::TEXTURE_2D,old_texture);
    g.ActiveTexture(old_active);
    if errors(g).is_empty() {result}else{Err("consumer_gl_error")}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn color_detail_gate() {
        assert!(!has_color_detail(&[0,0,0,0,0,0,0,255]));
        assert!(!has_color_detail(&[]));
        assert!(has_color_detail(&[0,0,0,0,1,0,0,255]));
    }
    #[test] fn layer_override_policy() {
        let f=Some(Frame{texture:7,width:768,height:1024,force_opaque_alpha:false});
        assert!(select_frame(f,true,false).is_some());
        assert!(select_frame(f,false,false).is_none());
        assert!(select_frame(f,true,true).is_none());
        assert!(select_frame(None,true,false).is_none());
    }
    #[test] fn queued_errors_are_flags_not_duplicate_events() {
        let mut q=std::collections::VecDeque::new();
        enqueue_errors(&mut q,[gl::INVALID_ENUM,gl::INVALID_ENUM,0]);
        enqueue_errors(&mut q,[gl::INVALID_VALUE,gl::INVALID_ENUM]);
        assert_eq!(q.into_iter().collect::<Vec<_>>(),vec![gl::INVALID_ENUM,gl::INVALID_VALUE]);
    }
    #[test] fn limits() {
        assert_eq!(rgba_size(768,1024,4096),Some(3145728));
        assert_eq!(rgba_size(2048,2048,4096),Some(16777216));
        for (w,h,m) in [(0,1,4096),(-1,3,4096),(4096,2049,8192),(4097,1,4096),(i32::MAX,i32::MAX,i32::MAX)] {
            assert_eq!(rgba_size(w,h,m),None);
        }
        assert_eq!(rgba_size(7,5,4096),Some(140));
    }
    #[test] fn opaque_pixel_oracle() {
        assert!(matches_opaque(&[1,2,3,0,4,5,6,128],&[1,2,3,255,4,5,6,255]));
        assert!(!matches_opaque(&[1,2,3,0],&[1,2,4,255]));
        assert!(!matches_opaque(&[1,2,3,0],&[1,2,3,0]));
        assert!(!matches_opaque(&[1,2,3,0],&[]));
        assert!(!matches_opaque(&[],&[]));
        assert!(!matches_opaque(&[1],&[1]));
    }
}
