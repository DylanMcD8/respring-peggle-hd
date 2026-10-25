/* This Source Code Form is subject to the Mozilla Public
 * License, v. 2.0. https://mozilla.org/MPL/2.0/ */
//! One guarded, reversible main color target for the verified Peggle HD image.
//! Original guest commands and names are retained; unsupported access restores
//! the original renderbuffer before the original guest API operation executes.
use crate::{Environment,gles::{GLES,gpu_transfer as transfer,direct_texture as direct},objc::id,mem::{ConstPtr,Mem}};
use super::eagl::EAGLContextHostObject;
use crate::gles::gles11_raw as gl;
pub(super) struct State {owner:id,drawable:id,surface:direct::Surface}
fn current(env:&mut Environment)->Option<id>{*env.framework_state.opengles.current_ctx_for_thread(env.current_thread)}
fn owner<T>(env:&mut Environment,context:id,f:impl FnOnce(&mut dyn GLES)->T)->T {
 let (result,prior)={
  let mut g=env.objc.borrow_mut::<EAGLContextHostObject>(context).gles_ctx.as_mut().unwrap().make_current(env.window.as_mut().unwrap());
  let prior=unsafe{direct::drain_errors(g.as_mut())};let result=f(g.as_mut());(result,prior)
 };
 transfer::enqueue_errors(&mut env.objc.borrow_mut::<EAGLContextHostObject>(context).pending_errors,prior);result
}
fn frame(s:&State)->transfer::Frame{transfer::Frame{texture:s.surface.texture,width:s.surface.width as _,height:s.surface.height as _,force_opaque_alpha:true}}
fn fatal(reason:&str)->!{crate::diagnostics::event(&format!("{{\"event\":\"direct_texture_recovery_failed\",\"reason\":\"{reason}\"}}"));panic!("direct texture recovery failed: {reason}")}
fn disable_taken(env:&mut Environment,mut s:State,reason:&str) {
 let caller=current(env);
 owner(env,s.owner,|g|unsafe{g.Flush()});
 if s.surface.mode==direct::Mode::Active {
  let restored={let mut g=env.window.as_mut().unwrap().make_internal_gl_ctx_current();unsafe{direct::restore_renderbuffer(g.as_mut(),s.surface.texture,s.surface.renderbuffer,s.surface.width,s.surface.height)}};
  if let Err(e)=restored{fatal(e);}
 }
 let recovered=owner(env,s.owner,|g|unsafe{s.surface.restore_attachment(g)?;direct::read_renderbuffer(g,s.surface.renderbuffer,s.surface.width,s.surface.height,vec![])});
 let pixels=match recovered{Ok(v)=>v,Err(e)=>fatal(e)};
 crate::frameworks::core_animation::ca_eagl_layer::present_pixels(env,s.drawable,pixels,s.surface.width as _,s.surface.height as _);
 // Keep the single owned texture alive until sharegroup teardown: a retained
 // composition reference may still exist. No new direct target is created.
 // Reacquire the restored shared image in the original caller as well.
 // Its FBO may have been bound long before this recovery in another context.
 if let Some(c)=caller{owner(env,c,|g|unsafe{
  let mut fbo=0;let mut rbo=0;g.GetIntegerv(gl::FRAMEBUFFER_BINDING_OES,&mut fbo);g.GetIntegerv(gl::RENDERBUFFER_BINDING_OES,&mut rbo);
  g.BindFramebufferOES(gl::FRAMEBUFFER_OES,fbo as _);g.BindRenderbufferOES(gl::RENDERBUFFER_OES,rbo as _);
  if !direct::drain_errors(g).is_empty(){fatal("caller_restore_rebind");}
 });}
 env.framework_state.opengles.direct_disabled=true;
 crate::diagnostics::event(&format!("{{\"event\":\"direct_texture_disabled\",\"reason\":\"{reason}\",\"active_frames_before_disable\":{},\"restored_original\":true}}",s.surface.frames));
}
pub(super) fn before_mutation(env:&mut Environment,reason:&str){if let Some(s)=env.framework_state.opengles.direct.take(){disable_taken(env,s,reason);}}
/// Inspect only attachment/storage APIs, not every draw. Unrelated offscreen
/// image creation is normal guest work and must not disable the main target.
/// Original native calls still execute with their arguments and error flags.
pub(super) fn before_attachment(env:&mut Environment,target:u32,resource:Option<(bool,u32)>,reason:&str){
 let caller=current(env);
 if env.framework_state.opengles.direct.as_ref().is_some_and(|s|Some(s.owner)!=caller){after_bind(env);}
 let Some(s)=env.framework_state.opengles.direct.as_ref()else{return;};
 let (main,source,texture,owner_id,frames)=(s.surface.framebuffer,s.surface.renderbuffer,s.surface.texture,s.owner,s.surface.frames);
 let Some(c)=current(env)else{return;};
 let direct_resource=resource.is_some_and(|(is_texture,name)|name!=0&&name==if is_texture{texture}else{source});
 let bound=owner(env,c,|g|unsafe{let mut f=0;g.GetIntegerv(gl::FRAMEBUFFER_BINDING_OES,&mut f);if !direct::drain_errors(g).is_empty(){fatal("attachment_inspection");}f as u32});
 let affected=target==gl::FRAMEBUFFER_OES&&((c==owner_id&&bound==main)||direct_resource);
 // Log each normal offscreen attachment while infrequent, then stop at16.
 // No texture contents, user data or private paths enter diagnostics.
 let n=env.framework_state.opengles.direct_attachment_events;
 if n<16||affected{
  crate::diagnostics::event(&format!("{{\"event\":\"direct_texture_attachment_guard\",\"api\":\"{reason}\",\"target\":{target},\"bound_fbo\":{bound},\"owner_context\":{},\"main_fbo\":{main},\"resource_is_direct\":{direct_resource},\"affected\":{affected},\"active_frames\":{frames}}}",c==owner_id));
  env.framework_state.opengles.direct_attachment_events=n.saturating_add(1);
 }
 if affected{before_mutation(env,reason);}
}
pub(super) fn before_storage(env:&mut Environment,target:u32,reason:&str){
 let Some(s)=env.framework_state.opengles.direct.as_ref()else{return;};let source=s.surface.renderbuffer;
 let Some(c)=current(env)else{return;};
 let bound=owner(env,c,|g|unsafe{let mut r=0;g.GetIntegerv(gl::RENDERBUFFER_BINDING_OES,&mut r);if !direct::drain_errors(g).is_empty(){fatal("storage_inspection");}r as u32});
 if target==gl::RENDERBUFFER_OES&&bound==source{before_mutation(env,reason);}
}
pub(super) fn context_will_drop(env:&mut Environment,c:id){if env.framework_state.opengles.direct.as_ref().is_some_and(|s|s.owner==c){before_mutation(env,"owner_teardown");}}
pub(super) fn before_texture(env:&mut Environment,name:u32){if env.framework_state.opengles.direct.as_ref().is_some_and(|s|s.surface.texture==name){before_mutation(env,"owned_texture_access");}}
pub(super) fn before_delete(env:&mut Environment,n:i32,names:ConstPtr<u32>,kind:u8){
 let Some(s)=env.framework_state.opengles.direct.as_ref()else{return;};
 let target=match kind{0=>s.surface.texture,1=>s.surface.framebuffer,_=>s.surface.renderbuffer};
 if n>0&&(0..n as u32).any(|i|env.mem.read(names+i)==target){before_mutation(env,"source_object_delete");}
}
pub(super) fn after_bind(env:&mut Environment){
 let Some(mut s)=env.framework_state.opengles.direct.take()else{return;};
 let Some(c)=current(env)else{env.framework_state.opengles.direct=Some(s);return;};
 let (bound,alias)=owner(env,c,|g|unsafe{
  let mut b=0;g.GetIntegerv(gl::FRAMEBUFFER_BINDING_OES,&mut b);
  let mut type_=0;let mut name=0;
  if b!=0 {g.GetFramebufferAttachmentParameterivOES(gl::FRAMEBUFFER_OES,gl::COLOR_ATTACHMENT0_OES,gl::FRAMEBUFFER_ATTACHMENT_OBJECT_TYPE_OES,&mut type_);if type_!=0{g.GetFramebufferAttachmentParameterivOES(gl::FRAMEBUFFER_OES,gl::COLOR_ATTACHMENT0_OES,gl::FRAMEBUFFER_ATTACHMENT_OBJECT_NAME_OES,&mut name);}}
  let alias=(type_==gl::RENDERBUFFER_OES as i32&&name==s.surface.renderbuffer as i32)||(type_==gl::TEXTURE as i32&&name==s.surface.texture as i32);
  // These valid inspection queries must not introduce visible guest errors.
  if !direct::drain_errors(g).is_empty(){fatal("binding_inspection");}
  (b as u32,alias)
 });
 if c==s.owner{s.surface.bound_framebuffer=bound;}
 if alias&&(c!=s.owner||bound!=s.surface.framebuffer){disable_taken(env,s,"source_alias_binding");}else{env.framework_state.opengles.direct=Some(s);}
}
pub(super) fn draw(env:&mut Environment,mut command:impl FnMut(&mut dyn GLES,&mut Mem)) {
 // A different guest context can retain an alias binding across context
 // switches. Inspect it before drawing even without a new BindFramebuffer.
 let c=current(env);
 if env.framework_state.opengles.direct.as_ref().is_some_and(|s|Some(s.owner)!=c){after_bind(env);}
 let mut state=env.framework_state.opengles.direct.take();let c=current(env);
 super::gles_guest::with_ctx_and_mem(env,|g,mem|{
  if let Some(s)=state.as_mut().filter(|s|Some(s.owner)==c){unsafe{s.surface.draw(g,|g|command(g,mem));}}else{command(g,mem);}
 });
 env.framework_state.opengles.direct=state;
}
pub(super) fn try_present(env:&mut Environment,context:id,drawable:id,renderbuffer:u32)->bool {
 use crate::frameworks::core_animation::ca_eagl_layer as layer;
 if !direct::enabled()||env.framework_state.opengles.direct_disabled{return false;}
 if let Some(s)=env.framework_state.opengles.direct.as_ref(){
  if s.owner!=context||s.drawable!=drawable||s.surface.renderbuffer!=renderbuffer{return false;}
  if !layer::is_opaque(env,drawable){before_mutation(env,"nonopaque_layer");return false;}
 }else{
  if env.peggle_hd_is_loading()||!layer::is_opaque(env,drawable)||!env.objc.borrow::<EAGLContextHostObject>(context).shares_host{return false;}
  let created=owner(env,context,|g|unsafe{let(result,prior)=direct::Surface::create(g,renderbuffer);(result,prior)});
  transfer::enqueue_errors(&mut env.objc.borrow_mut::<EAGLContextHostObject>(context).pending_errors,created.1);
  match created.0 {
   Ok(surface)=>{crate::diagnostics::event(&format!("{{\"event\":\"direct_texture_shadow\",\"width\":{},\"height\":{}}}",surface.width,surface.height));env.framework_state.opengles.direct=Some(State{owner:context,drawable,surface});},
   Err(reason)=>{env.framework_state.opengles.direct_disabled=true;crate::diagnostics::event(&format!("{{\"event\":\"direct_texture_setup_rejected\",\"reason\":\"{reason}\"}}"));}
  }
  return false;
 }
 let mut s=env.framework_state.opengles.direct.take().unwrap();
 let _timing=crate::diagnostics::scope(crate::diagnostics::Phase::GpuTransfer);
 owner(env,s.owner,|g|unsafe{g.Flush()});
 if s.surface.mode==direct::Mode::Active {
  layer::present_gpu_frame(env,drawable,frame(&s));s.surface.needs_rebind=true;s.surface.frames+=1;
  if s.surface.frames==1||s.surface.frames%600==0{crate::diagnostics::event(&format!("{{\"event\":\"direct_texture_active\",\"frames\":{},\"width\":{},\"height\":{},\"copy\":false,\"readback\":false}}",s.surface.frames,s.surface.width,s.surface.height));}
 }else{
  let reference=owner(env,s.owner,|g|unsafe{direct::read_renderbuffer(g,renderbuffer,s.surface.width,s.surface.height,vec![])});
  let pixels=match reference{Ok(v)=>v,Err(e)=>{disable_taken(env,s,e);return true;}};
  let actual={let mut g=env.window.as_mut().unwrap().make_internal_gl_ctx_current();unsafe{transfer::read_shared(g.as_mut(),frame(&s))}};
  let matched=actual.as_ref().is_ok_and(|v|*v==pixels);let detailed=transfer::has_color_detail(&pixels);let ops=s.surface.mirrored_ops;
  s.surface.attempts+=1;if matched&&detailed&&ops>0{s.surface.verified+=1;}s.surface.mirrored_ops=0;
  crate::diagnostics::event(&format!("{{\"event\":\"direct_texture_oracle\",\"verified_frames\":{},\"attempts\":{},\"mirrored_ops\":{ops},\"matched_rgba\":{matched},\"color_detail\":{detailed},\"bytes\":{}}}",s.surface.verified,s.surface.attempts,pixels.len()));
  if !matched||(s.surface.attempts>=120&&s.surface.verified<4){disable_taken(env,s,if !matched{"rgba_mismatch"}else{"no_mirrored_detail"});return true;}
  if s.surface.verified>=4{
   let restored={let mut g=env.window.as_mut().unwrap().make_internal_gl_ctx_current();unsafe{direct::restore_renderbuffer(g.as_mut(),s.surface.texture,renderbuffer,s.surface.width,s.surface.height)}};
   if let Err(e)=restored{fatal(e);}
   let roundtrip=owner(env,s.owner,|g|unsafe{direct::read_renderbuffer(g,renderbuffer,s.surface.width,s.surface.height,vec![])});
   if !roundtrip.as_ref().is_ok_and(|v|*v==pixels){fatal("restore_rgba_gate");}
   let opaque={let mut g=env.window.as_mut().unwrap().make_internal_gl_ctx_current();unsafe{direct::opaque_oracle(g.as_mut(),s.surface.texture,s.surface.width,s.surface.height)}};
   if !opaque.as_ref().is_ok_and(|v|transfer::matches_opaque(&pixels,v)){disable_taken(env,s,"opaque_gate");return true;}
   let activation=owner(env,s.owner,|g|unsafe{s.surface.activate(g)});
   if let Err(e)=activation{disable_taken(env,s,e);return true;}
   crate::diagnostics::event("{\"event\":\"direct_texture_gate_passed\",\"rgba_frames\":4,\"restore_exact\":true,\"opaque_exact\":true}");
  }
  layer::present_pixels(env,drawable,pixels,s.surface.width as _,s.surface.height as _);
 }
 env.framework_state.opengles.direct=Some(s);true
}

pub(super)fn main_renderbuffer_for_binding(env:&Environment,context:id,fbo:u32)->Option<u32>{env.framework_state.opengles.direct.as_ref().filter(|s|s.owner==context&&s.surface.framebuffer==fbo).map(|s|s.surface.renderbuffer)}
