//! Pre-launch, game-only save archives and recoverable directory-slot selection.
//! No guest execution, network, live-game copying, profile merging or deletion.
use std::{collections::{BTreeMap,HashSet},fs::{self,File,OpenOptions},io::{Read,Write},path::{Path,PathBuf},os::{fd::AsRawFd,unix::fs::OpenOptionsExt}};
use serde_json::{json,Value};
pub type Result<T> = std::result::Result<T,String>;
pub const GAME:&str="com.popcap.ios.PeggleHD";
pub const GAME_VERSION:&str="2.0.9";
pub const GAME_SHA:&str="34bfa363268126101e271e10f5cf8b2623e1c01128185d90d26fce074f67ea57";
pub const MAX_FILE:u64=16*1024*1024;
pub const MAX_TOTAL:u64=128*1024*1024;
pub const MAX_FILES:usize=4096;
const MAX_META:u64=2*1024*1024;
fn err(e:impl std::fmt::Display)->String{e.to_string()}
fn id()->String{uuid::Uuid::new_v4().simple().to_string()}
#[link(name="System")]
extern "C" {fn CC_SHA256(data:*const std::ffi::c_void,len:u32,out:*mut u8)->*mut u8;}
pub fn hash(bytes:&[u8])->String{
    assert!(bytes.len()<=u32::MAX as usize);
    let mut out=[0u8;32];unsafe{CC_SHA256(bytes.as_ptr().cast(),bytes.len()as u32,out.as_mut_ptr());}
    out.iter().map(|b|format!("{b:02x}")).collect()
}
fn no_link(p:&Path)->Result<()> {
    if fs::symlink_metadata(p).is_ok_and(|m|m.file_type().is_symlink()){return Err("Symbolic links are not supported".into());}Ok(())
}
fn directory(p:&Path)->Result<()> {no_link(p)?;if !p.is_dir(){return Err("Required save directory is missing".into());}Ok(())}
fn child_dir(p:&Path)->Result<()> {no_link(p)?;if !p.exists(){fs::create_dir(p).map_err(err)?;}directory(p)}
fn read(p:&Path,limit:u64)->Result<Vec<u8>>{
    no_link(p)?;let f=OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW).open(p).map_err(err)?;
    let m=f.metadata().map_err(err)?;if !m.is_file()||m.len()>limit{return Err("Save input exceeds the supported size or is not a regular file".into());}
    let mut b=Vec::new();f.take(limit+1).read_to_end(&mut b).map_err(err)?;
    if b.len()as u64!=m.len()||b.len()as u64>limit{return Err("Save input changed while being read".into());}Ok(b)
}
fn write_new(p:&Path,b:&[u8])->Result<()> {
    let mut f=OpenOptions::new().write(true).create_new(true).mode(0o600).custom_flags(libc::O_NOFOLLOW).open(p).map_err(err)?;
    f.write_all(b).map_err(err)?;f.sync_all().map_err(err)
}
fn sync_dir(p:&Path)->Result<()>{File::open(p).and_then(|f|f.sync_all()).map_err(err)}
fn valid_id(s:&str)->bool{s.len()==32&&s.bytes().all(|c|c.is_ascii_hexdigit()&&!c.is_ascii_uppercase())}
fn digits(s:&str,zero:bool)->bool{!s.is_empty()&&s.bytes().all(|c|c.is_ascii_digit())&&((zero&&s=="0")||!s.starts_with('0'))&&s.parse::<i32>().is_ok()}
fn allowed(p:&str)->bool {
    if p.len()>192||!p.is_ascii()||p.contains('\\')||p.contains(':')||p.contains('\0')||p.split('/').any(|x|x.is_empty()||x=="."||x==".."){return false;}
    let Some(name)=p.strip_prefix("userdata/")else{return false;};
    if let Some(r)=name.strip_prefix("replays/Replay"){
        let Some((n,tail))=r.split_once(".peg")else{return false;};
        return digits(n,true)&&(tail.is_empty()||tail.strip_prefix('.').is_some_and(|s|!s.is_empty()&&s.bytes().all(|c|c.is_ascii_alphanumeric()||c==b'_'||c==b'-')));
    }
    if name.contains('/'){return false;}
    if matches!(name,"users.dat"|"highscores.dat"){return true;}
    if let Some(s)=name.strip_prefix("user").and_then(|s|s.strip_suffix(".dat")){return digits(s,false);}
    if let Some(s)=name.strip_prefix("stat_").and_then(|s|s.strip_suffix(".dat")){return !s.is_empty()&&s.bytes().all(|c|c.is_ascii_alphanumeric()||c==b'_'||c==b'-');}
    for prefix in ["stage","stagenights","arcade","trophynights","trophy","duel"]{
        if let Some(s)=name.strip_prefix(prefix).and_then(|s|s.strip_suffix(".sav")){if digits(s,false){return true;}}
    }false
}
#[derive(Debug,Clone)]
struct Profiles {entries:Vec<(String,u32)>}
fn profiles(files:&BTreeMap<String,Vec<u8>>)->Result<Profiles>{
    let b=files.get("userdata/users.dat").ok_or("Missing in-game user index")?;
    if b.len()<6||u32::from_le_bytes(b[0..4].try_into().unwrap())!=1{return Err("Unsupported HD in-game user index version".into());}
    let n=u16::from_le_bytes(b[4..6].try_into().unwrap())as usize;
    if n==0||n>1000{return Err("A backup must contain 1–1000 HD in-game users".into());}
    let(mut off,mut entries,mut names,mut ids)=(6,Vec::new(),HashSet::new(),HashSet::new());
    for _ in 0..n {
        if off+2>b.len(){return Err("Truncated in-game user index".into());}
        let len=u16::from_le_bytes(b[off..off+2].try_into().unwrap())as usize;off+=2;
        if len==0||len>1024||off+len+8>b.len(){return Err("Invalid in-game user entry length".into());}
        let name=std::str::from_utf8(&b[off..off+len]).map_err(|_|"Unsupported in-game user name encoding")?.to_owned();off+=len;
        let file_id=u32::from_le_bytes(b[off+4..off+8].try_into().unwrap());off+=8;
        if name.chars().any(|c|c.is_control())||file_id==0||!names.insert(name.clone())||!ids.insert(file_id){return Err("Duplicate or invalid in-game user identity".into());}
        let details=files.get(&format!("userdata/user{file_id}.dat")).ok_or("In-game user details referenced by the index are missing")?;
        if details.len()<494||u32::from_le_bytes(details[0..4].try_into().unwrap())!=25{return Err("Unsupported or truncated HD in-game user details".into());}
        entries.push((name,file_id));
    }
    if off!=b.len(){return Err("Unsupported trailing in-game user index data".into());}
    // An unindexed details file is ambiguous; no silent profile merging.
    for p in files.keys(){if let Some(s)=p.strip_prefix("userdata/user").and_then(|s|s.strip_suffix(".dat")){if s!="s"&&!ids.contains(&s.parse::<u32>().map_err(|_|"Invalid in-game user detail ID")?){return Err("Unindexed in-game user details are not supported".into());}}}
    Ok(Profiles{entries})
}
#[derive(Debug,Clone)]
pub struct Preview {pub token:String,pub profiles:usize,pub files:usize,pub bytes:u64,pub selected_id:u32}
struct Validated {files:BTreeMap<String,Vec<u8>>,selected_id:u32,profiles:Profiles,state:Value}
pub struct Manager {docs:PathBuf,base:PathBuf,lock:File,pending:BTreeMap<String,Validated>,raw_pending:BTreeMap<String,raw_import::RawValidated>}
pub struct GameLease {pub data_root:PathBuf,_lock:File}
impl Manager {
    pub fn open(documents:&Path)->Result<Self>{
        if !documents.is_absolute()||documents.components().any(|x|matches!(x,std::path::Component::ParentDir)){return Err("Invalid application documents path".into());}
        let docs=fs::canonicalize(documents).map_err(err)?;directory(&docs)?;
        if docs.file_name().and_then(|s|s.to_str())!=Some("Documents"){return Err("Expected this application's Documents directory".into());}
        let library=docs.parent().ok_or("Invalid app container")?.join("Library");child_dir(&library)?;
        let support=library.join("Application Support");child_dir(&support)?;
        let base=support.join("PeggleSaveManager");child_dir(&base)?;
        for x in ["slots","recovery","outbox"]{child_dir(&base.join(x))?;}
        let lock=OpenOptions::new().read(true).write(true).create(true).mode(0o600).custom_flags(libc::O_NOFOLLOW).open(base.join("session.lock")).map_err(err)?;
        if unsafe{libc::flock(lock.as_raw_fd(),libc::LOCK_EX|libc::LOCK_NB)}!=0{return Err("Save management is unavailable while this game session is active".into());}
        let m=Self{docs,base,lock,pending:BTreeMap::new(),raw_pending:BTreeMap::new()};m.state()?;Ok(m)
    }
    fn state(&self)->Result<Value>{
        directory(&self.base)?;for x in ["slots","recovery","outbox"]{directory(&self.base.join(x))?;}
        let p=self.base.join("state.json");no_link(&p)?;if !p.exists(){return Ok(json!({"schema":1,"generation":0,"active":"legacy","previous":null,"recovery":null}));}
        let v:Value=serde_json::from_slice(&read(&p,MAX_META)?).map_err(err)?;
        if v["schema"]!=1||v["generation"].as_u64().is_none(){return Err("The save file selection is not recognized; existing files were retained".into());}
        for key in ["active","previous"]{if key=="previous"&&v[key].is_null(){continue;}let s=v[key].as_str().ok_or("Invalid save file selection")?;if s!="legacy"&&!valid_id(s){return Err("Invalid save file set identifier".into());}self.data_root(s)?;}
        if v["previous"].is_null()!=v["recovery"].is_null(){return Err("Save recovery state is inconsistent".into());}
        if !v["recovery"].is_null()&&!v["recovery"].as_str().is_some_and(valid_id){return Err("Unsafe recovery identifier".into());}Ok(v)
    }
    fn data_root(&self,slot:&str)->Result<PathBuf>{
        if slot=="legacy"{return Ok(self.docs.clone());}
        if !valid_id(slot){return Err("Invalid save file set identifier".into());}
        let p=self.base.join("slots").join(slot);directory(&p)?;let complete=p.join("SlotComplete.json");let v:Value=serde_json::from_slice(&read(&complete,MAX_META)?).map_err(err)?;
        if v["game"]!=GAME||v["version"]!=GAME_VERSION||v["complete"]!=true{return Err("The selected set of save files is incomplete".into());}Ok(p)
    }
    fn guest(data_root:&Path)->PathBuf{data_root.join("touchHLE_sandbox").join(GAME)}
    fn active_guest(&self,state:&Value)->Result<PathBuf>{let data=self.data_root(state["active"].as_str().ok_or("Invalid active save file selection")?)?;let sandbox=data.join("touchHLE_sandbox");no_link(&sandbox)?;if sandbox.exists(){directory(&sandbox)?;}let guest=Self::guest(&data);no_link(&guest)?;if guest.exists(){directory(&guest)?;}Ok(guest)}
    fn collect(&self,guest:&Path)->Result<(BTreeMap<String,Vec<u8>>,u32,bool)>{
        let dir=guest.join("Documents");directory(&dir)?;let userdata=dir.join("userdata");directory(&userdata)?;
        let(mut files,mut total,mut excluded)=(BTreeMap::new(),0u64,false);
        for entry in fs::read_dir(&userdata).map_err(err)?{
            let p=entry.map_err(err)?.path();no_link(&p)?;let name=p.file_name().and_then(|s|s.to_str()).ok_or("Unsupported save filename")?;
            if name=="game_center.dat"{if !p.is_file(){return Err("Unsupported account-state entry".into());}excluded=true;continue;}
            let members=if name=="replays"{directory(&p)?;fs::read_dir(&p).map_err(err)?.map(|x|x.map(|x|x.path()).map_err(err)).collect::<Result<Vec<_>>>()?}else{vec![p.clone()]};
            for file in members {
                no_link(&file)?;let rel=file.strip_prefix(&dir).map_err(err)?.to_str().ok_or("Unsupported save path")?.to_owned();
                if !allowed(&rel)||files.len()>=MAX_FILES{return Err("Unsupported save file or too many files".into());}
                let bytes=read(&file,MAX_FILE)?;total+=bytes.len()as u64;if total>MAX_TOTAL{return Err("Save set is too large".into());}files.insert(rel,bytes);
            }
        }
        let ps=profiles(&files)?;let mut selected=ps.entries[0].1;
        let library=guest.join("Library");no_link(&library)?;let prefs=library.join("Preferences");no_link(&prefs)?;let p=prefs.join(format!("{GAME}.plist"));
        no_link(&p)?;if p.exists(){let bytes=read(&p,MAX_META)?;let value=plist::Value::from_reader(std::io::Cursor::new(bytes)).map_err(err)?;
            let dictionary=value.as_dictionary().ok_or("Unsupported guest preference format")?;
            if let Some(value)=dictionary.get("CurUser"){let name=value.as_string().ok_or("Unsupported current-in-game user preference")?;selected=ps.entries.iter().find(|(n,_)|n==name).ok_or("Current in-game user is not present in the index")?.1;}}
        Ok((files,selected,excluded))
    }
    pub fn export(&self)->Result<PathBuf>{
        let state=self.state()?;let guest=self.active_guest(&state)?;if !guest.exists(){return Err("No in-game users found yet. Open Peggle, create an in-game user, then restart to export their save files.".into());}let(files,selected,excluded)=self.collect(&guest)?;
        let manifest=json!({"format":"PeggleHDSave","schema":1,"game":GAME,"version":GAME_VERSION,"original_ipa_sha256":GAME_SHA,"selected_profile_id":selected,"account_state_policy":"excluded","game_center_file_excluded":excluded,"files":files.iter().map(|(p,b)|json!({"path":p,"bytes":b.len(),"sha256":hash(b)})).collect::<Vec<_>>()});
        let out=self.base.join("outbox").join(format!("Peggle-HD-backup-{}.zip",id()));let f=OpenOptions::new().write(true).create_new(true).mode(0o600).custom_flags(libc::O_NOFOLLOW).open(&out).map_err(err)?;
        let mut zip=zip::ZipWriter::new(f);let options=zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated).unix_permissions(0o600);
        zip.start_file("manifest.json",options).map_err(err)?;zip.write_all(&serde_json::to_vec(&manifest).map_err(err)?).map_err(err)?;
        for(p,b)in &files{zip.start_file(p,options).map_err(err)?;zip.write_all(b).map_err(err)?;}
        zip.finish().map_err(err)?.sync_all().map_err(err)?;
        // Verify the actual resulting archive, not only its source buffers.
        self.parse_archive(&out)?;
        let (again,again_selected,again_excluded)=self.collect(&guest)?;
        if again!=files||again_selected!=selected||again_excluded!=excluded||self.state()?!=state{return Err("Save set changed during export; no backup was offered for sharing".into());}Ok(out)
    }
    fn parse_archive(&self,path:&Path)->Result<(BTreeMap<String,Vec<u8>>,u32,Profiles)>{
        no_link(path)?;let f=OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW).open(path).map_err(err)?;
        if !f.metadata().map_err(err)?.is_file()||f.metadata().map_err(err)?.len()>MAX_TOTAL+MAX_META+4*1024*1024{return Err("Backup archive is too large".into());}
        let mut zip=zip::ZipArchive::new(f).map_err(err)?;if zip.len()==0||zip.len()>MAX_FILES+1{return Err("Invalid backup file count".into());}
        let(mut files,mut manifest,mut seen,mut total)=(BTreeMap::new(),None,HashSet::new(),0u64);
        for n in 0..zip.len(){let mut f=zip.by_index(n).map_err(err)?;let name=f.name().to_owned();
            if f.is_dir()||!seen.insert(name.to_ascii_lowercase())||f.unix_mode().is_some_and(|m|m&0o170000!=0&&m&0o170000!=0o100000){return Err("Directories, links and duplicate archive paths are not accepted".into());}
            let limit=if name=="manifest.json"{MAX_META}else{if !allowed(&name){return Err("Foreign or unsafe backup path".into());}MAX_FILE};
            if f.size()>limit{return Err("Backup member exceeds the size limit".into());}if name!="manifest.json"{total=total.checked_add(f.size()).ok_or("Backup size overflow")?;if total>MAX_TOTAL{return Err("Backup exceeds the total size limit".into());}}
            let mut b=Vec::new();(&mut f).take(limit+1).read_to_end(&mut b).map_err(err)?;if b.len()as u64!=f.size()||b.len()as u64>limit{return Err("Invalid or oversized backup member".into());}
            if name=="manifest.json"{let value=serde_json::from_slice::<Value>(&b).map_err(err)?;if serde_json::to_vec(&value).map_err(err)?!=b{return Err("Backup manifest has duplicate keys or unsupported edited metadata".into());}manifest=Some(value);}else{files.insert(name,b);}
        }
        let m=manifest.ok_or("This is not a Save File Management backup; original folder of save filess need a separate compatible importer")?;
        if m["format"]!="PeggleHDSave"||m["schema"]!=1||m["game"]!=GAME||m["version"]!=GAME_VERSION||m["original_ipa_sha256"]!=GAME_SHA||m["account_state_policy"]!="excluded"{return Err("Unsupported backup format or foreign game/version".into());}
        let entries=m["files"].as_array().ok_or("Missing backup manifest file list")?;let mut hashes=BTreeMap::new();
        for x in entries{let p=x["path"].as_str().ok_or("Invalid manifest path")?;if !allowed(p)||hashes.contains_key(p){return Err("Invalid or duplicate manifest path".into());}
            let b=files.get(p).ok_or("A manifest file is missing")?;if x["bytes"].as_u64()!=Some(b.len()as u64)||x["sha256"].as_str()!=Some(hash(b).as_str()){return Err("Backup file hash or length mismatch".into());}hashes.insert(p,());}
        if hashes.len()!=files.len(){return Err("Unlisted backup data is not accepted".into());}
        let ps=profiles(&files)?;let selected=m["selected_profile_id"].as_u64().and_then(|x|u32::try_from(x).ok()).ok_or("Invalid selected in-game user")?;
        if !ps.entries.iter().any(|(_,n)|*n==selected){return Err("Selected in-game user is not indexed".into());}Ok((files,selected,ps))
    }
    pub fn validate(&mut self,path:&Path)->Result<Preview>{
        self.raw_pending.clear();let(files,selected_id,profiles)=self.parse_archive(path)?;let token=id();let preview=Preview{token:token.clone(),profiles:profiles.entries.len(),files:files.len(),bytes:files.values().map(|b|b.len()as u64).sum(),selected_id};
        // Memory-only pending data binds confirmation to these exact bytes.
        self.pending.clear();self.pending.insert(token,Validated{files,selected_id,profiles,state:self.state()?});Ok(preview)
    }
    pub fn cancel_validation(&mut self){self.pending.clear();self.raw_pending.clear();}
    fn fingerprint(&self,guest:&Path)->Result<Value>{
        no_link(guest)?;if !guest.exists(){return Ok(json!({}));}directory(guest)?;
        fn walk(root:&Path,p:&Path,out:&mut BTreeMap<String,Value>,total:&mut u64,depth:usize,entries:&mut usize)->Result<()>{
            if depth>16{return Err("Recovery directory nesting exceeds the supported limit".into());}
            for e in fs::read_dir(p).map_err(err)?{let p=e.map_err(err)?.path();no_link(&p)?;*entries+=1;if *entries>MAX_FILES{return Err("Recovery set has too many entries".into());}
                if p.is_dir(){walk(root,&p,out,total,depth+1,entries)?;}else{if out.len()>=MAX_FILES{return Err("Recovery set has too many files".into());}let b=read(&p,MAX_FILE)?;*total+=b.len()as u64;if *total>MAX_TOTAL{return Err("Recovery set exceeds the size limit".into());}out.insert(p.strip_prefix(root).map_err(err)?.to_str().ok_or("Unsupported recovery filename")?.into(),json!({"bytes":b.len(),"sha256":hash(&b)}));}
            }Ok(())
        }
        let(mut out,mut total,mut entries)=(BTreeMap::new(),0,0);walk(guest,guest,&mut out,&mut total,0,&mut entries)?;Ok(json!(out))
    }
    fn publish(&self,old:&Value,active:&str,previous:&str,recovery:&str)->Result<()>{
        if self.state()?!=*old{return Err("The selected save files changed; check them again before restoring".into());}
        let new=json!({"schema":1,"generation":old["generation"].as_u64().unwrap().checked_add(1).ok_or("Save generation overflow")?,"active":active,"previous":previous,"recovery":recovery});
        let temp=self.base.join(format!("state-next-{}.json",id()));write_new(&temp,&serde_json::to_vec(&new).map_err(err)?)?;
        fs::rename(temp,self.base.join("state.json")).map_err(err)?;
        // Publication is committed at rename; directory-sync failure must not
        // misleadingly report that the old selection is still active. Power
        // loss durability beyond the completed atomic rename is not promised.
        let _=sync_dir(&self.base);Ok(())
    }
    pub fn restore(&mut self,token:&str,confirmed:bool)->Result<()>{self.restore_inner(token,confirmed,0)}
    fn restore_inner(&mut self,token:&str,confirmed:bool,fail:u8)->Result<()>{
        if !confirmed{return Err("Restore requires explicit confirmation".into());}
        let v=self.pending.remove(token).ok_or("Validate this backup before restoring")?;if self.state()?!=v.state{return Err("Save state changed; validate again".into());}
        if fail==1{return Err("Injected interruption before staging".into());}
        let slot=id();let data=self.base.join("slots").join(&slot);fs::create_dir(&data).map_err(err)?;
        let sandbox=data.join("touchHLE_sandbox");fs::create_dir(&sandbox).map_err(err)?;let guest=Self::guest(&data);fs::create_dir(&guest).map_err(err)?;let documents=guest.join("Documents");fs::create_dir(&documents).map_err(err)?;fs::create_dir(documents.join("userdata")).map_err(err)?;
        for(p,b)in &v.files{let p=documents.join(p);if let Some(parent)=p.parent(){if !parent.exists(){fs::create_dir(parent).map_err(err)?;}}write_new(&p,b)?;}
        fs::create_dir(guest.join("tmp")).map_err(err)?;
        let library=guest.join("Library");fs::create_dir(&library).map_err(err)?;let preferences=library.join("Preferences");fs::create_dir(&preferences).map_err(err)?;
        let name=&v.profiles.entries.iter().find(|(_,n)|*n==v.selected_id).unwrap().0;let mut d=plist::Dictionary::new();d.insert("CurUser".into(),plist::Value::String(name.clone()));let mut b=Vec::new();plist::Value::Dictionary(d).to_writer_xml(&mut b).map_err(err)?;write_new(&preferences.join(format!("{GAME}.plist")),&b)?;
        // Verify the staged bytes/profile selection before marking complete.
        let(checked,selected,excluded)=self.collect(&guest)?;if checked!=v.files||selected!=v.selected_id||excluded{return Err("Staged save verification failed".into());}
        if fail==2{return Err("Injected interruption after staging".into());}
        write_new(&data.join("SlotComplete.json"),&serde_json::to_vec(&json!({"complete":true,"game":GAME,"version":GAME_VERSION,"archive_files":v.files.len()})).map_err(err)?)?;sync_dir(&data)?;
        let old_guest=self.active_guest(&v.state)?;let recovery=id();let fingerprint=self.fingerprint(&old_guest)?;
        write_new(&self.base.join("recovery").join(format!("{recovery}.json")),&serde_json::to_vec(&json!({"game":GAME,"state":v.state,"files":fingerprint})).map_err(err)?)?;sync_dir(&self.base.join("recovery"))?;
        if fail==3{return Err("Injected interruption after recovery receipt".into());}
        if self.fingerprint(&old_guest)?!=fingerprint{return Err("Current set of save files changed before restore; existing data retained".into());}
        self.publish(&v.state,&slot,v.state["active"].as_str().unwrap(),&recovery)
    }
    pub fn can_rollback(&self)->Result<bool>{Ok(!self.state()?["previous"].is_null())}
    pub fn rollback(&mut self,confirmed:bool)->Result<()>{
        if !confirmed{return Err("Rollback requires explicit confirmation".into());}let old=self.state()?;let previous=old["previous"].as_str().ok_or("No previous set of save files is retained")?;let rid=old["recovery"].as_str().ok_or("Missing recovery receipt")?;
        let receipt:Value=serde_json::from_slice(&read(&self.base.join("recovery").join(format!("{rid}.json")),MAX_META)?).map_err(err)?;
        if receipt["game"]!=GAME||receipt["state"]["active"]!=previous{return Err("The recovery record does not match the previous set of save files".into());}
        let previous_guest=self.active_guest(&json!({"active":previous}))?;if self.fingerprint(&previous_guest)?!=receipt["files"]{return Err("Previous set of save files changed; rollback was not applied".into());}
        let recovery=id();let current=self.active_guest(&old)?;let fingerprint=self.fingerprint(&current)?;
        write_new(&self.base.join("recovery").join(format!("{recovery}.json")),&serde_json::to_vec(&json!({"game":GAME,"state":old,"files":fingerprint})).map_err(err)?)?;
        self.publish(&old,previous,old["active"].as_str().unwrap(),&recovery)
    }
    pub fn begin_game(self)->Result<GameLease>{let s=self.state()?;self.active_guest(&s)?;let data_root=self.data_root(s["active"].as_str().unwrap())?;Ok(GameLease{data_root,_lock:self.lock})}
}

#[cfg(test)]
#[path="save_manager_tests.rs"]mod tests;

#[path="raw_import.rs"]pub mod raw_import;

#[path="progress.rs"]pub mod progress;

impl Manager {
 pub fn export_folder(&self)->Result<PathBuf>{
  // Export and verify through the same bounded snapshot as ZIP backups.
  let archive=self.export()?;let(files,_,_)=self.parse_archive(&archive)?;
  let out=self.base.join("outbox").join(format!("Peggle-Saves-{}",id()));fs::create_dir(&out).map_err(err)?;
  for(path,bytes)in files {let dest=out.join(path);fs::create_dir_all(dest.parent().unwrap()).map_err(err)?;write_new(&dest,&bytes)?;if read(&dest,MAX_FILE)?!=bytes{return Err("Exported folder verification failed".into());}}
  // Include the verified backup's manifest for source identity on reimport.
  let mut zip=zip::ZipArchive::new(File::open(&archive).map_err(err)?).map_err(err)?;let mut manifest=Vec::new();zip.by_name("manifest.json").map_err(err)?.take(MAX_META+1).read_to_end(&mut manifest).map_err(err)?;write_new(&out.join("manifest.json"),&manifest)?;Ok(out)
 }
}
