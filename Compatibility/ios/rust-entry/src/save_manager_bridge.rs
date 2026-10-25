//! Native UI boundary. The manager is consumed before the interpreter starts.
use super::save_manager::{Manager,GameLease};
use std::{ffi::{CStr,CString,c_char},path::{Path,PathBuf},sync::{Mutex,OnceLock}};
use serde_json::{json,Value};
#[derive(Default)]struct Session{manager:Option<Manager>,game:Option<GameLease>,documents:Option<PathBuf>}
static SESSION:OnceLock<Mutex<Session>>=OnceLock::new();
fn session()->&'static Mutex<Session>{SESSION.get_or_init(||Mutex::new(Session::default()))}
pub fn approved_game_root(documents:&Path)->Result<PathBuf,String>{
 let s=session().lock().map_err(|_|"Save session needs a restart")?;
 if s.documents.as_deref()!=Some(std::fs::canonicalize(documents).map_err(|e|e.to_string())?.as_path()){return Err("Save session does not match this application container".into());}
 s.game.as_ref().map(|g|g.data_root.clone()).ok_or_else(||"Continue from Save File Management before starting the game".into())
}
fn action(code:i32,value:&str,confirmed:bool)->Result<Value,String>{
 if code==20{if value.len()>1024*1024{return Err("Save file exceeds size limit".into());}return super::save_manager::progress::exchange(&serde_json::from_str(value).map_err(|_|"Invalid shared save file data")?);}
 let mut s=session().lock().map_err(|_|"Save file management needs a restart")?;
 if s.game.is_some(){return Err("Restart the app to manage save files. No save operations run while the game is active.".into());}
 if code==0{
  let documents=std::fs::canonicalize(value).map_err(|e|e.to_string())?;
  if let Some(old)=s.documents.as_ref(){if old!=&documents{return Err("Another application container is already selected".into());}}
  if s.manager.is_none(){s.manager=Some(Manager::open(&documents)?);s.documents=Some(documents);}
 }
 if code==5{
  let manager=s.manager.take().ok_or("Save File Management is not open")?;
  match manager.begin_game(){
   Ok(lease)=>{let root=lease.data_root.to_string_lossy().into_owned();s.game=Some(lease);return Ok(json!({"ok":true,"game_started":true,"data_root":root}));},
   Err(error)=>{if let Some(d)=s.documents.as_ref(){s.manager=Manager::open(d).ok();}return Err(error);}
  }
 }
 let m=s.manager.as_mut().ok_or("Save File Management is not open")?;
 match code{
  0|6=>Ok(json!({"ok":true,"can_rollback":m.can_rollback()?,"game_started":false})),
  1=>{let path=m.export()?;Ok(json!({"ok":true,"path":path.to_string_lossy(),"account_state_policy":"excluded"}))},
  2=>{let p=m.validate(Path::new(value))?;Ok(json!({"ok":true,"token":p.token,"profiles":p.profiles,"files":p.files,"bytes":p.bytes,"game":"Peggle HD 2.0.9","account_state_policy":"excluded","selection_id":p.selected_id}))},
  3=>{m.restore(value,confirmed)?;Ok(json!({"ok":true,"restored":true,"can_rollback":m.can_rollback()?}))},
  4=>{m.rollback(confirmed)?;Ok(json!({"ok":true,"rolled_back":true,"can_rollback":m.can_rollback()?}))},
  8=>{let args:Value=serde_json::from_str(value).map_err(|_|"Invalid file selection")?;let paths=args["paths"].as_array().ok_or("Missing file selection")?.iter().map(|v|v.as_str().map(PathBuf::from).ok_or("Invalid selected path")).collect::<Result<Vec<_>,_>>()?;m.preview_raw(&paths,args["replace_matching"].as_bool().unwrap_or(false))},
  9=>{m.restore_raw(value,confirmed)?;Ok(json!({"ok":true,"restored":true,"can_rollback":m.can_rollback()?}))},
  10=>m.export_progress(),
  11=>m.preview_progress_file(Path::new(value)),
  12=>{m.restore_raw(value,confirmed)?;Ok(json!({"ok":true,"merged":true,"can_rollback":m.can_rollback()?}))},
  13=>{if value.len()>512*1024{return Err("Save file exceeds size limit".into());}let v:Value=serde_json::from_str(value).map_err(|_|"Invalid save file")?;m.preview_progress(&v)},
  14=>{let path=m.export_folder()?;Ok(json!({"ok":true,"path":path.to_string_lossy()}))},
  15=>{let args:Value=serde_json::from_str(value).map_err(|_|"Invalid selection")?;let paths=args["paths"].as_array().ok_or("Missing selection")?.iter().map(|p|p.as_str().map(PathBuf::from).ok_or("Invalid path")).collect::<Result<Vec<_>,_>>()?;m.preview_merge_selection(&paths,args["archive"].as_bool().unwrap_or(false))},
  7=>{m.cancel_validation();Ok(json!({"ok":true,"cancelled_preview":true}))},
  _=>Err("Unsupported save operation".into())
 }
}
#[no_mangle]pub unsafe extern "C" fn peggle_save_manager_action(code:i32,value:*const c_char,confirmed:i32)->*mut c_char{
 let result=std::panic::catch_unwind(||{
  let value=if value.is_null(){""}else{CStr::from_ptr(value).to_str().map_err(|_|"Invalid save operation input")?};
  action(code,value,confirmed!=0)
 });
 let value=match result{Ok(Ok(v))=>v,Ok(Err(e))=>json!({"ok":false,"error":e}),Err(_)=>json!({"ok":false,"error":"Save operation stopped. Existing save data is retained; restart before retrying."})};
 CString::new(value.to_string()).unwrap().into_raw()
}
#[no_mangle]pub unsafe extern "C" fn peggle_save_manager_free(value:*mut c_char){if !value.is_null(){drop(CString::from_raw(value));}}

