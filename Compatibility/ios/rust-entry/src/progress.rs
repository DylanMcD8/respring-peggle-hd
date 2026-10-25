//! App-independent, additive progress and Adventure checkpoints. No original save bytes travel in
//! the document. The v25 adapters preserve all non-progress fields verbatim.
//! Explicit pre-launch operations only; the session lease excludes live writes.
use super::*;
use std::collections::BTreeSet;
const FORMAT:&str="RespringPeggleProgress";
const LIMIT:u64=512*1024;
#[derive(Clone,Debug,PartialEq,Eq)]
struct Campaign {checkpoint:Option<(u32,u32)>,stage:u32,level:u32,completed:bool,trophies:BTreeSet<u32>,cleared:BTreeSet<String>,aced:BTreeSet<String>}
#[derive(Clone,Debug,PartialEq,Eq)]
struct Progress {classic:Campaign,nights:Campaign}
fn keys(v:&Value,want:&[&str])->Result<()> {let m=v.as_object().ok_or("Expected a save file object")?;if m.len()!=want.len()||want.iter().any(|k|!m.contains_key(*k)){return Err("Unsupported or incomplete save file fields".into());}Ok(())}
fn number(v:&Value)->Result<u32>{v.as_u64().and_then(|n|u32::try_from(n).ok()).ok_or("Invalid number in the save file".into())}
fn text_value(v:&Value)->Result<String>{let s=v.as_str().ok_or("Invalid text in the save file")?;if s.is_empty()||s.len()>1024||s.chars().any(char::is_control){return Err("Unsupported text in the save file".into());}Ok(s.into())}
fn array(v:&Value)->Result<&Vec<Value>>{let a=v.as_array().ok_or("Expected a list in the save file")?;if a.len()>1024{return Err("Too many entries in the save file".into());}Ok(a)}
impl Campaign {
 fn value(&self)->Value{json!({"checkpoint":self.checkpoint.map(|(stage,level)|json!({"stage":stage,"level":level})),"furthest_stage":self.stage,"furthest_level":self.level,"adventure_completed":self.completed,"trophies":self.trophies,"cleared_levels":self.cleared,"aced_levels":self.aced})}
 fn parse(v:&Value,nights:bool,version:u64)->Result<Self>{
  let mut expected=vec!["furthest_stage","furthest_level","adventure_completed","trophies","cleared_levels","aced_levels"];if version==2{expected.push("checkpoint");}keys(v,&expected)?;
  let checkpoint=if version==1||v["checkpoint"].is_null(){None}else{let cp=&v["checkpoint"];keys(cp,&["stage","level"])?;Some((number(&cp["stage"])?,number(&cp["level"])?))};
  let c=Self{checkpoint,stage:number(&v["furthest_stage"])?,level:number(&v["furthest_level"])?,completed:v["adventure_completed"].as_bool().ok_or("Invalid completion flag")?,trophies:array(&v["trophies"])?.iter().map(number).collect::<Result<_>>()?,cleared:array(&v["cleared_levels"])?.iter().map(|v|text_value(v).map(|s|s.to_ascii_lowercase())).collect::<Result<_>>()?,aced:array(&v["aced_levels"])?.iter().map(|v|text_value(v).map(|s|s.to_ascii_lowercase())).collect::<Result<_>>()?};
  let range=if nights{11..=23}else{0..=11};if c.checkpoint.is_some_and(|(s,l)|!range.contains(&s)||l>5||(s,l)>(c.stage,c.level))||!range.contains(&c.stage)||c.level>5||c.trophies.iter().any(|n|*n>65535){return Err("The save file contains a value outside the supported game range".into());}
  for s in [&c.cleared,&c.aced]{let mut seen=HashSet::new();if s.iter().any(|s|!s.is_ascii()||s.len()>256||!seen.insert(s.to_ascii_lowercase())){return Err("Ambiguous or unsupported level names".into());}}
  if c.trophies.len()!=v["trophies"].as_array().unwrap().len()||c.cleared.len()!=v["cleared_levels"].as_array().unwrap().len()||c.aced.len()!=v["aced_levels"].as_array().unwrap().len(){return Err("Duplicate entries in the save file".into());}Ok(c)
 }
 fn merged(&self,other:&Self)->Self{let mut c=self.clone();c.checkpoint=self.checkpoint.max(other.checkpoint);(c.stage,c.level)=(self.stage,self.level).max((other.stage,other.level));c.completed|=other.completed;c.trophies.extend(&other.trophies);for (to,from)in [(&mut c.cleared,&other.cleared),(&mut c.aced,&other.aced)]{for s in from{if !to.iter().any(|x|x.eq_ignore_ascii_case(s)){to.insert(s.clone());}}}c}
}
impl Progress {
 fn document(&self,name:&str)->Value{json!({"format":FORMAT,"schema":2,"scope":"adventure-progress","profile_label":name,"classic":self.classic.value(),"nights":self.nights.value()})}
 fn parse(v:&Value)->Result<Self>{keys(v,&["format","schema","scope","profile_label","classic","nights"])?;let version=v["schema"].as_u64().ok_or("Invalid save file structure")?;if v["format"]!=FORMAT||!((version==1&&v["scope"]=="unlock-progress")||(version==2&&v["scope"]=="adventure-progress")){return Err("Unsupported save file format or version".into());}text_value(&v["profile_label"])?;Ok(Self{classic:Campaign::parse(&v["classic"],false,version)?,nights:Campaign::parse(&v["nights"],true,version)?})}
 fn merged(&self,other:&Self)->Self{Self{classic:self.classic.merged(&other.classic),nights:self.nights.merged(&other.nights)}}
 fn summary(&self)->Value{json!({"classic_checkpoint":self.classic.checkpoint.map(|(s,l)|format!("{}-{}",s+1,l+1)),"nights_checkpoint":self.nights.checkpoint.map(|(s,l)|format!("{}-{}",s-10,l+1)),"classic_stage":self.classic.stage+1,"classic_level":self.classic.level+1,"nights_stage":self.nights.stage-10,"nights_level":self.nights.level+1,"trophies":self.classic.trophies.len()+self.nights.trophies.len(),"cleared":self.classic.cleared.len()+self.nights.cleared.len(),"aced":self.classic.aced.len()+self.nights.aced.len()})}
}
struct Decoder<'a>{b:&'a[u8],n:usize,spans:BTreeMap<u32,(usize,usize)>}
impl<'a> Decoder<'a>{
 fn take(&mut self,n:usize)->Result<&'a[u8]>{let end=self.n.checked_add(n).ok_or("Save file size exceeds the supported limit")?;let b=self.b.get(self.n..end).ok_or("Truncated in-game user details")?;self.n=end;Ok(b)}
 fn num(&mut self)->Result<u32>{Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))}
 fn count(&mut self)->Result<usize>{let n=self.num()?as usize;if n>1024{return Err("Unsupported in-game user collection size".into());}Ok(n)}
 fn string(&mut self)->Result<&'a[u8]>{let n=u16::from_le_bytes(self.take(2)?.try_into().unwrap())as usize;self.take(n)}
 fn field(&mut self,key:u32,kind:&str)->Result<()> {let start=self.n;match kind{
  "i"=>{self.take(4)?;},"b"=>{self.take(1)?;},"h"=>{self.take(2)?;},"ten"=>{self.take(10)?;},
  "s"=>{self.string()?;},"wide"=>{let b=self.string()?;if b.len()%4!=0{return Err("Invalid wide-string length".into());}},
  "ints"=>{let n=self.count()?;self.take(n*4)?;},"map"=>{let n=self.count()?;self.take(n*8)?;},"strings"=>{let n=self.count()?;for _ in 0..n{self.string()?;}},
  "stats"=>{if self.take(1)?!=[1]{return Err("Unsupported embedded statistics version".into());}self.take(28)?;if self.num()?!=24{return Err("Unsupported embedded statistic count".into());}self.take(96)?;},
  _=>return Err("Unknown adapter field".into())};self.spans.insert(key,(start,self.n));Ok(())}
 fn parse(b:&'a[u8])->Result<Self>{
  if b.len()as u64>LIMIT{return Err("The in-game user save file is too large for this version of the app".into());}let mut d=Self{b,n:0,spans:BTreeMap::new()};if d.num()?!=25{return Err("Unsupported in-game user details version".into());}
  for key in [0x10,0x14,0x24,0x18,0x1c,0x28,0x2c]{d.field(key,"i")?;}
  for (key,t)in [(0x84,"s"),(0x20,"i"),(0xa8,"ints"),(0x90,"ints"),(0xc0,"map"),(0x88,"wide"),(0xffff,"b"),(0x38,"i"),(0x3c,"b"),(0x180,"b"),(0x184,"b"),(0x14c,"strings"),(0x30,"i"),(0x40,"b"),(0x4c,"i"),(0x280,"i"),(0x284,"i"),(0x288,"i"),(0x188,"stats"),(0x204,"stats"),(0x50,"h"),(0x58,"ten"),(0x54,"h"),(0x144,"h"),(0x148,"h")]{d.field(key,t)?;}
  for key in (0x28c..0x2ac).step_by(4){d.field(key,"i")?;}
  for (key,t)in [(0x2ac,"b"),(0x2b0,"i"),(0xd8,"i"),(0x128,"ints"),(0xdc,"ints"),(0xe8,"ints"),(0x164,"strings"),(0x17c,"b"),(0xf8,"strings"),(0x110,"strings"),(0x140,"b"),(0x340,"i"),(0x344,"wide"),(0x348,"wide"),(0x330,"b"),(0x331,"b"),(0xf4,"i")]{d.field(key,t)?;}
  if d.n!=b.len(){return Err("Unexpected data at the end of the in-game user file; no save files were changed".into());}Ok(d)
 }
 fn field_bytes(&self,key:u32)->&'a[u8]{let (a,b)=self.spans[&key];&self.b[a..b]}
 fn int(&self,key:u32)->u32{u32::from_le_bytes(self.field_bytes(key).try_into().unwrap())}
 fn list(&self,key:u32,strings:bool)->Result<Value>{let mut d=Self{b:self.field_bytes(key),n:0,spans:BTreeMap::new()};let n=d.count()?;let mut a=Vec::new();for _ in 0..n{a.push(if strings{json!(std::str::from_utf8(d.string()?).map_err(|_|"Unsupported level-name encoding")?)}else{json!(d.num()?)});}Ok(json!(a))}
 fn campaign(&self,n:bool)->Result<Campaign>{let (stage,level,wins,trophies,cleared,aced)=if n{(0x28c,0x290,0x29c,0x128,0xf8,0x110)}else{(0x10,0x14,0x30,0xa8,0x14c,0x164)};
  let (last_stage,last_level)=if n{(0x294,0x298)}else{(0x18,0x1c)};
  let checkpoint=if self.int(last_stage)==if n{10}else{u32::MAX}{Value::Null}else{json!({"stage":self.int(last_stage),"level":self.int(last_level)})};
  Campaign::parse(&json!({"checkpoint":checkpoint,"furthest_stage":self.int(stage),"furthest_level":self.int(level),"adventure_completed":self.int(wins)>0,"trophies":self.list(trophies,false)?,"cleared_levels":self.list(cleared,true)?,"aced_levels":self.list(aced,true)?}),n,2)
 }
 fn progress(&self)->Result<Progress>{Ok(Progress{classic:self.campaign(false)?,nights:self.campaign(true)?})}
 fn apply(&self,p:&Progress)->Result<Vec<u8>>{
  if self.progress()?==*p{return Ok(self.b.to_vec());}
  let mut changes=BTreeMap::new();
  for (c,n)in [(&p.classic,false),(&p.nights,true)]{
   let (stage,level,wins,trophies,cleared,aced)=if n{(0x28c,0x290,0x29c,0x128,0xf8,0x110)}else{(0x10,0x14,0x30,0xa8,0x14c,0x164)};
   for(k,v)in [(stage,c.stage),(level,c.level),(wins,self.int(wins).max(c.completed as u32))]{changes.insert(k,v.to_le_bytes().to_vec());}
   let mut b=(c.trophies.len()as u32).to_le_bytes().to_vec();for v in &c.trophies{b.extend(v.to_le_bytes());}changes.insert(trophies,b);
   for(k,set)in [(cleared,&c.cleared),(aced,&c.aced)]{let mut b=(set.len()as u32).to_le_bytes().to_vec();for s in set{b.extend((s.len()as u16).to_le_bytes());b.extend(s.as_bytes());}changes.insert(k,b);}
   let old=self.campaign(n)?;
   if c.checkpoint>old.checkpoint{if let Some((s,l))=c.checkpoint{let (sk,lk)=if n{(0x294,0x298)}else{(0x18,0x1c)};changes.insert(sk,s.to_le_bytes().to_vec());changes.insert(lk,l.to_le_bytes().to_vec());}}
   if old.cleared==c.cleared{changes.remove(&cleared);}if old.aced==c.aced{changes.remove(&aced);}
  }
  let mut ranges=self.spans.iter().map(|(k,(a,b))|(*a,*b,*k)).collect::<Vec<_>>();ranges.sort();let mut out=self.b[..4].to_vec();for (a,b,k)in ranges{out.extend(changes.get(&k).map(Vec::as_slice).unwrap_or(&self.b[a..b]));}
  let again=Decoder::parse(&out)?;if again.progress()?!=*p{return Err("Save file conversion could not be verified".into());}
  for (k,_)in &self.spans{if !changes.contains_key(k)&&self.field_bytes(*k)!=again.field_bytes(*k){return Err("Local-only data changed during conversion".into());}}Ok(out)
 }
}
impl Manager {
 fn progress_source(&self)->Result<(Value,PathBuf,Value,BTreeMap<String,Vec<u8>>,Vec<String>,String,String,Progress)>{
  let state=self.state()?;let guest=self.active_guest(&state)?;let fp=self.fingerprint(&guest)?;let (files,dirs)=raw_import::snapshot_guest(&guest)?;
  let (_,selected,_)=self.collect(&guest)?;let idx=raw_import::index(files.get("Documents/userdata/users.dat").ok_or("Open Peggle and create an in-game user before syncing save files")?)?;
  let name=idx.iter().find(|e|e.id==selected).ok_or("Selected in-game user is missing")?.name.clone();let path=format!("Documents/userdata/user{selected}.dat");let p=Decoder::parse(files.get(&path).ok_or("In-game user details missing")?)?.progress()?;
  if self.fingerprint(&guest)?!=fp||self.state()?!=state{return Err("Local save files changed; restart before syncing".into());}Ok((state,guest,fp,files,dirs,name,path,p))
 }
 pub fn export_progress(&self)->Result<Value>{let (_,_,_,_,_,name,profile_path,p)=self.progress_source()?;let v=p.document(&name);let bytes=serde_json::to_vec_pretty(&v).map_err(err)?;let path=self.base.join("outbox").join(format!("Peggle-Save-File-{}.json",id()));write_new(&path,&bytes)?;Ok(json!({"ok":true,"path":path.to_string_lossy(),"profile_id":profile_path.strip_prefix("Documents/userdata/user").unwrap().strip_suffix(".dat").unwrap().parse::<u32>().map_err(err)?,"document":v,"profile":name,"summary":p.summary()}))}
 pub fn preview_progress_file(&mut self,path:&Path)->Result<Value>{let p=raw_import::selected_path(path)?;let b=read(&p,LIMIT)?;let v=serde_json::from_slice(&b).map_err(err)?;self.preview_progress(&v)}
 pub fn preview_progress(&mut self,v:&Value)->Result<Value>{
  if serde_json::to_vec(v).map_err(err)?.len()as u64>LIMIT{return Err("Save file exceeds size limit".into());}
  let incoming=Progress::parse(v)?;let (state,_,fp,mut files,dirs,name,path,local)=self.progress_source()?;let merged=local.merged(&incoming);
  // Validate the union as well as each input, including count and name limits.
  let document=merged.document(&name);Progress::parse(&document)?;
  let bytes=Decoder::parse(&files[&path])?.apply(&merged)?;let changed=bytes!=files[&path];
  // The native board snapshot overrides PlayerInfo on resume. Remove only the
  // selected profile/campaign snapshot when adopting a later checkpoint, in
  // the same transactional slot as the updated profile. Equal checkpoints
  // retain the current shot/board. Other profiles and game modes stay local.
  let profile_id=path.strip_prefix("Documents/userdata/user").and_then(|s|s.strip_suffix(".dat")).ok_or("Invalid selected profile path")?;
  let mut advanced_campaigns=Vec::new();
  for (campaign,old,new,prefix)in [("classic",&local.classic,&merged.classic,"stage"),("nights",&local.nights,&merged.nights,"stagenights")]{if new.checkpoint>old.checkpoint{files.remove(&format!("Documents/userdata/{prefix}{profile_id}.sav"));advanced_campaigns.push(campaign);}}
  files.insert(path,bytes);
  self.pending.clear();self.raw_pending.clear();let token=id();
  self.raw_pending.insert(token.clone(),raw_import::RawValidated{guest_files:files,dirs,state,previous_fingerprint:fp});
  Ok(json!({"ok":true,"token":token,"changed":changed,"profile":name,"source_profile":v["profile_label"],"advanced_campaigns":advanced_campaigns,"document":document,"before":local.summary(),"after":merged.summary()}))
 }
}
#[cfg(test)]#[path="progress_tests.rs"]mod tests;

