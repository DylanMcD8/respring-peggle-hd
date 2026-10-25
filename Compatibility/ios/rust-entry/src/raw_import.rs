//! Loose save files and folders. Preview is memory-only; publication uses a new
//! full guest slot and retains the previous slot. Never invent profile IDs/names.
use super::*;
use std::os::unix::fs::MetadataExt;
#[derive(Clone,Debug,PartialEq)]
pub(super) struct Entry {pub(super) name:String,sequence:u32,pub(super) id:u32}
pub(super) fn index(b:&[u8])->Result<Vec<Entry>> {
 let mut c=Cursor::new(b);if c.u32()?!=1{return Err("Unsupported in-game user index version".into());}let n=c.u16()?as usize;if n==0||n>1000{return Err("Invalid in-game user count".into());}
 let(mut out,mut names,mut ids)=(Vec::new(),HashSet::new(),HashSet::new());
 for _ in 0..n{let name=c.string(false)?;let sequence=c.u32()?;let id=c.u32()?;if name.len()>1024||id==0||id>i32::MAX as u32||!names.insert(name.to_lowercase())||!ids.insert(id){return Err("Duplicate or invalid in-game user identity".into());}out.push(Entry{name,sequence,id});}c.end()?;Ok(out)
}
fn encode_index(entries:&[Entry])->Result<Vec<u8>>{if entries.is_empty()||entries.len()>1000{return Err("Invalid combined in-game user count".into());}let mut b=Vec::from(1u32.to_le_bytes());b.extend_from_slice(&(entries.len()as u16).to_le_bytes());for e in entries{b.extend_from_slice(&(e.name.len()as u16).to_le_bytes());b.extend_from_slice(e.name.as_bytes());b.extend_from_slice(&e.sequence.to_le_bytes());b.extend_from_slice(&e.id.to_le_bytes());}index(&b)?;Ok(b)}
struct Cursor<'a>{b:&'a[u8],n:usize}
impl<'a> Cursor<'a>{
 fn new(b:&'a[u8])->Self{Self{b,n:0}}
 fn take(&mut self,n:usize)->Result<&'a[u8]>{let end=self.n.checked_add(n).ok_or("Size overflow")?;let out=self.b.get(self.n..end).ok_or("Truncated save contents")?;self.n=end;Ok(out)}
 fn u8(&mut self)->Result<u8>{Ok(self.take(1)?[0])}fn u16(&mut self)->Result<u16>{Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))}fn u32(&mut self)->Result<u32>{Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))}
 fn count(&mut self)->Result<usize>{let n=self.u32()?as usize;if n>MAX_FILES{return Err("Save container count exceeds limit".into());}Ok(n)}
 fn string(&mut self,empty:bool)->Result<String>{let n=self.u16()?as usize;if n>4096||(!empty&&n==0){return Err("Invalid save string length".into());}let s=std::str::from_utf8(self.take(n)?).map_err(|_|"Unsupported save text encoding")?;if s.chars().any(|c|c.is_control()){return Err("Invalid save text".into());}Ok(s.into())}
 fn end(&self)->Result<()>{if self.n!=self.b.len(){Err("Unexpected trailing save contents".into())}else{Ok(())}}
}
fn map_scores(c:&mut Cursor)->Result<()>{let n=c.count()?;let mut keys=HashSet::new();let mut total=0usize;for _ in 0..n{let key=c.string(false)?;if !keys.insert(key.to_lowercase()){return Err("Duplicate score table".into());}let count=c.count()?;total=total.checked_add(count).ok_or("Score count overflow")?;if total>MAX_FILES{return Err("Too many score entries".into());}for _ in 0..count{c.string(true)?;c.u32()?;}}Ok(())}
fn map_level_stats(c:&mut Cursor)->Result<()>{let n=c.count()?;let mut keys=HashSet::new();for _ in 0..n{let k=c.string(false)?;if !keys.insert(k.to_lowercase()){return Err("Duplicate level statistics".into());}c.take(44)?;}Ok(())}
fn board_header(b:&[u8])->Result<()>{let mut c=Cursor::new(b);if c.u32()?!=87{return Err("Unsupported board version; expected87".into());}if c.u8()?>1{return Err("Invalid board flag".into());}c.string(true)?;c.take(20)?;if b.len()<64{return Err("Truncated board state".into());}Ok(())}
/// Structural checks, not proof of source identity or coherent gameplay state.
pub fn inspect_payload(path:&str,b:&[u8])->Result<&'static str>{
 if !allowed(path)||b.len()as u64>MAX_FILE{return Err("Unsupported path or oversized file".into());}
 if path=="userdata/users.dat"{index(b)?;return Ok("In-game user list checked");}
 if profile_id(path).is_some(){if b.len()<494||b.get(..4)!=Some(25u32.to_le_bytes().as_slice()){return Err("Unsupported or truncated in-game user details; expected25".into());}return Ok("In-game user file format checked; load the imported save files in Peggle to verify them");}
 let mut c=Cursor::new(b);
 if path=="userdata/highscores.dat"{let v=c.u32()?;if !(1..=3).contains(&v){return Err("Unsupported score format".into());}for _ in 1..v{map_scores(&mut c)?;}c.end()?;return Ok("Score tables checked");}
 if path.starts_with("userdata/stat_"){let v=c.u32()?;if !(1..=3).contains(&v){return Err("Unsupported statistics format".into());}c.take(4)?;c.string(true)?;c.take(4+12*4+16*8)?;map_level_stats(&mut c)?;if v>=2{let n=c.count()?;let mut ids=HashSet::new();for _ in 0..n{if !ids.insert(c.u32()?){return Err("Duplicate trophy statistics".into());}c.take(8)?;}}if v>=3{c.take(12*4+16*8)?;map_level_stats(&mut c)?;}c.end()?;return Ok("Statistics tables checked");}
 if path.starts_with("userdata/replays/"){if c.u32()?!=87{return Err("Unsupported replay version; expected87".into());}c.string(true)?;c.u32()?;let n=c.u16()?as usize;if n==0||n>1024{return Err("Invalid replay chunk count".into());}for _ in 0..n{let size=c.u32()?as usize;if size<64||size as u64>MAX_FILE{return Err("Invalid replay chunk size".into());}board_header(c.take(size)?)?;}c.end()?;return Ok("Replay format checked; loading still needs testing in game");}
 if board_id(path).is_some(){board_header(b)?;return Ok("Board format checked; loading still needs testing in game");}
 Err("No content validator for this file".into())
}
fn profile_id(p:&str)->Option<u32>{p.strip_prefix("userdata/user")?.strip_suffix(".dat")?.parse().ok()}
fn board_id(p:&str)->Option<u32>{for prefix in ["stage","stagenights","arcade","trophynights","trophy","duel"]{if let Some(n)=p.strip_prefix(&format!("userdata/{prefix}")).and_then(|s|s.strip_suffix(".sav")).and_then(|s|s.parse().ok()){return Some(n);}}None}
fn canonical(name:&str)->Option<String>{
 if !name.is_ascii()||name.len()>192||name.contains(['/', '\\',':','\0']){return None;}let lower=name.to_ascii_lowercase();
 let p=if lower.starts_with("replay"){format!("userdata/replays/Replay{}",&lower[6..])}else if lower.starts_with("stat_")&&lower.ends_with(".dat"){let stem=name.get(..name.len().checked_sub(4)?)?;format!("userdata/stat_{}.dat",&stem[5..])}else{format!("userdata/{lower}")};allowed(&p).then_some(p)
}
fn row(path:&str,reason:&str)->Value{json!({"path":path,"reason":reason})}
#[derive(Default)]struct Scan{files:BTreeMap<String,Vec<u8>>,skipped:Vec<Value>,conflicts:Vec<Value>,blocked:HashSet<String>,count:usize,bytes:u64}
impl Scan{
 fn check_metadata(&mut self,meta:&Path)->Result<()>{
  no_link(meta)?;if !meta.is_file()||meta.metadata().map_err(err)?.nlink()!=1{return Err("Source metadata must be regular without links".into());}
  let b=read(meta,MAX_META)?;self.bytes=self.bytes.checked_add(b.len()as u64).ok_or("Metadata size overflow")?;if self.bytes>MAX_TOTAL{return Err("Selected metadata exceeds total size limit".into());}
  let identity=if meta.file_name().and_then(|s|s.to_str())==Some("Info.plist"){let v=plist::Value::from_reader(std::io::Cursor::new(b)).map_err(err)?;v.as_dictionary().and_then(|d|d.get("CFBundleIdentifier")).and_then(|v|v.as_string()).map(str::to_owned)}else{let v:Value=serde_json::from_slice(&b).map_err(err)?;v["game"].as_str().map(str::to_owned)};
  if identity.as_deref().is_some_and(|game|game!=GAME){return Err("Selected metadata identifies a foreign game; not imported".into());}Ok(())
 }
 fn add(&mut self,name:&str,b:Vec<u8>)->Result<()>{self.count+=1;if self.count>MAX_FILES{return Err("Too many selected entries".into());}self.bytes=self.bytes.checked_add(b.len()as u64).ok_or("Input size overflow")?;if self.bytes>MAX_TOTAL{return Err("Selection exceeds total size limit".into());}
  let Some(path)=canonical(name)else{self.skipped.push(row(name,"Unrecognized Peggle filename"));return Ok(());};
  if let Err(e)=inspect_payload(&path,&b){self.skipped.push(row(&path,&format!("Rejected contents: {e}")));if path=="userdata/users.dat"{self.blocked.insert(path);}return Ok(());}
  let key=path.to_ascii_lowercase();if self.blocked.contains(&key){self.conflicts.push(row(&path,"Conflicting selected copies; choose one source"));return Ok(());}
  if let Some(existing)=self.files.keys().find(|p|p.to_ascii_lowercase()==key).cloned(){if self.files[&existing]==b{self.skipped.push(row(&path,"Identical selected duplicate"));}else{self.files.remove(&existing);self.blocked.insert(key);self.conflicts.push(row(&path,"Selected copies differ; neither will be imported"));}}else{self.files.insert(path,b);}Ok(())
 }
 fn walk(&mut self,p:&Path,depth:usize)->Result<()>{if depth>4{return Err("Selected folder nesting exceeds limit".into());}no_link(p)?;directory(p)?;
  for name in ["Info.plist","manifest.json"]{let meta=p.join(name);no_link(&meta)?;if meta.exists(){self.check_metadata(&meta)?;}}
  for e in fs::read_dir(p).map_err(err)?{let p=e.map_err(err)?.path();no_link(&p)?;self.count+=1;if self.count>MAX_FILES{return Err("Too many selected entries".into());}
  let name=p.file_name().and_then(|x|x.to_str()).ok_or("Unsupported selected filename")?;
  if p.is_dir(){if matches!(name.to_ascii_lowercase().as_str(),"userdata"|"documents"|"replays"){self.walk(&p,depth+1)?;}else{self.skipped.push(row(name,"Unrecognized folder; not traversed"));}continue;}
  if !p.is_file(){return Err("Only regular selected files are supported".into());}if p.metadata().map_err(err)?.nlink()!=1{return Err("Hard-linked source files are not supported".into());}if name=="game_center.dat"||name==".DS_Store"||name.starts_with("._"){self.skipped.push(row(name,"Account or Mac metadata excluded"));continue;}
  if canonical(name).is_none(){self.skipped.push(row(name,"Unrecognized filename; not read"));continue;}if p.metadata().map_err(err)?.len()>MAX_FILE{self.skipped.push(row(name,"File exceeds16MiB limit; not read"));continue;}let b=read(&p,MAX_FILE)?;self.count-=1;self.add(name,b)?;
 }Ok(())}
}
pub(super) struct RawValidated{pub(super) guest_files:BTreeMap<String,Vec<u8>>,pub(super) dirs:Vec<String>,pub(super) state:Value,pub(super) previous_fingerprint:Value}
pub(super) fn selected_path(p:&Path)->Result<PathBuf>{
 if !p.is_absolute()||p.components().any(|c|matches!(c,std::path::Component::ParentDir)){return Err("Unsafe selected path".into());}
 for ancestor in p.ancestors(){
  // Apple's public file URLs may use these fixed OS aliases. User-controlled
  // links remain rejected before canonicalization, including a selected leaf.
  let alias=match ancestor.to_str(){Some("/var")=>Some("/private/var"),Some("/tmp")=>Some("/private/tmp"),_=>None};
  if alias.is_some_and(|target|fs::symlink_metadata(ancestor).is_ok_and(|m|m.file_type().is_symlink())&&fs::canonicalize(ancestor).is_ok_and(|p|p==Path::new(target))){continue;}
  no_link(ancestor)?;
 }
 fs::canonicalize(p).map_err(err)
}
pub(super) fn snapshot_guest(guest:&Path)->Result<(BTreeMap<String,Vec<u8>>,Vec<String>)>{
 no_link(guest)?;if !guest.exists(){return Ok((BTreeMap::new(),Vec::new()));}directory(guest)?;
 fn walk(root:&Path,p:&Path,files:&mut BTreeMap<String,Vec<u8>>,dirs:&mut Vec<String>,bytes:&mut u64,count:&mut usize,depth:usize)->Result<()>{if depth>16{return Err("Current guest nesting exceeds limit".into());}for e in fs::read_dir(p).map_err(err)?{let p=e.map_err(err)?.path();no_link(&p)?;*count+=1;if *count>MAX_FILES{return Err("Current guest has too many entries".into());}let rel=p.strip_prefix(root).map_err(err)?.to_str().ok_or("Unsupported current filename")?.to_owned();if p.is_dir(){dirs.push(rel);walk(root,&p,files,dirs,bytes,count,depth+1)?;}else{let b=read(&p,MAX_FILE)?;*bytes+=b.len()as u64;if *bytes>MAX_TOTAL{return Err("Current guest exceeds size limit".into());}files.insert(rel,b);}}Ok(())}
 let(mut files,mut dirs,mut bytes,mut count)=(BTreeMap::new(),Vec::new(),0,0);walk(guest,guest,&mut files,&mut dirs,&mut bytes,&mut count,0)?;Ok((files,dirs))
}
impl Manager{
 pub fn preview_raw(&mut self,paths:&[PathBuf],replace:bool)->Result<Value>{
  self.pending.clear();self.raw_pending.clear();if paths.is_empty()||paths.len()>128{return Err("Select1–128 files or folders".into());}
  let mut scan=Scan::default();for original in paths{let p=selected_path(original)?;if p.is_dir(){scan.walk(&p,0)?;}else{if !p.is_file()||p.metadata().map_err(err)?.nlink()!=1{return Err("Only regular source files without hard links are supported".into());}let name=p.file_name().and_then(|x|x.to_str()).ok_or("Unsupported filename")?;if matches!(name,"Info.plist"|"manifest.json"){scan.check_metadata(&p)?;scan.skipped.push(row(name,"Source metadata checked but not imported"));continue;}if name.to_ascii_lowercase().ends_with(".zip"){scan.skipped.push(row(name,"Use Restore Backup ZIP for this archive; other selected files are checked"));continue;}if canonical(name).is_none(){scan.skipped.push(row(name,"Unrecognized filename; not read"));continue;}if p.metadata().map_err(err)?.len()>MAX_FILE{scan.skipped.push(row(name,"File exceeds16MiB limit; not read"));continue;}scan.add(name,read(&p,MAX_FILE)?)?;}}
  let state=self.state()?;let guest=self.active_guest(&state)?;let fingerprint=self.fingerprint(&guest)?;let(mut final_files,dirs)=snapshot_guest(&guest)?;if self.fingerprint(&guest)?!=fingerprint{return Err("Current save files changed during preview".into());}
  let current_index=final_files.get("Documents/userdata/users.dat").map(|b|index(b)).transpose()?;let mut entries=current_index.clone().unwrap_or_default();
  if current_index.is_some(){let game:BTreeMap<_,_>=final_files.iter().filter_map(|(p,b)|p.strip_prefix("Documents/").filter(|p|allowed(p)).map(|p|(p.to_owned(),b.clone()))).collect();profiles(&game)?;}
  let incoming=scan.files.get("userdata/users.dat").map(|b|index(b)).transpose()?;let invalid_index=scan.blocked.contains("userdata/users.dat");let mut missing=Vec::new();let mut accepted=Vec::new();let mut notes=vec!["The original game/version is unknown. The game still needs to load these files.".to_owned(),"Unselected current files and local preferences are retained; rollback keeps the previous set.".to_owned()];let mut forbidden_ids=HashSet::new();let mut matched_notes=HashSet::new();let mut added_ids=HashSet::new();
  if let Some(source)=incoming.as_ref(){for e in source{let collision=entries.iter().find(|x|x.id==e.id||x.name.to_lowercase()==e.name.to_lowercase());if collision.is_some_and(|x|x.id!=e.id||x.name!=e.name){forbidden_ids.insert(e.id);scan.conflicts.push(row(&format!("userdata/user{}.dat",e.id),"In-game user ID/name conflicts with current identity; not merged or renamed"));continue;}
    let detail=format!("userdata/user{}.dat",e.id);if !scan.files.contains_key(&detail)&&collision.is_none(){missing.push(row(&detail,"users.dat needs this matching detail file for a new in-game user"));continue;}
    if collision.is_none(){entries.push(e.clone());added_ids.insert(e.id);}
  }}
  let mut projected_bytes:u64=final_files.values().map(|b|b.len()as u64).sum();
  // A supplied but rejected/ambiguous index cannot establish source identities.
  for(path,b)in &scan.files{if path=="userdata/users.dat"{continue;}if path.starts_with("userdata/stat_")&&!forbidden_ids.is_empty(){scan.conflicts.push(row(path,"Source in-game user identities conflict; statistics ownership is ambiguous"));continue;}
   let pid=profile_id(path).or_else(||board_id(path));if let Some(pid)=pid{
    if invalid_index||forbidden_ids.contains(&pid){scan.conflicts.push(row(path,"Source in-game user identity is conflicting or invalid"));continue;}
    let indexed=entries.iter().any(|e|e.id==pid);if !indexed{missing.push(row(path,"Needs users.dat plus matching user<ID>.dat, or an existing indexed in-game user"));continue;}
    if incoming.as_ref().is_some_and(|src|!src.iter().any(|e|e.id==pid))&&profile_id(path).is_some(){missing.push(row(path,"Detail is not referenced by the supplied in-game user index"));continue;}
    if incoming.is_none()&&matched_notes.insert(pid){let name=&entries.iter().find(|e|e.id==pid).unwrap().name;notes.push(format!("Selected files will match existing in-game user ID{pid} ({name}); source identity cannot be independently checked."));}
   }
   let target=format!("Documents/{path}");if let Some(old)=final_files.get(&target){if old==b{scan.skipped.push(row(path,"Already identical in current set"));continue;}if !replace{scan.conflicts.push(row(path,"Existing file differs; enable Replace matching files to include it"));continue;}}
   projected_bytes=projected_bytes.checked_sub(final_files.get(&target).map(|b|b.len()as u64).unwrap_or(0)).and_then(|n|n.checked_add(b.len()as u64)).ok_or("Result size overflow")?;if projected_bytes>MAX_TOTAL{return Err("Resulting guest exceeds size limit".into());}
   final_files.insert(target,b.clone());accepted.push(json!({"path":path,"bytes":b.len(),"action":if current_index.is_some()&&!added_ids.contains(&pid.unwrap_or(0)){"add or replace after confirmation"}else{"add"},"validation":inspect_payload(path,b)?}));
  }
  // Only complete, nonconflicting new identities are appended. No ID remapping.
  let accepted_ids:HashSet<u32>=accepted.iter().filter_map(|v|v["path"].as_str().and_then(profile_id)).collect();entries.retain(|e|!added_ids.contains(&e.id)||accepted_ids.contains(&e.id));
  if !entries.is_empty(){let combined=encode_index(&entries)?;let key="Documents/userdata/users.dat";if final_files.get(key)!=Some(&combined){final_files.insert(key.into(),combined.clone());accepted.push(json!({"path":"userdata/users.dat","bytes":combined.len(),"action":"combine only complete compatible in-game user groups; keep existing identities","validation":"In-game user list updated from complete selected in-game users; existing in-game users kept"}));}}
  if incoming.is_some()&&accepted.iter().all(|v|v["path"]!="userdata/users.dat"){scan.skipped.push(row("userdata/users.dat","Current index retained; no new complete identity to add"));}
  let pref_path=format!("Library/Preferences/{GAME}.plist");let mut prefs=match final_files.get(&pref_path){Some(b)=>plist::Value::from_reader(std::io::Cursor::new(b)).map_err(err)?.into_dictionary().ok_or("Invalid current preference format")?,None=>plist::Dictionary::new()};
  if let Some(value)=prefs.get("CurUser"){let name=value.as_string().ok_or("Invalid current in-game user preference")?;if !entries.iter().any(|e|e.name==name){return Err("Current in-game user preference is not indexed; existing data retained".into());}}
  let selected=if entries.is_empty(){None}else{let current=prefs.get("CurUser").and_then(|v|v.as_string()).and_then(|n|entries.iter().find(|e|e.name==n));Some(current.unwrap_or(&entries[0]).clone())};
  if let Some(e)=selected.as_ref(){if prefs.get("CurUser").and_then(|v|v.as_string())!=Some(e.name.as_str()){notes.push(format!("No usable current selection: use first indexed in-game user ID{}; no source preference was supplied.",e.id));prefs.insert("CurUser".into(),plist::Value::String(e.name.clone()));let mut b=Vec::new();plist::Value::Dictionary(prefs).to_writer_xml(&mut b).map_err(err)?;final_files.insert(pref_path,b);}}
  notes.push(match selected.as_ref(){Some(e)=>format!("After import: {} in-game users; active in-game user ID{} ({}).",entries.len(),e.id,e.name),None=>"No in-game user identity is created. The game can ask for a name after startup.".into()});
  let total:u64=final_files.values().map(|b|b.len()as u64).sum();let mut result_dirs:std::collections::BTreeSet<PathBuf>=dirs.iter().map(PathBuf::from).collect();for file in final_files.keys(){let mut p=Path::new(file).parent();while let Some(dir)=p{if !dir.as_os_str().is_empty(){result_dirs.insert(dir.to_owned());}p=dir.parent();}}for dir in ["Documents","Documents/userdata","Library","Library/Preferences","tmp"]{result_dirs.insert(PathBuf::from(dir));}if total>MAX_TOTAL||final_files.len()+result_dirs.len()>MAX_FILES{return Err("Resulting guest exceeds supported limits".into());}
  let token=id();let result=json!({"ok":true,"token":token,"accepted":accepted,"skipped":scan.skipped,"conflicts":scan.conflicts,"missing_dependencies":missing,"notes":notes,"profiles_after":entries.len(),"selected_profile_id":selected.map(|e|e.id),"files_to_import":accepted.len(),"replace_matching_files":replace,"can_import":!accepted.is_empty(),"source_provenance":"unconfirmed","previous_set_retained":true});
  if !accepted.is_empty(){self.raw_pending.insert(token,RawValidated{guest_files:final_files,dirs,state,previous_fingerprint:fingerprint});}Ok(result)
 }
 pub fn restore_raw(&mut self,token:&str,confirmed:bool)->Result<()>{self.restore_raw_inner(token,confirmed,0)}
 fn restore_raw_inner(&mut self,token:&str,confirmed:bool,fail:u8)->Result<()>{
  if !confirmed{return Err("Import requires explicit confirmation".into());}let v=self.raw_pending.remove(token).ok_or("Preview this selection before importing")?;if self.state()?!=v.state{return Err("Current selection changed; preview again".into());}let old_guest=self.active_guest(&v.state)?;if self.fingerprint(&old_guest)?!=v.previous_fingerprint{return Err("Current data changed after preview; import not applied".into());}if fail==1{return Err("Injected interruption before staging".into());}
  let slot=id();let data=self.base.join("slots").join(&slot);fs::create_dir(&data).map_err(err)?;let guest=Self::guest(&data);fs::create_dir_all(&guest).map_err(err)?;
  for dir in v.dirs.iter().map(String::as_str).chain(["Documents/userdata","Library/Preferences","tmp"]){fs::create_dir_all(guest.join(dir)).map_err(err)?;}
  for(p,b)in &v.guest_files{let p=guest.join(p);fs::create_dir_all(p.parent().unwrap()).map_err(err)?;write_new(&p,b)?;}
  if snapshot_guest(&guest)?.0!=v.guest_files{return Err("Staged import verification failed".into());}if fail==2{return Err("Injected interruption after staging".into());}
  write_new(&data.join("SlotComplete.json"),&serde_json::to_vec(&json!({"complete":true,"game":GAME,"version":GAME_VERSION,"raw_import":true})).map_err(err)?)?;sync_dir(&data)?;
  let recovery=id();write_new(&self.base.join("recovery").join(format!("{recovery}.json")),&serde_json::to_vec(&json!({"game":GAME,"state":v.state,"files":v.previous_fingerprint})).map_err(err)?)?;sync_dir(&self.base.join("recovery"))?;if fail==3{return Err("Injected interruption after recovery receipt".into());}
  if self.fingerprint(&old_guest)?!=v.previous_fingerprint{return Err("Current save files changed before import; old selection retained".into());}self.publish(&v.state,&slot,v.state["active"].as_str().unwrap(),&recovery)
 }
}

