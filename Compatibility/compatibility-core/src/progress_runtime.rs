//! Host sync service at the main guest run-loop boundary. No save mutation
//! occurs here. Reload yields once, then drops the entire Environment.
use crate::Environment;
use std::sync::atomic::{AtomicBool,Ordering};
static RELOAD_READY:AtomicBool=AtomicBool::new(false);
pub fn reload_ready()->bool{RELOAD_READY.load(Ordering::SeqCst)}
pub fn take_reload()->bool{RELOAD_READY.swap(false,Ordering::SeqCst)}
#[cfg(target_os="ios")]
extern "C" {fn peggle_auto_capture_due()->u32;fn peggle_auto_profile()->u32;fn peggle_progress_capture(id:u32)->*mut std::ffi::c_char;fn peggle_auto_snapshot(json:*const std::ffi::c_char);fn peggle_save_manager_free(json:*mut std::ffi::c_char);fn peggle_auto_prompt_due()->i32;fn peggle_auto_prompt()->i32;}
pub fn snapshot(env:&mut Environment,force:bool){
 #[cfg(target_os="ios")]{
  if crate::libc::posix_io::progress_writer_open(env){return;}
  env.on_parent_stack_in_coroutine(|_,_|unsafe{let id=if force{peggle_auto_profile()}else{peggle_auto_capture_due()};if id!=0{let json=peggle_progress_capture(id);if !json.is_null(){peggle_auto_snapshot(json);peggle_save_manager_free(json);}}});
 }
}
pub fn service(env:&mut Environment){
 #[cfg(target_os="ios")]{
  snapshot(env,false);
  if env.on_parent_stack_in_coroutine(|_,_|unsafe{peggle_auto_prompt_due()!=0}){crate::frameworks::uikit::ui_application::pause_for_progress(env);}
 }
}
pub fn prompt()->bool{
 #[cfg(target_os="ios")]unsafe{return peggle_auto_prompt()!=0;}
 #[cfg(not(target_os="ios"))]false
}
pub fn finish_reload(env:&mut Environment)->!{
 RELOAD_READY.store(true,Ordering::SeqCst);
 env.yield_thread(crate::environment::ThreadBlock::NotBlocked);
 unreachable!("The old guest must never run after requesting reload")
}