/// Pure canonical merge: available during play, never reads/writes guest files.
pub fn exchange(v:&Value)->Result<Value>{
 keys(v,&["local","remote"])?;let local=Progress::parse(&v["local"])?;let remote=Progress::parse(&v["remote"])?;let merged=local.merged(&remote);let name=text_value(&v["local"]["profile_label"])?;let document=merged.document(&name);Progress::parse(&document)?;
 Ok(json!({"ok":true,"document":document,"remote_adds":merged!=local,"upload_needed":merged!=remote,"summary":merged.summary()}))
}
impl GameLease {
 /// Only called synchronously by the guest main run loop with no userdata
 /// writer open. The lease remains held; this cannot apply or select saves.
 pub fn progress_snapshot(&self,profile_id:u32)->Result<Value>{
  let guest=Manager::guest(&self.data_root);let dir=guest.join("Documents/userdata");directory(&dir)?;
  let index=raw_import::index(&read(&dir.join("users.dat"),MAX_META)?)?;let entry=index.iter().find(|e|e.id==profile_id).ok_or("Linked profile no longer exists")?;
  let bytes=read(&dir.join(format!("user{profile_id}.dat")),LIMIT)?;let p=Decoder::parse(&bytes)?.progress()?;
  Ok(json!({"ok":true,"document":p.document(&entry.name),"profile":entry.name,"profile_id":profile_id,"summary":p.summary()}))
 }
}

