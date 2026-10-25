/* This Source Code Form is subject to the Mozilla Public License, v. 2.0. */
//! Four audited leaves with private predictions and independent branch gates.
//! No owning containers or physics callbacks are replaced by this module.
use crate::{Environment,abi::GuestFunction,cpu::CpuContext,mem::{ConstPtr,MutPtr}};
use crate::dyld::{export_c_func,FunctionExports};
use crate::{collision_geometry_kernel as rect,collision_pair_kernel as pair};
#[derive(Default)]struct Entry{original:Option<GuestFunction>,address:u32,prefix:[u8;8],disabled:bool,validating:bool,corpus:bool,shadows:[u64;4],fast:[u64;4],calls:[u64;4],total_shadows:u64,fallback:u64}
#[derive(Default)]pub(crate)struct State{entries:[Entry;4]}
struct Region{base:u32,bytes:Vec<u8>}
struct Prediction{context:CpuContext,stack:Vec<u8>,reads:Vec<Region>,writes:Vec<Region>,class:usize}
fn report(i:usize,class:usize,kind:&str,count:u64){let name=["rect_objects","rect_values","pair_key","is_collidable"][i];crate::diagnostics::event(&format!("{{\"event\":\"collision_geometry_native\",\"routine\":\"{name}\",\"class\":{class},\"kind\":\"{kind}\",\"count\":{count}}}"));}
fn equal(a:CpuContext,b:CpuContext)->bool{a.regs==b.regs&&a.extregs==b.extregs&&a.cpsr==b.cpsr&&a.fpscr==b.fpscr}
fn original(e:&mut Environment,i:usize){e.libc_state.peggle_collision_geometry.entries[i].original.unwrap().call_without_pushing_stack_frame(e);}
fn disjoint(a:u32,n:u32,b:u32,m:u32)->bool{match(a.checked_add(n),b.checked_add(m)){(Some(ae),Some(be))=>ae<=b||be<=a,_=>false}}
fn read(e:&Environment,reads:&mut Vec<Region>,base:u32,len:u32)->Option<Vec<u8>>{base.checked_add(len)?;let bytes=e.mem.get_bytes_fallible(ConstPtr::<u8>::from_bits(base).cast(),len)?.to_vec();reads.push(Region{base,bytes:bytes.clone()});Some(bytes)}
fn word(e:&Environment,reads:&mut Vec<Region>,base:u32)->Option<u32>{if base&3!=0{return None;}Some(u32::from_le_bytes(read(e,reads,base,4)?.try_into().unwrap()))}
fn inputs(e:&Environment,i:usize,c:CpuContext)->Option<Prediction>{
 let sp=c.regs[13];let len=[16,28,20,4][i];if sp<len||sp&3!=0||c.cpsr&0x0600fc00!=0{return None;}
 let stack=e.mem.get_bytes_fallible(ConstPtr::<u8>::from_bits(sp-len).cast(),len)?;let mut reads=Vec::new();let mut writes=Vec::new();
 let(context,stack,class)=match i{
  0|1=>{
   if c.regs[0]&3!=0{return None;}let a=read(e,&mut reads,c.regs[0].checked_add(20)?,16)?;let a=std::array::from_fn(|j|u32::from_le_bytes(a[j*4..j*4+4].try_into().unwrap()));
   let(b,fifth,kind)=if i==0{if c.regs[1]&3!=0{return None;}let b=read(e,&mut reads,c.regs[1].checked_add(20)?,16)?;(std::array::from_fn(|j|u32::from_le_bytes(b[j*4..j*4+4].try_into().unwrap())),0,rect::Kind::Objects)}else{let fifth=word(e,&mut reads,sp)?;([0;4],fifth,rect::Kind::Values)};
   let p=rect::calculate(c,kind,a,b,fifth,stack)?;(p.context,p.stack,p.last_comparison)
  },
  2=>{
   let left=c.regs[1];let right=c.regs[2];let dest=c.regs[0];if left&3!=0||right&3!=0||dest&3!=0{return None;}let lk=word(e,&mut reads,left.checked_add(16)?)?;
   let rk=if lk==2{word(e,&mut reads,right.checked_add(16)?)?}else{0};let(li,ri)=if lk==2&&rk==2{(word(e,&mut reads,left.checked_add(88)?)?,word(e,&mut reads,right.checked_add(88)?)?)}else{(0,0)};
   e.mem.get_bytes_fallible(ConstPtr::<u8>::from_bits(dest).cast(),8)?;let p=pair::pair(c,pair::Object{address:left,kind:lk,id:li},pair::Object{address:right,kind:rk,id:ri},stack)?;writes.push(Region{base:dest,bytes:p.pair.to_vec()});(p.context,p.stack.to_vec(),p.branch)
  },
  3=>{let b=read(e,&mut reads,c.regs[0].checked_add(36)?,1)?;let(c,stack)=pair::collidable(c,b[0],stack)?;(c,stack.to_vec(),0)},
  _=>return None
 };
 for r in &reads{if !disjoint(r.base,r.bytes.len()as u32,sp-len,len){return None;}for w in &writes{if !disjoint(r.base,r.bytes.len()as u32,w.base,w.bytes.len()as u32){return None;}}}
 for w in &writes{if !disjoint(w.base,w.bytes.len()as u32,sp-len,len){return None;}}
 Some(Prediction{context,stack,reads,writes,class})
}
fn compare(e:&mut Environment,i:usize,before:CpuContext,p:&Prediction)->bool{
 original(e,i);equal(e.cpu.visible_context(),p.context)&&e.mem.bytes_at(ConstPtr::<u8>::from_bits(before.regs[13]-p.stack.len()as u32),p.stack.len()as u32)==p.stack&&p.reads.iter().all(|r|e.mem.bytes_at(ConstPtr::<u8>::from_bits(r.base),r.bytes.len()as u32)==r.bytes)&&p.writes.iter().all(|r|e.mem.bytes_at(ConstPtr::<u8>::from_bits(r.base),r.bytes.len()as u32)==r.bytes)
}
fn disable(e:&mut Environment,i:usize,class:usize,reason:&str){let s=&mut e.libc_state.peggle_collision_geometry.entries[i];s.disabled=true;let(a,p)=(s.address,s.prefix);e.mem.bytes_at_mut(MutPtr::<u8>::from_bits(a),8).copy_from_slice(&p);e.cpu.invalidate_cache_range(a,8);report(i,class,reason,0);}
fn corpus(e:&mut Environment,i:usize)->bool{
 let saved=e.cpu.visible_context();let Some(p)=e.mem.try_alloc(4096).map(|p|p.cast::<u8>())else{return false;};let a=p.to_bits();e.libc_state.peggle_collision_geometry.entries[i].validating=true;let mut ok=true;let mut done=0;
 for n in 0..128u32{
  e.mem.bytes_at_mut(p,4096).fill(0xa5);let this=a+64;let other=a+512;let dest=a+1024;let sp=a+4000;let edges=crate::endpoint_compare_kernel::EDGES;
  let mut left=[0u32,0,2.0f32.to_bits(),2.0f32.to_bits()];let right=[1.0f32.to_bits();4];match n%8{0=>left[0]=3.0f32.to_bits(),1=>left[2]=0.5f32.to_bits(),2=>left[1]=3.0f32.to_bits(),3=>left[3]=0.5f32.to_bits(),4=>left[(n/8%4)as usize]=edges[(n/4%20)as usize],_=>()}
  for j in 0..4{e.mem.write(MutPtr::<u32>::from_bits(this+20+j*4),left[j as usize]);e.mem.write(MutPtr::<u32>::from_bits(other+20+j*4),right[j as usize]);}
  e.mem.write(MutPtr::<u32>::from_bits(this+16),if n%4==0{1}else{2});e.mem.write(MutPtr::<u32>::from_bits(other+16),if n%4==1{1}else{2});e.mem.write(MutPtr::<u32>::from_bits(this+88),if n%4==2{0x80000000}else{0x7fffffff});e.mem.write(MutPtr::<u32>::from_bits(other+88),if n%8==7{0x7fffffff}else{0});e.mem.write(MutPtr::<u8>::from_bits(this+36),n as u8);
  let mut c=saved;c.regs[0]=if i==2{dest}else{this};c.regs[1]=if i==1{right[0]}else if i==2{this}else{other};c.regs[2]=if i==1{right[1]}else{other};c.regs[3]=right[2];c.regs[13]=sp;c.cpsr=(saved.cpsr&0x0fffffff)|((n&15)<<28);c.fpscr=saved.fpscr&0xf800009f;e.mem.write(MutPtr::<u32>::from_bits(sp),right[3]);e.cpu.restore_visible_context(c);
  let Some(p)=inputs(e,i,c)else{ok=false;break;};if !compare(e,i,c,&p){ok=false;report(i,p.class,"corpus_mismatch_at",n as u64);break;}done+=1;
 }
 e.cpu.restore_visible_context(saved);e.mem.free(p.cast());e.libc_state.peggle_collision_geometry.entries[i].validating=false;report(i,255,if ok{"corpus_passed"}else{"corpus_failed"},done);ok
}
// Tiny leaves need no heap buffers after validation. Periodic/original gates
// still use the comprehensive private snapshot path below.
fn raw_word(e:&Environment,a:u32)->Option<u32>{if a&3!=0{return None;}a.checked_add(4)?;Some(u32::from_le_bytes(e.mem.get_bytes_fallible(ConstPtr::<u8>::from_bits(a).cast(),4)?.try_into().unwrap()))}
fn small_ready(e:&Environment,i:usize,class:usize)->bool{let s=&e.libc_state.peggle_collision_geometry.entries[i];s.corpus&&s.shadows[class]>=64&&s.total_shadows>=128&&(s.calls[class]+1)%4096!=0}
fn fast_pair(e:&mut Environment,c:CpuContext)->Option<usize>{
 let sp=c.regs[13];let dest=c.regs[0];let left=c.regs[1];let right=c.regs[2];if sp<20||sp&3!=0||dest&3!=0||left&3!=0||right&3!=0||c.cpsr&0x0600fc00!=0{return None;}
 let mut spans=[(0u32,0u32);4];let la=left.checked_add(16)?;let lk=raw_word(e,la)?;spans[0]=(la,4);let mut rk=0;let mut li=0;let mut ri=0;
 if lk==2{let ra=right.checked_add(16)?;rk=raw_word(e,ra)?;spans[1]=(ra,4);if rk==2{let a=left.checked_add(88)?;let b=right.checked_add(88)?;li=raw_word(e,a)?;ri=raw_word(e,b)?;spans[2]=(a,4);spans[3]=(b,4);}}
 let class=pair::key(pair::Object{address:left,kind:lk,id:li},pair::Object{address:right,kind:rk,id:ri}).2;if !small_ready(e,2,class){return None;}
 for(a,n)in spans{if n!=0&&(!disjoint(a,n,sp-20,20)||!disjoint(a,n,dest,8)){return None;}}
 if !disjoint(dest,8,sp-20,20){return None;}e.mem.get_bytes_fallible(ConstPtr::<u8>::from_bits(dest).cast(),8)?;
 let stack=e.mem.get_bytes_fallible(ConstPtr::<u8>::from_bits(sp-20).cast(),20)?;let p=pair::pair(c,pair::Object{address:left,kind:lk,id:li},pair::Object{address:right,kind:rk,id:ri},stack)?;
 e.mem.bytes_at_mut(MutPtr::<u8>::from_bits(sp-20),20).copy_from_slice(&p.stack);e.mem.bytes_at_mut(MutPtr::<u8>::from_bits(dest),8).copy_from_slice(&p.pair);e.cpu.restore_visible_context(p.context);Some(class)
}
fn fast_collidable(e:&mut Environment,c:CpuContext)->Option<usize>{
 if !small_ready(e,3,0){return None;}let sp=c.regs[13];if sp<4||sp&3!=0||c.cpsr&0x0600fc00!=0{return None;}let a=c.regs[0].checked_add(36)?;if !disjoint(a,1,sp-4,4){return None;}let byte=e.mem.get_bytes_fallible(ConstPtr::<u8>::from_bits(a).cast(),1)?[0];e.mem.get_bytes_fallible(ConstPtr::<u8>::from_bits(sp-4).cast(),4)?;
 e.mem.bytes_at_mut(MutPtr::<u8>::from_bits(sp-4),4).copy_from_slice(&c.regs[0].to_le_bytes());e.cpu.regs_mut()[0]=u32::from(byte&1);Some(0)
}
fn fast_small(e:&mut Environment,i:usize)->bool{
 let c=e.cpu.visible_context();let class=if i==2{fast_pair(e,c)}else{fast_collidable(e,c)};let Some(class)=class else{return false;};let s=&mut e.libc_state.peggle_collision_geometry.entries[i];s.calls[class]+=1;s.fast[class]+=1;let n=s.fast[class];if n==1||n%65536==0{report(i,class,"fast_calls",n);}true
}
fn leaf(e:&mut Environment,i:usize){
 let s=&e.libc_state.peggle_collision_geometry.entries[i];if s.disabled||s.validating{return original(e,i);}
 if i>=2&&fast_small(e,i){return;}
 let before=e.cpu.visible_context();let Some(p)=inputs(e,i,before)else{let s=&mut e.libc_state.peggle_collision_geometry.entries[i];s.fallback+=1;let n=s.fallback;if n==1||n%65536==0{report(i,255,"unsupported_contract_original_forwarded",n);}return original(e,i);};let class=p.class;
 if !e.libc_state.peggle_collision_geometry.entries[i].corpus{if !corpus(e,i){disable(e,i,class,"disabled_after_corpus_rejection");return original(e,i);}e.libc_state.peggle_collision_geometry.entries[i].corpus=true;}
 let s=&mut e.libc_state.peggle_collision_geometry.entries[i];s.calls[class]+=1;let shadow=s.shadows[class]<64||s.total_shadows<128||s.calls[class]%4096==0;
 if shadow{if !compare(e,i,before,&p){disable(e,i,class,"disabled_after_real_state_or_memory_mismatch");return;}let s=&mut e.libc_state.peggle_collision_geometry.entries[i];s.shadows[class]+=1;s.total_shadows+=1;if s.shadows[class]==64{report(i,class,"real_branch_gate_passed",64);}if e.libc_state.peggle_collision_geometry.entries[i].total_shadows==128{report(i,255,"real_global_gate_passed",128);}return;}
 e.mem.bytes_at_mut(MutPtr::<u8>::from_bits(before.regs[13]-p.stack.len()as u32),p.stack.len()as u32).copy_from_slice(&p.stack);for r in &p.writes{e.mem.bytes_at_mut(MutPtr::<u8>::from_bits(r.base),r.bytes.len()as u32).copy_from_slice(&r.bytes);}e.cpu.restore_visible_context(p.context);let s=&mut e.libc_state.peggle_collision_geometry.entries[i];s.fast[class]+=1;let n=s.fast[class];if n==1||n%65536==0{report(i,class,"fast_calls",n);}
}
fn rect_objects(e:&mut Environment){leaf(e,0)}fn rect_values(e:&mut Environment){leaf(e,1)}fn pair_key(e:&mut Environment){leaf(e,2)}fn is_collidable(e:&mut Environment){leaf(e,3)}
const FUNCTIONS:FunctionExports=&[export_c_func!(rect_objects()),export_c_func!(rect_values()),export_c_func!(pair_key()),export_c_func!(is_collidable())];
fn trampoline(first:&[u8],address:u32)->Vec<u8>{let n=first.len();let aligned=(n+3)&!3;let mut b=vec![0;aligned+8];b[..n].copy_from_slice(first);for j in (n..aligned).step_by(2){b[j..j+2].copy_from_slice(&0xbf00u16.to_le_bytes());}b[aligned..aligned+4].copy_from_slice(&0xf000f8dfu32.to_le_bytes());b[aligned+4..].copy_from_slice(&((address+n as u32)|1).to_le_bytes());b}
pub(crate)fn install(verified:bool,bins:&[crate::mach_o::MachO],mem:&mut crate::mem::Mem,dyld:&mut crate::dyld::Dyld,libc:&mut crate::libc::State){
 use digest::Digest;if !verified{return;}let guards=GUARDS;
 for(i,(symbol,address,length,prefix_len,hash))in guards.into_iter().enumerate(){let flag=if i==2{"PEGGLE_HD_COLLISION_PAIR_NATIVE"}else{"PEGGLE_HD_COLLISION_GEOMETRY_NATIVE"};if std::env::var(flag).as_deref()!=Ok("1"){continue;}
  if bins[0].exported_symbols.get(symbol)!=Some(&(address|1))||format!("{:x}",sha1::Sha1::digest(mem.bytes_at(ConstPtr::<u8>::from_bits(address),length)))!=hash{report(i,255,"image_guard_rejected",0);continue;}
  let prefix:[u8;8]=mem.bytes_at(ConstPtr::<u8>::from_bits(address),8).try_into().unwrap();let bytes=trampoline(mem.bytes_at(ConstPtr::<u8>::from_bits(address),prefix_len),address);let original=mem.alloc(bytes.len()as u32).cast::<u8>();mem.bytes_at_mut(original,bytes.len()as u32).copy_from_slice(&bytes);let host=dyld.create_guest_function(mem,symbol,FUNCTIONS[i].1);libc.peggle_collision_geometry.entries[i]=Entry{original:Some(GuestFunction::from_addr_with_thumb_bit(original.to_bits()|1)),address,prefix,..Entry::default()};let p=MutPtr::<u32>::from_bits(address);mem.write(p,0xf000f8dfu32);mem.write(p+1,host.addr_with_thumb_bit());report(i,255,"guarded_hook_installed",0);
 }
}
// Exact original image contracts, generated from static data by record/make_c29_geometry_fixture.py.
const GUARDS:[(&str,u32,u32,u32,&str);4]=[
 ("__ZN4Sexy7PhysObj18CheckRectCollisionEPS0_",0x61418,0x8c,8,"4e58748774f5fb3ed0e37c28c22da639d642d3cc"),
 ("__ZN4Sexy7PhysObj18CheckRectCollisionEffff",0x61810,0xac,10,"5ebce75cfb6ffd007b311c8acebece4c97e24a58"),
 ("__ZN4Sexy13CollisionPairC2EPNS_7PhysObjES2_",0x666cc,0x64,8,"19aff6ea8c02da6997e2f594cc8769e1710b2381"),
 ("__ZN4Sexy7PhysObj12IsCollidableEv",0x618bc,0x14,10,"fe13832a283ec574a42b4e608f458e3340b38135"),
];
