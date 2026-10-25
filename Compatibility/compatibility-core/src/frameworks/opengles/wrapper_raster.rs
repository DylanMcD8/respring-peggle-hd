/* MPL-2.0. Guest-facing GL raster virtualization for the wrapper's main target. */
use crate::{Environment,objc::id,wrapper_resolution::{Size,Surface}};
use crate::gles::gles11_raw as gl;
use std::{collections::HashMap,rc::Rc,cell::RefCell};
#[derive(Default)]pub(super)struct Raster{pub surface:Option<Surface>,pub viewport:Option<[i32;4]>,pub scissor:Option<[i32;4]>}
pub(super)type Sizes=Rc<RefCell<HashMap<u32,Surface>>>;
fn current(env:&mut Environment)->Option<id>{*env.framework_state.opengles.current_ctx_for_thread(env.current_thread)}
pub(super)fn surface(env:&mut Environment)->Option<Surface>{if !crate::wrapper_resolution::virtualizes_raster(env.options.wrapper_resolution){return None;}let c=current(env)?;env.objc.borrow::<super::eagl::EAGLContextHostObject>(c).wrapper_raster.surface}
pub(super)fn query(env:&mut Environment,p:u32)->Option<[i32;4]>{if !crate::wrapper_resolution::virtualizes_raster(env.options.wrapper_resolution){return None;}let c=current(env)?;let r=&env.objc.borrow::<super::eagl::EAGLContextHostObject>(c).wrapper_raster;match p{gl::VIEWPORT=>r.viewport,gl::SCISSOR_BOX=>r.scissor,_=>None}}
pub(super)fn remember_rect(env:&mut Environment,p:u32,r:[i32;4]){if !crate::wrapper_resolution::virtualizes_raster(env.options.wrapper_resolution)||r[2]<0||r[3]<0{return;}if let Some(c)=current(env){let s=&mut env.objc.borrow_mut::<super::eagl::EAGLContextHostObject>(c).wrapper_raster;match p{gl::VIEWPORT=>s.viewport=Some(r),gl::SCISSOR_BOX=>s.scissor=Some(r),_=>{}}}}
pub(super)fn storage(env:&mut Environment,renderbuffer:u32,guest:Size,actual:Size){if !crate::wrapper_resolution::virtualizes_raster(env.options.wrapper_resolution){return;}if let Some(c)=current(env){env.objc.borrow_mut::<super::eagl::EAGLContextHostObject>(c).wrapper_sizes.borrow_mut().insert(renderbuffer,Surface{guest,actual});}}
pub(super)fn renderbuffer_size(env:&mut Environment,p:u32)->Option<Surface>{let c=current(env)?;let v=env.objc.borrow::<super::eagl::EAGLContextHostObject>(c).wrapper_sizes.borrow().get(&p).copied();v}
pub(super)fn storage_plan(env:&mut Environment,format:u32,guest:Size)->Option<Surface>{let c=current(env)?;if !crate::wrapper_resolution::virtualizes_raster(env.options.wrapper_resolution){return None;}let sizes=env.objc.borrow::<super::eagl::EAGLContextHostObject>(c).wrapper_sizes.borrow();let known=sizes.values().find(|s|s.guest==guest).copied();if matches!(format,gl::DEPTH_COMPONENT16_OES|0x8D48 /* STENCIL_INDEX8_OES */|gl::RGBA8_OES|gl::RGB565_OES){known}else{None}}
pub(super)fn refresh(env:&mut Environment){
 if !crate::wrapper_resolution::virtualizes_raster(env.options.wrapper_resolution){return;}let Some(c)=current(env)else{return;};
 let(fbo,kind,name)=super::gles_guest::with_ctx_and_mem(env,|g,_|unsafe{let(mut f,mut k,mut n)=(0,0,0);g.GetIntegerv(gl::FRAMEBUFFER_BINDING_OES,&mut f);if f!=0{g.GetFramebufferAttachmentParameterivOES(gl::FRAMEBUFFER_OES,gl::COLOR_ATTACHMENT0_OES,gl::FRAMEBUFFER_ATTACHMENT_OBJECT_TYPE_OES,&mut k);if k!=0{g.GetFramebufferAttachmentParameterivOES(gl::FRAMEBUFFER_OES,gl::COLOR_ATTACHMENT0_OES,gl::FRAMEBUFFER_ATTACHMENT_OBJECT_NAME_OES,&mut n);}}(f as u32,k as u32,n as u32)});
 let rbo=super::direct_texture::main_renderbuffer_for_binding(env,c,fbo).or((kind==gl::RENDERBUFFER_OES).then_some(name));
 let s=rbo.and_then(|r|renderbuffer_size(env,r));let r=&mut env.objc.borrow_mut::<super::eagl::EAGLContextHostObject>(c).wrapper_raster;r.surface=s;let(v,sc)=(r.viewport,r.scissor);
 super::gles_guest::with_ctx_and_mem(env,|g,_|unsafe{if let Some(v)=v{let v=s.and_then(|s|s.rect(v)).unwrap_or(v);g.Viewport(v[0],v[1],v[2],v[3]);}if let Some(sc)=sc{let sc=s.and_then(|s|s.rect(sc)).unwrap_or(sc);g.Scissor(sc[0],sc[1],sc[2],sc[3]);}});
}
pub(super)fn delete_sizes(env:&mut Environment,names:&[u32]){if let Some(c)=current(env){let mut sizes=env.objc.borrow_mut::<super::eagl::EAGLContextHostObject>(c).wrapper_sizes.borrow_mut();for n in names{sizes.remove(n);}}}
pub(super)fn with_logical_read<T,U:Default>(env:&mut Environment,command:T)->U where T:FnOnce(&mut dyn crate::gles::GLES,&mut crate::mem::Mem)->U{
 let Some(s)=surface(env).filter(|s|s.actual!=s.guest)else{return super::gles_guest::with_ctx_and_mem(env,command);};
 let Some((result,prior,errors))=super::gles_guest::with_ctx_and_mem(env,|g,mem|unsafe{let prior=crate::gles::direct_texture::drain_errors(g);let(result,errors)=crate::gles::wrapper_readback::with_logical(g,s,|g|command(g,mem));Some((result,prior,errors))})else{return U::default();};
 if let Some(c)=current(env){let q=&mut env.objc.borrow_mut::<super::eagl::EAGLContextHostObject>(c).pending_errors;crate::gles::gpu_transfer::enqueue_errors(q,prior);crate::gles::gpu_transfer::enqueue_errors(q,errors);}
 result.unwrap_or_else(|reason|{crate::diagnostics::event(&format!("{{\"event\":\"wrapper_logical_read_failed\",\"reason\":\"{reason}\"}}"));panic!("Wrapper logical read/copy failed: {reason}")})
}