impl Manager {
 /// Convert one explicitly reviewed source profile to our canonical progress.
 /// Never copy scores, opaque board state or preferences across editions.
 pub fn preview_merge_selection(&mut self,paths:&[PathBuf],archive:bool)->Result<Value>{
  self.cancel_validation();
  if paths.len()==1 && paths[0].extension().and_then(|s|s.to_str()).is_some_and(|s|s.eq_ignore_ascii_case("json")){return self.preview_progress_file(&paths[0]);}
  let (files,selected)=if archive {
   if paths.len()!=1{return Err("Choose one backup ZIP".into());}
   let p=raw_import::selected_path(&paths[0])?;let(f,id,_)=self.parse_archive(&p)?;(f,Some(id))
  }else{(raw_import::progress_inputs(paths)?,None)};
  let local=self.export_progress()?;
  let entries=files.get("userdata/users.dat").map(|b|raw_import::index(b)).transpose()?;
  let candidates:Vec<_>=files.iter().filter_map(|(p,b)|p.strip_prefix("userdata/user").and_then(|s|s.strip_suffix(".dat")).and_then(|s|s.parse::<u32>().ok()).map(|id|(id,b))).collect();
  if candidates.is_empty(){return Err("Choose an in-game user’s user<ID>.dat save file, or a folder or ZIP backup containing one. Files containing only scores or unfinished boards cannot be merged.".into());}
  let source_id=if let Some(id)=selected{id}else if candidates.len()==1{candidates[0].0}else{
   entries.as_ref().and_then(|entries|entries.iter().find(|e|Some(e.name.as_str())==local["profile"].as_str())).map(|e|e.id).ok_or("Multiple source in-game users found. Choose just the user<ID>.dat file you want to merge.")?
  };
  let bytes=candidates.iter().find(|(id,_)|*id==source_id).ok_or("Selected source in-game user details are missing")?.1;
  let source_name=if let Some(entries)=entries.as_ref(){entries.iter().find(|e|e.id==source_id).ok_or("Source in-game user is not in its index")?.name.clone()}else{format!("Selected in-game user {source_id}")};
  let document=Decoder::parse(bytes)?.progress()?.document(&source_name);self.preview_progress(&document)
 }
}
