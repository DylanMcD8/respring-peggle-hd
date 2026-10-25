/* This Source Code Form is subject to the Mozilla Public License, v. 2.0. */
//! Bounded private sort prediction plus passive instruction-boundary observer.
//! This module NEVER writes guest memory/registers, calls guest code or enables
//! a native sort. Original C++ tree operations execute exactly once as usual.
use crate::{cpu::CpuContext,mem::{ConstPtr,Mem},collision_pair_kernel::{self as pair,Object},endpoint_compare_kernel as fp};
use std::collections::BTreeMap;
pub(crate)const ENTRY:u32=0x6019c;
const MAX_ENDPOINTS:usize=256;const MAX_EVENTS:usize=65536;
#[derive(Clone,Debug,PartialEq,Eq)]pub(crate)enum Event{
 Size(u32),Compare{left:usize,right:usize,a:u32,b:u32,less:bool},Rect{left:u32,right:u32,hit:bool},
 Construct{left:u32,right:u32,first:u32,second:u32},Erase(u32,u32),Insert(u32,u32),Swap{left:usize,right:usize,a:[u8;12],b:[u8;12]}
}
#[derive(Clone)]pub(crate)struct Endpoint{pub bytes:[u8;12],pub coordinate:u32}
#[derive(Clone,Copy)]pub(crate)struct Phys{pub object:Object,pub flag:u8,pub rect:[u32;4]}
pub(crate)struct Plan{pub endpoints:Vec<Endpoint>,pub events:Vec<Event>}
fn number(b:&[u8])->u32{u32::from_le_bytes(b.try_into().unwrap())}
fn push(events:&mut Vec<Event>,event:Event)->Option<()>{if events.len()>=MAX_EVENTS{return None;}events.push(event);Some(())}
fn overlap(a:[u32;4],b:[u32;4],cpsr:u32,fpscr:u32)->Option<bool>{
 for(index,(x,y))in[(a[0],b[2]),(a[2],b[0]),(a[1],b[3]),(a[3],b[1])].into_iter().enumerate(){let p=fp::compare(x,y,cpsr,fpscr)?;let f=p.cpsr;let hi=f&0x20000000!=0&&f&0x40000000==0;let lt=(f>>31)&1!=(f>>28)&1;if if index==0||index==2{hi}else{lt}{return Some(false);}}Some(true)
}
pub(crate)fn predict(mut endpoints:Vec<Endpoint>,objects:&BTreeMap<u32,Phys>,cpsr:u32,fpscr:u32)->Option<Plan>{
 if endpoints.len()>MAX_ENDPOINTS||!fp::supported(cpsr,fpscr){return None;}let mut events=Vec::new();push(&mut events,Event::Size(endpoints.len()as u32))?;
 for i in 1..endpoints.len(){let mut j=i;while j>0{
  let l=&endpoints[j-1];let r=&endpoints[j];let less=fp::compare(l.coordinate,r.coordinate,cpsr,fpscr)?.less!=0;push(&mut events,Event::Compare{left:j-1,right:j,a:l.coordinate,b:r.coordinate,less})?;if less{break;}
  let left_flag=l.bytes[8]&1!=0;let right_flag=r.bytes[8]&1!=0;
  if left_flag!=right_flag{
   let la=number(&l.bytes[..4]);let ra=number(&r.bytes[..4]);let a=*objects.get(&la)?;let b=*objects.get(&ra)?;
   let eligible=if a.object.kind==2{b.object.kind!=2||(a.flag&1!=0&&b.flag&1!=0)}else{b.object.kind==2};
   if eligible{
    let hit=if left_flag{true}else{let hit=overlap(a.rect,b.rect,cpsr,fpscr)?;push(&mut events,Event::Rect{left:la,right:ra,hit})?;hit};
    if hit{let(first,second,_)=pair::key(a.object,b.object);push(&mut events,Event::Construct{left:la,right:ra,first,second})?;push(&mut events,if left_flag{Event::Erase(first,second)}else{Event::Insert(first,second)})?;}
   }
  }
  push(&mut events,Event::Swap{left:j-1,right:j,a:l.bytes,b:r.bytes})?;endpoints.swap(j-1,j);j-=1;
 }}Some(Plan{endpoints,events})
}
struct Region{base:u32,bytes:Vec<u8>}
struct Capture{plan:Plan,base:u32,vector:u32,set:u32,reads:Vec<Region>,return_pc:u32,sp:u32}
fn bytes(mem:&Mem,a:u32,n:u32)->Option<Vec<u8>>{a.checked_add(n)?;Some(mem.get_bytes_fallible(ConstPtr::<u8>::from_bits(a).cast(),n)?.to_vec())}
fn disjoint(a:u32,n:u32,b:u32,m:u32)->bool{match(a.checked_add(n),b.checked_add(m)){(Some(ae),Some(be))=>ae<=b||be<=a,_=>false}}
fn source(mem:&Mem,reads:&mut Vec<Region>,a:u32,n:u32,base:u32,len:u32,sp:u32)->Option<Vec<u8>>{if !disjoint(a,n,base,len)||!disjoint(a,n,sp-4096,4112){return None;}let b=bytes(mem,a,n)?;reads.push(Region{base:a,bytes:b.clone()});Some(b)}
fn capture(c:CpuContext,mem:&Mem)->Option<Capture>{
 let sp=c.regs[13];let vector=c.regs[1];if sp<4096||sp&3!=0||vector&3!=0||!fp::supported(c.cpsr,c.fpscr){return None;}let header=bytes(mem,vector,12)?;let base=number(&header[..4]);let end=number(&header[4..8]);let capacity=number(&header[8..]);let len=end.checked_sub(base)?;if base&3!=0||len%12!=0||len/12>MAX_ENDPOINTS as u32||end>capacity{return None;}let mut reads=vec![];source(mem,&mut reads,vector,12,base,len,sp)?;
 if !disjoint(base,len,sp-4096,4112){return None;}let raw=if len==0{Vec::new()}else{bytes(mem,base,len)?};let mut endpoints=Vec::new();let mut objects=BTreeMap::new();
 for b in raw.chunks_exact(12){let mut value=[0u8;12];value.copy_from_slice(b);let p=number(&b[4..8]);if p&3!=0{return None;}let coordinate=number(&source(mem,&mut reads,p,4,base,len,sp)?);let object=number(&b[..4]);
  if let std::collections::btree_map::Entry::Vacant(entry)=objects.entry(object){if object&3!=0{return None;}let kind=number(&source(mem,&mut reads,object.checked_add(16)?,4,base,len,sp)?);let(id,flag)=if kind==2{(number(&source(mem,&mut reads,object.checked_add(88)?,4,base,len,sp)?),source(mem,&mut reads,object.checked_add(293)?,1,base,len,sp)?[0])}else{(0,0)};let rect=source(mem,&mut reads,object.checked_add(20)?,16,base,len,sp)?;entry.insert(Phys{object:Object{address:object,kind,id},flag,rect:std::array::from_fn(|j|number(&rect[j*4..j*4+4]))});}
  endpoints.push(Endpoint{bytes:value,coordinate});
 }
 let plan=predict(endpoints,&objects,c.cpsr,c.fpscr)?;Some(Capture{plan,base,vector,set:c.regs[0].checked_add(28)?,reads,return_pc:c.regs[14]&!1,sp})
}
#[derive(Clone)]enum Pending{Size,Compare{left:usize,right:usize,a:u32,b:u32},Rect{left:u32,right:u32},Construct{left:u32,right:u32,dest:u32}}
struct Active{capture:Capture,events:Vec<Event>,pending:Option<(u32,Pending)>}
/// Observer state belongs to the CPU, not guest memory. Context switches cancel
/// observation so events from different guest threads cannot be mixed.
pub(crate)struct Observer{active:Option<Active>,disabled:bool,pub completed:u64,pub passed:u64,pub rejected:u64,pub last_equal:Option<bool>,coverage:[u64;7]}
impl Default for Observer{fn default()->Self{Self{active:None,disabled:false,completed:0,passed:0,rejected:0,last_equal:None,coverage:[0;7]}}}
impl Observer{
 fn report(kind:&str,count:u64){crate::diagnostics::event(&format!("{{\"event\":\"collision_sort_observer\",\"kind\":\"{kind}\",\"count\":{count},\"read_only\":true,\"native_sort_enabled\":false}}"));}
 pub fn wants_pc(&self,pc:u32,sp:u32)->bool{self.enabled()&&(pc==ENTRY||self.active.as_ref().is_some_and(|a|(ENTRY..0x60364).contains(&pc)||(pc==a.capture.return_pc&&sp==a.capture.sp)))}
 pub fn enabled(&self)->bool{!self.disabled&&self.completed<64}
 pub fn cancel(&mut self,reason:&str){if self.active.take().is_some(){self.rejected+=1;Self::report(reason,self.rejected);}}
 fn reject(&mut self,reason:&str){self.cancel(reason);self.disabled=true;self.last_equal=Some(false);}
 pub fn tick(&mut self,c:CpuContext,mem:&Mem){
  if !self.enabled(){return;}let pc=c.regs[15];
  if pc==ENTRY{if self.active.is_some(){self.reject("reentrant_observation_disabled");return;}if let Some(capture)=capture(c,mem){self.active=Some(Active{capture,events:Vec::new(),pending:None});}else{self.rejected+=1;if self.rejected==1{Self::report("unsupported_snapshot_original_unchanged",self.rejected);}}return;}
  let Some(a)=self.active.as_mut()else{return;};
  if pc==a.capture.return_pc&&c.regs[13]==a.capture.sp{
   let a=self.active.take().unwrap();let n=(a.capture.plan.endpoints.len()*12)as u32;let actual=if n==0{Some(Vec::new())}else{bytes(mem,a.capture.base,n)};let expected=a.capture.plan.endpoints.iter().flat_map(|e|e.bytes).collect::<Vec<_>>();let equal=a.pending.is_none()&&a.events==a.capture.plan.events&&actual.as_deref()==Some(expected.as_slice())&&a.capture.reads.iter().all(|r|bytes(mem,r.base,r.bytes.len()as u32).as_deref()==Some(r.bytes.as_slice()));
   self.completed+=1;self.last_equal=Some(equal);if equal{self.passed+=1;for event in &a.events{let k=match event{Event::Size(_)=>0,Event::Compare{..}=>1,Event::Rect{..}=>2,Event::Construct{..}=>3,Event::Erase(..)=>4,Event::Insert(..)=>5,Event::Swap{..}=>6};self.coverage[k]+=1;}if self.passed==1||self.passed%16==0{Self::report("ordered_trace_and_endpoints_match",self.passed);let counts=self.coverage.iter().map(|v|v.to_string()).collect::<Vec<_>>().join(",");crate::diagnostics::event(&format!("{{\"event\":\"collision_sort_observer_coverage\",\"passed\":{},\"event_order\":\"size,compare,rect,construct,erase,insert,swap\",\"counts\":[{counts}],\"native_sort_enabled\":false,\"tree_structure_validated\":false}}",self.passed));}}else{self.rejected+=1;self.disabled=true;Self::report("trace_or_endpoint_or_readonly_source_mismatch",self.rejected);}return;
  }
  if let Some((ret,p))=a.pending.clone(){if pc==ret{a.pending=None;let event=match p{Pending::Size=>Event::Size(c.regs[0]),Pending::Compare{left,right,a,b}=>Event::Compare{left,right,a,b,less:c.regs[0]&1!=0},Pending::Rect{left,right}=>Event::Rect{left,right,hit:c.regs[0]&1!=0},Pending::Construct{left,right,dest}=>{let Some(b)=bytes(mem,dest,8)else{self.reject("constructor_result_unmapped");return;};Event::Construct{left,right,first:number(&b[..4]),second:number(&b[4..])}}};if push(&mut a.events,event).is_none(){self.reject("trace_capacity_exceeded");}return;}}
  if !(ENTRY..0x60364).contains(&pc){return;}
  let slots=|left:u32,right:u32|->Option<(usize,usize)>{let l=left.checked_sub(a.capture.base)?;let r=right.checked_sub(a.capture.base)?;let len=a.capture.plan.endpoints.len()*12;if l%12!=0||r%12!=0||l as usize>=len||r as usize>=len{return None;}Some((l as usize/12,r as usize/12))};
  let pending=match pc{
   0x601ae=>Some((0x601b2,Pending::Size)),
   0x601ec=>{let Some((left,right))=slots(c.regs[0],c.regs[1])else{self.reject("compare_slot_guard");return;};let Some(l)=bytes(mem,c.regs[0]+4,4)else{self.reject("compare_left_unmapped");return;};let Some(r)=bytes(mem,c.regs[1]+4,4)else{self.reject("compare_right_unmapped");return;};let(Some(l),Some(r))=(bytes(mem,number(&l),4),bytes(mem,number(&r),4))else{self.reject("coordinate_unmapped");return;};Some((0x601f0,Pending::Compare{left,right,a:number(&l),b:number(&r)}))},
   0x602d6|0x60312=>Some((pc+4,Pending::Rect{left:c.regs[0],right:c.regs[1]})),
   0x60250|0x6027c|0x602f0|0x6032c=>Some((pc+4,Pending::Construct{left:c.regs[1],right:c.regs[2],dest:c.regs[0]})),
   0x6025c|0x60288=>{if c.regs[0]!=a.capture.set{self.reject("erase_set_identity");return;}let Some(b)=bytes(mem,c.regs[1],8)else{self.reject("erase_key_unmapped");return;};if push(&mut a.events,Event::Erase(number(&b[..4]),number(&b[4..]))).is_none(){self.reject("trace_capacity_exceeded");return;}None},
   0x602fe|0x6033a=>{if c.regs[1]!=a.capture.set{self.reject("insert_set_identity");return;}let Some(b)=bytes(mem,c.regs[2],8)else{self.reject("insert_key_unmapped");return;};if push(&mut a.events,Event::Insert(number(&b[..4]),number(&b[4..]))).is_none(){self.reject("trace_capacity_exceeded");return;}None},
   0x60348=>{let Some((left,right))=slots(c.regs[0],c.regs[1])else{self.reject("swap_slot_guard");return;};let(Some(l),Some(r))=(bytes(mem,c.regs[0],12),bytes(mem,c.regs[1],12))else{self.reject("swap_data_unmapped");return;};if push(&mut a.events,Event::Swap{left,right,a:l.try_into().unwrap(),b:r.try_into().unwrap()}).is_none(){self.reject("trace_capacity_exceeded");return;}None},
   _=>None
  };if let Some(p)=pending{if a.pending.is_some(){self.reject("nested_observed_operation");return;}a.pending=Some(p);}
 }
}

pub(crate)fn image_verified(verified:bool,bins:&[crate::mach_o::MachO],mem:&Mem)->bool{
 use digest::Digest;if !verified||std::env::var("PEGGLE_HD_COLLISION_SORT_TRACE").as_deref()!=Ok("1"){return false;}
 let symbol="__ZN4Sexy12CollisionMgr13SortEndPointsERSt6vectorINS0_8EndPointESaIS2_EE";
 bins[0].exported_symbols.get(symbol)==Some(&(ENTRY|1))&&format!("{:x}",sha1::Sha1::digest(mem.bytes_at(ConstPtr::<u8>::from_bits(ENTRY),0x1c8)))=="2980990398adb88da54741d91913c9bb81a6f418"
}