#[cfg(test)]mod tests{
 use super::*;
 fn ffi(code:i32,value:&str,yes:bool)->Value{let value=CString::new(value).unwrap();unsafe{let raw=peggle_save_manager_action(code,value.as_ptr(),yes as i32);assert!(!raw.is_null());let bytes=CStr::from_ptr(raw).to_bytes().to_vec();peggle_save_manager_free(raw);serde_json::from_slice(&bytes).unwrap()}}
 #[test]fn native_FFI_confirmation_preview_and_restart_boundary_use_actual_engine(){
  let root=std::env::temp_dir().join(format!("peggle-owned-bridge-test-{}",uuid::Uuid::new_v4().simple()));let docs=root.join("Documents");std::fs::create_dir_all(&docs).unwrap();
  let userdata=docs.join("touchHLE_sandbox").join(super::super::save_manager::GAME).join("Documents/userdata");std::fs::create_dir_all(&userdata).unwrap();let mut index=Vec::from(1u32.to_le_bytes());index.extend_from_slice(&1u16.to_le_bytes());index.extend_from_slice(&5u16.to_le_bytes());index.extend_from_slice(b"Owned");index.extend_from_slice(&1u32.to_le_bytes());index.extend_from_slice(&1u32.to_le_bytes());std::fs::write(userdata.join("users.dat"),index).unwrap();let mut details=vec![0;494];details[..4].copy_from_slice(&25u32.to_le_bytes());std::fs::write(userdata.join("user1.dat"),details).unwrap();
  assert!(approved_game_root(&docs).is_err());assert_eq!(ffi(0,docs.to_str().unwrap(),false)["ok"],true);assert_eq!(ffi(2,"/owned-missing-backup.zip",false)["ok"],false);
  let out=ffi(1,"",false);assert_eq!(out["ok"],true);let preview=ffi(2,out["path"].as_str().unwrap(),false);assert_eq!(preview["profiles"],1);assert_eq!(preview["files"],2);let token=preview["token"].as_str().unwrap();
  assert_eq!(ffi(3,token,false)["ok"],false);assert_eq!(ffi(6,"",false)["can_rollback"],false);assert_eq!(ffi(3,token,true)["ok"],true);assert_eq!(ffi(4,"",false)["ok"],false);assert_eq!(ffi(4,"",true)["ok"],true);
  let raw_source=root.join("highscores.dat");let mut score_bytes=Vec::from(3u32.to_le_bytes());score_bytes.extend_from_slice(&[0;8]);std::fs::write(&raw_source,score_bytes).unwrap();
  let selection=json!({"paths":[raw_source.to_string_lossy()],"replace_matching":false}).to_string();let raw=ffi(8,&selection,false);assert_eq!(raw["can_import"],true);let token=raw["token"].as_str().unwrap();assert_eq!(ffi(9,token,false)["ok"],false);assert_eq!(ffi(9,token,true)["ok"],true);assert_eq!(ffi(4,"",true)["ok"],true);
  assert!(approved_game_root(&docs).is_err());let ready=ffi(5,"",false);assert_eq!(ready["game_started"],true);assert_eq!(approved_game_root(&docs).unwrap(),std::fs::canonicalize(&docs).unwrap());
  for operation in [0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15]{let denied=ffi(operation,docs.to_str().unwrap(),true);assert_eq!(denied["ok"],false);assert!(denied["error"].as_str().unwrap().contains("Restart"));}
  assert!(super::super::save_manager::Manager::open(&docs).is_err());unsafe{peggle_save_manager_free(std::ptr::null_mut());}
  std::fs::remove_dir_all(root).unwrap();
  println!("C50_NATIVE_SAVE_FFI export_validate_confirm_restore_rollback=true begin_game_consumes_manager=true ten_live_operations_rejected=true file_lease_enforced=true real_saves_or_original_game_not_used=true");
 }
}

// Called exclusively from the interpreter's guarded safe-point service.
#[no_mangle]pub extern "C" fn peggle_progress_capture(profile_id:u32)->*mut c_char{
 let result=std::panic::catch_unwind(||{let s=session().lock().map_err(|_|"Save session unavailable".to_string())?;s.game.as_ref().ok_or("Game not running".to_string())?.progress_snapshot(profile_id)});
 let v=match result{Ok(Ok(v))=>v,_=>return std::ptr::null_mut()};CString::new(v.to_string()).unwrap().into_raw()
}
pub fn finish_game_for_reload()->Result<(),String>{let mut s=session().lock().map_err(|_|"Save session unavailable")?;s.game.take();let docs=s.documents.clone().ok_or("Missing save container")?;s.manager=Some(Manager::open(&docs)?);Ok(())}
