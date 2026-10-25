//! C53's host-only feature profile survives ordinary app launches. Guest saves,
//! guest preferences, original code/assets and runtime hook guards are separate.
use std::{fs::{self,OpenOptions},io::{self,Read,Write},path::Path};
pub const FILE_NAME:&str="Peggle-C53-runtime.conf";
pub const FEATURES:[(&str,&str);30]=[
    ("matrix","PEGGLE_HD_MATRIX_FAST"),
    ("endpoint_index","PEGGLE_HD_ENDPOINT_NATIVE"),
    ("endpoint_compare","PEGGLE_HD_ENDPOINT_COMPARE_NATIVE"),
    ("png","PEGGLE_HD_PNG_NATIVE"),
    ("gpu_transfer","PEGGLE_HD_GPU_TRANSFER"),
    ("copy_ring","PEGGLE_HD_COPY_RING"),
    ("direct_texture","PEGGLE_HD_DIRECT_TEXTURE"),
    ("collision_swap","PEGGLE_HD_COLLISION_SWAP_NATIVE"),
    ("file_zlib","PEGGLE_HD_FILE_ZLIB_NATIVE"),
    ("font_lookup","PEGGLE_HD_FONT_LOOKUP_NATIVE"),
    ("png_filter","PEGGLE_HD_PNG_FILTER_NATIVE"),
    ("vertex_pack","PEGGLE_HD_VERTEX_PACK_NATIVE"),
    ("collision_geometry","PEGGLE_HD_COLLISION_GEOMETRY_NATIVE"),
    ("collision_pair","PEGGLE_HD_COLLISION_PAIR_NATIVE"),
    ("jpeg_ycc","PEGGLE_HD_JPEG_YCC_NATIVE"),
    ("mapped_char","PEGGLE_HD_MAPPED_CHAR_NATIVE"),
    ("premultiply","PEGGLE_HD_PREMULTIPLY_NATIVE"),
    ("adaptive_present","PEGGLE_HD_ADAPTIVE_PRESENT"),
    ("pixel_pack","PEGGLE_HD_PIXEL_PACK_NATIVE"),
    ("inflate_loops","PEGGLE_HD_INFLATE_LOOPS_NATIVE"),
    ("ui_helpers","PEGGLE_HD_UI_HELPERS_NATIVE"),
    ("row_transforms","PEGGLE_HD_ROW_TRANSFORMS_NATIVE"),
    ("menu_cadence","PEGGLE_HD_MENU_CADENCE"),
    ("inflate_fast","PEGGLE_HD_INFLATE_FAST_NATIVE"),
    ("clip_rect","PEGGLE_HD_CLIP_RECT_NATIVE"),
    ("jpeg_idct","PEGGLE_HD_JPEG_IDCT_NATIVE"),
    ("gif_island","PEGGLE_HD_GIF_ISLAND_NATIVE"),
    ("font_pipeline","PEGGLE_HD_FONT_PIPELINE_NATIVE"),
    ("jpeg_pipeline","PEGGLE_HD_JPEG_PIPELINE_NATIVE"),
    ("collision_pipeline","PEGGLE_HD_COLLISION_PIPELINE_NATIVE"),
];
pub const FEATURE_COUNT:usize=FEATURES.len();
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct Settings(pub [bool;FEATURE_COUNT]);
// C59: prefer the C34 device-validated LZ-copy primitive over the whole-loop
// path that disabled itself on C54 real inputs. Explicit overrides still work.
impl Default for Settings {fn default()->Self {let mut v=[true;FEATURE_COUNT];v[23]=false;Self(v)}}
impl Settings {
    pub fn encode(self)->String {
        let mut out="format=1\ncandidate=46\n".to_owned();
        for ((key,_),enabled) in FEATURES.into_iter().zip(self.0) {
            out.push_str(&format!("{key}={}\n",u8::from(enabled)));
        }
        out
    }
    pub fn parse(bytes:&[u8])->Result<Self,&'static str> {
        if bytes.len()>4096 {return Err("oversized");}
        let text=std::str::from_utf8(bytes).map_err(|_|"non_utf8")?;
        let mut seen=[false;FEATURE_COUNT+2];let mut settings=Settings([false;FEATURE_COUNT]);
        for line in text.lines() {
            let line=line.trim();if line.is_empty()||line.starts_with('#') {continue;}
            let (key,value)=line.split_once('=').ok_or("syntax")?;
            let key=key.trim();let value=value.trim();
            let index=match key {
                "format"=>{if value!="1" {return Err("format");}0},
                "candidate"=>{if value!="46" {return Err("candidate");}1},
                _=>{
                    let i=FEATURES.iter().position(|(k,_)|*k==key).ok_or("unknown_key")?;
                    settings.0[i]=match value {"1"=>true,"0"=>false,_=>return Err("boolean")};i+2
                }
            };
            if seen[index] {return Err("duplicate_key");}seen[index]=true;
        }
        if !seen.into_iter().all(|v|v) {return Err("missing_key");}
        Ok(settings)
    }
}
pub struct Resolution {
    pub effective:Settings,
    pub source:&'static str,
    pub persisted:bool,
    pub warnings:Vec<String>,
    pub overrides:Vec<&'static str>,
}
impl Resolution {
    pub fn event_json(&self)->String {
        let features=FEATURES.iter().zip(self.effective.0).map(|((key,_),v)|format!("\"{key}\":{v}")).collect::<Vec<_>>().join(",");
        // All warning strings come from fixed enum/key identifiers, never raw
        // environment values, file contents, user names or private paths.
        let warnings=self.warnings.iter().map(|w|format!("\"{w}\"")).collect::<Vec<_>>().join(",");
        let overrides=self.overrides.iter().map(|w|format!("\"{w}\"")).collect::<Vec<_>>().join(",");
        format!("{{\"event\":\"runtime_configuration\",\"candidate\":53,\"source\":\"{}\",\"persisted\":{},\"settings_file\":\"{FILE_NAME}\",\"effective\":{{{features}}},\"environment_overrides\":[{overrides}],\"warnings\":[{warnings}],\"runtime_guards_required\":true}}",self.source,self.persisted)
    }
}
fn safe_off(reason:String)->Resolution {Resolution{effective:Settings([false;FEATURE_COUNT]),source:"invalid_settings_safe_off",persisted:false,warnings:vec![reason],overrides:vec![]}}
fn store(path:&Path,settings:Settings,replace:bool)->io::Result<()> {
    let stamp=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos();
    let temporary=path.with_file_name(format!(".{FILE_NAME}.{}.{stamp}.tmp",std::process::id()));
    let mut options=OpenOptions::new();options.write(true).create_new(true);
    #[cfg(unix)] {use std::os::unix::fs::OpenOptionsExt;options.mode(0o600);}
    let result=(|| {
        let mut f=options.open(&temporary)?;f.write_all(settings.encode().as_bytes())?;f.sync_all()?;drop(f);
        if replace {fs::rename(&temporary,path)}else{fs::hard_link(&temporary,path)}
    })();
    let _=fs::remove_file(temporary);result
}
pub fn resolve(documents:&Path,mut environment:impl FnMut(&str)->Option<String>)->Resolution {
    let meta=match fs::symlink_metadata(documents) {Ok(m)=>m,Err(e)=>return safe_off(format!("documents_{:?}",e.kind()))};
    if !meta.is_dir()||meta.file_type().is_symlink() {return safe_off("documents_not_regular_directory".into());}
    let path=documents.join(FILE_NAME);
    let previous=match fs::symlink_metadata(&path) {
        Ok(m)=>{
            if !m.is_file()||m.file_type().is_symlink()||m.len()>4096 {return safe_off("settings_not_regular_or_oversized".into());}
            let mut bytes=Vec::new();
            let read=fs::File::open(&path).and_then(|f|f.take(4097).read_to_end(&mut bytes));
            if let Err(e)=read {return safe_off(format!("settings_read_{:?}",e.kind()));}
            match Settings::parse(&bytes) {Ok(v)=>Some(v),Err(reason)=>return safe_off(format!("settings_parse_{reason}"))}
        }
        Err(e)if e.kind()==io::ErrorKind::NotFound=>None,
        Err(e)=>return safe_off(format!("settings_metadata_{:?}",e.kind())),
    };
    let mut desired=previous.unwrap_or_default();let mut invalid=Vec::new();let mut overrides=Vec::new();
    for (i,(key,var)) in FEATURES.iter().enumerate() {
        match environment(var).as_deref() {
            Some("0")=>{desired.0[i]=false;overrides.push(*key);},
            Some("1")=>{desired.0[i]=true;overrides.push(*key);},
            Some(_)=>invalid.push(i),None=>(),
        }
    }
    let changed=previous!=Some(desired);
    let mut warnings=Vec::new();
    let persisted=if changed {
        match store(&path,desired,previous.is_some()) {Ok(())=>true,Err(e)=>{warnings.push(format!("settings_write_{:?}",e.kind()));false}}
    }else{true};
    let invalid_environment=!invalid.is_empty();
    let mut effective=desired;
    for i in invalid {effective.0[i]=false;warnings.push(format!("invalid_environment_{}",FEATURES[i].0));}
    let source=if !warnings.is_empty(){"resolved_with_warning"}else if !overrides.is_empty(){"environment_persisted"}else if previous.is_some(){"persisted_settings"}else{"candidate_defaults_persisted"};
    Resolution{effective,source,persisted:persisted && !invalid_environment,warnings,overrides}
}