#[cfg(test)]#[path="raw_import_tests.rs"]mod tests;

// Read-only source scan shared by reviewed overwrite and additive progress import.
pub(super) fn progress_inputs(paths:&[PathBuf])->Result<BTreeMap<String,Vec<u8>>>{
 if paths.is_empty()||paths.len()>128{return Err("Select 1–128 files or one folder".into());}
  let mut scan=Scan::default();for original in paths{let p=selected_path(original)?;if p.is_dir(){scan.walk(&p,0)?;}else{if !p.is_file()||p.metadata().map_err(err)?.nlink()!=1{return Err("Only regular source files without hard links are supported".into());}let name=p.file_name().and_then(|x|x.to_str()).ok_or("Unsupported filename")?;if matches!(name,"Info.plist"|"manifest.json"){scan.check_metadata(&p)?;scan.skipped.push(row(name,"Source metadata checked but not imported"));continue;}if name.to_ascii_lowercase().ends_with(".zip"){scan.skipped.push(row(name,"Use Restore Backup ZIP for this archive; other selected files are checked"));continue;}if canonical(name).is_none(){scan.skipped.push(row(name,"Unrecognized filename; not read"));continue;}if p.metadata().map_err(err)?.len()>MAX_FILE{scan.skipped.push(row(name,"File exceeds16MiB limit; not read"));continue;}scan.add(name,read(&p,MAX_FILE)?)?;}}
 if !scan.conflicts.is_empty()||!scan.blocked.is_empty(){return Err("Selected files contain conflicting copies; choose one source in-game user".into());}
 if scan.skipped.iter().any(|v|v["reason"].as_str().is_some_and(|s|s.starts_with("Rejected contents")||s.contains("limit"))){return Err("Some selected save files are invalid or oversized; choose a valid source".into());}
 Ok(scan.files)
}
