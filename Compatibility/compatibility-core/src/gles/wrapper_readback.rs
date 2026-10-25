/* MPL-2.0. Temporary logical read/copy surface; preserves the guest API's pixel dimensions. */
use super::{GLES,gles11_raw as gl,direct_texture::drain_errors};
use crate::wrapper_resolution::{Surface,logical_rgba};
/// The caller owns the GL error queue. Setup errors are distinct from errors
/// produced by the guest's actual command; no original owning callback runs twice.
pub unsafe fn with_logical<R>(g:&mut dyn GLES,s:Surface,command:impl FnOnce(&mut dyn GLES)->R)->(Result<R,&'static str>,Vec<u32>){
 if !s.actual.valid()||!s.guest.valid(){return(Err("logical_snapshot_dimensions"),vec![]);}
 let Some(physical_bytes)=super::full_device_budget::rgba_bytes(s.actual.width as i32,s.actual.height as i32,i32::MAX)else{return(Err("logical_snapshot_plane_limit"),vec![]);};
 let logical_bytes=s.guest.width as usize*s.guest.height as usize*4;
 let _budget=match super::full_device_budget::temporary(logical_bytes){Ok(v)=>v,Err(e)=>return(Err(e),vec![])};
 let(mut fbo,mut tex,mut pack,mut unpack)=(0,0,0,0);g.GetIntegerv(gl::FRAMEBUFFER_BINDING_OES,&mut fbo);g.GetIntegerv(gl::TEXTURE_BINDING_2D,&mut tex);g.GetIntegerv(gl::PACK_ALIGNMENT,&mut pack);g.GetIntegerv(gl::UNPACK_ALIGNMENT,&mut unpack);
 let(mut owned_tex,mut owned_fbo)=(0,0);let mut guest_errors=vec![];
 let result=(||{
  let mut physical=vec![0;physical_bytes];g.PixelStorei(gl::PACK_ALIGNMENT,1);g.ReadPixels(0,0,s.actual.width as i32,s.actual.height as i32,gl::RGBA,gl::UNSIGNED_BYTE,physical.as_mut_ptr().cast());
  if !drain_errors(g).is_empty(){return Err("logical_snapshot_read_failed");}
  let logical=logical_rgba(s,&physical).ok_or("logical_snapshot_dimensions")?;
  g.GenTextures(1,&mut owned_tex);g.BindTexture(gl::TEXTURE_2D,owned_tex);g.PixelStorei(gl::UNPACK_ALIGNMENT,1);
  g.TexImage2D(gl::TEXTURE_2D,0,gl::RGBA as i32,s.guest.width as i32,s.guest.height as i32,0,gl::RGBA,gl::UNSIGNED_BYTE,logical.as_ptr().cast());
  for p in [gl::TEXTURE_MIN_FILTER,gl::TEXTURE_MAG_FILTER]{g.TexParameteri(gl::TEXTURE_2D,p,gl::NEAREST as i32);}for p in [gl::TEXTURE_WRAP_S,gl::TEXTURE_WRAP_T]{g.TexParameteri(gl::TEXTURE_2D,p,gl::CLAMP_TO_EDGE as i32);}
  g.GenFramebuffersOES(1,&mut owned_fbo);g.BindFramebufferOES(gl::FRAMEBUFFER_OES,owned_fbo);g.FramebufferTexture2DOES(gl::FRAMEBUFFER_OES,gl::COLOR_ATTACHMENT0_OES,gl::TEXTURE_2D,owned_tex,0);
  let complete=g.CheckFramebufferStatusOES(gl::FRAMEBUFFER_OES)==gl::FRAMEBUFFER_COMPLETE_OES;let errors=drain_errors(g);
  // Guest copy commands must see their original destination texture and pixel-store state.
  g.BindTexture(gl::TEXTURE_2D,tex as u32);g.PixelStorei(gl::PACK_ALIGNMENT,pack);g.PixelStorei(gl::UNPACK_ALIGNMENT,unpack);
  if !complete||!errors.is_empty(){return Err("logical_snapshot_framebuffer_failed");}
  let value=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||command(g)));guest_errors=drain_errors(g);Ok(value)
 })();
 g.BindFramebufferOES(gl::FRAMEBUFFER_OES,fbo as u32);g.BindTexture(gl::TEXTURE_2D,tex as u32);g.PixelStorei(gl::PACK_ALIGNMENT,pack);g.PixelStorei(gl::UNPACK_ALIGNMENT,unpack);
 if owned_fbo!=0{g.DeleteFramebuffersOES(1,&owned_fbo);}if owned_tex!=0{g.DeleteTextures(1,&owned_tex);}
 let cleanup=drain_errors(g);if !cleanup.is_empty(){return(Err("logical_snapshot_restore_failed"),guest_errors);}
 let result=match result{Ok(Ok(v))=>Ok(v),Ok(Err(p))=>std::panic::resume_unwind(p),Err(e)=>Err(e)};(result,guest_errors)
}