/// Apply the resolved profile before engine startup. Kept here so subprocess
/// tests exercise the exact production environment application, not a copy.
pub fn initialize(documents:&Path)->Resolution {
    let runtime=resolve(documents,|key| {
        std::env::var_os(key).map(|v|v.into_string().unwrap_or_else(|_|"INVALID_NON_UNICODE".into()))
    });
    for ((_,key),enabled) in FEATURES.into_iter().zip(runtime.effective.0) {
        std::env::set_var(key,if enabled {"1"}else{"0"});
    }
    runtime
}

#[cfg(test)]
mod tests {
    use super::*;
    static NEXT:std::sync::atomic::AtomicUsize=std::sync::atomic::AtomicUsize::new(0);
    struct Temp(std::path::PathBuf);
    impl Temp {fn new()->Self {let p=std::env::temp_dir().join(format!("peggle-c35-settings-test-{}-{}",std::process::id(),NEXT.fetch_add(1,std::sync::atomic::Ordering::Relaxed)));fs::create_dir(&p).unwrap();Self(p)}}
    impl Drop for Temp {fn drop(&mut self){let _=fs::remove_dir_all(&self.0);}}
    #[test]fn ordinary_first_and_subsequent_launches_enable_all_intended_features(){
        let t=Temp::new();let a=resolve(&t.0,|_|None);assert_eq!(a.effective,Settings::default());assert!(a.persisted);assert_eq!(a.source,"candidate_defaults_persisted");
        let bytes=fs::read(t.0.join(FILE_NAME)).unwrap();assert_eq!(Settings::parse(&bytes).unwrap(),Settings::default());
        let b=resolve(&t.0,|_|None);assert_eq!(b.effective,a.effective);assert!(b.persisted);assert_eq!(b.source,"persisted_settings");assert_eq!(fs::read(t.0.join(FILE_NAME)).unwrap(),bytes);
    }
    #[test]fn copy_ring_rollback_is_persistent_without_disabling_other_features(){
        let t=Temp::new();let a=resolve(&t.0,|key|(key=="PEGGLE_HD_COPY_RING").then(||"0".into()));
        assert_eq!(a.effective.0,{let mut v=Settings::default().0;v[5]=false;v});
        assert_eq!(resolve(&t.0,|_|None).effective,a.effective);
        let b=resolve(&t.0,|key|(key=="PEGGLE_HD_COPY_RING").then(||"1".into()));
        assert_eq!(b.effective,Settings::default());assert_eq!(resolve(&t.0,|_|None).effective,b.effective);
    }
    #[test]fn direct_texture_rollback_is_persistent_and_independent(){
        let t=Temp::new();let a=resolve(&t.0,|key|(key=="PEGGLE_HD_DIRECT_TEXTURE").then(||"0".into()));
        assert_eq!(a.effective.0,{let mut v=Settings::default().0;v[6]=false;v});
        assert_eq!(resolve(&t.0,|_|None).effective,a.effective);
        let b=resolve(&t.0,|key|(key=="PEGGLE_HD_DIRECT_TEXTURE").then(||"1".into()));assert_eq!(b.effective,Settings::default());
    }
    #[test]fn native_feature_rollbacks_persist_independently(){
        for (index,key)in FEATURES.iter().enumerate().skip(7).map(|(i,(_,key))|(i,*key)){let t=Temp::new();let a=resolve(&t.0,|k|(k==key).then(||"0".into()));let mut expected=Settings::default().0;expected[index]=false;assert_eq!(a.effective.0,expected);assert_eq!(resolve(&t.0,|_|None).effective,a.effective);let mut enabled=Settings::default().0;enabled[index]=true;assert_eq!(resolve(&t.0,|k|(k==key).then(||"1".into())).effective,Settings(enabled));}
    }
    #[test]fn explicit_disable_survives_launch_without_environment(){
        let t=Temp::new();let a=resolve(&t.0,|key|(key=="PEGGLE_HD_GPU_TRANSFER").then(||"0".into()));assert!(!a.effective.0[4]);
        let b=resolve(&t.0,|_|None);assert_eq!(b.effective,a.effective);assert!(!b.effective.0[4]);assert_eq!(b.effective.0[..4],[true;4]);
    }
    #[test]fn valid_override_updates_existing_profile_atomically(){
        let t=Temp::new();resolve(&t.0,|_|Some("0".into()));let a=resolve(&t.0,|key|(key=="PEGGLE_HD_PNG_NATIVE").then(||"1".into()));
        assert_eq!(a.effective.0,{let mut v=[false;FEATURE_COUNT];v[3]=true;v});assert_eq!(resolve(&t.0,|_|None).effective,a.effective);assert_eq!(fs::read_dir(&t.0).unwrap().count(),1);
    }
    #[test]fn invalid_override_disables_only_live_feature_without_poisoning_saved_profile(){
        let t=Temp::new();resolve(&t.0,|_|None);let before=fs::read(t.0.join(FILE_NAME)).unwrap();
        let a=resolve(&t.0,|key|(key=="PEGGLE_HD_GPU_TRANSFER").then(||"not_a_boolean".into()));assert!(!a.effective.0[4]);assert_eq!(a.warnings,vec!["invalid_environment_gpu_transfer"]);assert_eq!(fs::read(t.0.join(FILE_NAME)).unwrap(),before);
        assert_eq!(resolve(&t.0,|_|None).effective,Settings::default());
    }
    #[test]fn malformed_profile_is_preserved_and_falls_back_safely(){
        let t=Temp::new();let p=t.0.join(FILE_NAME);fs::write(&p,b"unrecognized user data").unwrap();
        let a=resolve(&t.0,|_|Some("1".into()));assert_eq!(a.effective,Settings([false;FEATURE_COUNT]));assert!(!a.persisted);assert_eq!(fs::read(p).unwrap(),b"unrecognized user data");
    }
    #[test]fn strict_schema_rejects_unknown_duplicate_incomplete_and_wrong_version(){
        let text=Settings::default().encode();
        for bad in [format!("{text}unknown=1\n"),format!("{text}png=0\n"),text.replace("format=1","format=2"),text.replace("candidate=46","candidate=22"),text.replace("png=1\n",""),text.replace("png=1","png=yes")] {assert!(Settings::parse(bad.as_bytes()).is_err());}
        assert!(Settings::parse(&[255]).is_err());assert!(Settings::parse(&vec![b' ';4097]).is_err());
    }
    #[test]fn whitespace_comments_and_explicit_profile_values_are_supported(){
        let text=Settings({let mut v=[true;FEATURE_COUNT];v[1]=false;v[3]=false;v}).encode().replace("png=0","png = 0 ");assert_eq!(Settings::parse(format!("# local profile\n\n{text}").as_bytes()).unwrap(),Settings({let mut v=[true;FEATURE_COUNT];v[1]=false;v[3]=false;v}));
    }
    #[test]fn oversized_settings_are_not_read_or_replaced(){
        let t=Temp::new();let p=t.0.join(FILE_NAME);let bytes=vec![b'x';4097];fs::write(&p,&bytes).unwrap();assert_eq!(resolve(&t.0,|_|None).effective,Settings([false;FEATURE_COUNT]));assert_eq!(fs::read(&p).unwrap(),bytes);
    }
    #[cfg(unix)]#[test]fn redirected_settings_cannot_change_another_file(){
        let t=Temp::new();let external=t.0.join("external");fs::write(&external,Settings::default().encode()).unwrap();std::os::unix::fs::symlink(&external,t.0.join(FILE_NAME)).unwrap();
        assert_eq!(resolve(&t.0,|_|Some("0".into())).effective,Settings([false;FEATURE_COUNT]));assert_eq!(fs::read_to_string(external).unwrap(),Settings::default().encode());
    }
    #[test]fn missing_directory_falls_back_without_creating_other_paths(){
        let t=Temp::new();let p=t.0.join("missing");let a=resolve(&p,|_|None);assert_eq!(a.effective,Settings([false;FEATURE_COUNT]));assert!(!p.exists());
    }
    #[test]fn guest_saves_and_preferences_are_never_modified(){
        let t=Temp::new();let save=t.0.join("touchHLE_sandbox/com.popcap.ios.PeggleHD/Documents/userdata/user1.dat");fs::create_dir_all(save.parent().unwrap()).unwrap();fs::write(&save,b"real existing progress").unwrap();let prefs=t.0.join("existing-preferences");fs::write(&prefs,b"unchanged").unwrap();
        resolve(&t.0,|_|None);resolve(&t.0,|_|Some("0".into()));resolve(&t.0,|_|None);assert_eq!(fs::read(save).unwrap(),b"real existing progress");assert_eq!(fs::read(prefs).unwrap(),b"unchanged");
    }
    #[test]fn log_contains_effective_values_sources_and_no_raw_override_values(){
        let t=Temp::new();let a=resolve(&t.0,|k|(k=="PEGGLE_HD_GPU_TRANSFER").then(||"private-untrusted-value".into()));let log=a.event_json();assert!(log.contains("\"gpu_transfer\":false"));assert!(log.contains("\"candidate\":53"));assert!(log.contains("invalid_environment_gpu_transfer"));assert!(!log.contains("private-untrusted-value"));assert!(log.contains("\"runtime_guards_required\":true"));
    }
}

#[cfg(test)]mod c59_defaults {
 use super::*;
 #[test]fn conservative_decoder_default_preserves_all_other_features(){
  let s=Settings::default();assert!(!s.0[23]);assert!(s.0[19]);
  for (i,v)in s.0.into_iter().enumerate(){assert_eq!(v,i!=23);}
 }
 #[test]fn explicit_whole_loop_opt_in_still_round_trips(){
  let mut s=Settings::default();s.0[23]=true;assert_eq!(Settings::parse(s.encode().as_bytes()).unwrap(),s);
 }
}
