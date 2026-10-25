/* This Source Code Form is subject to the Mozilla Public License, v. 2.0. */
//! Exact integer JPEG color leaf, independent opt-in and original-state gates.
use crate::{Environment,abi::GuestFunction,cpu::CpuContext,mem::{ConstPtr,MutPtr}};
use crate::dyld::{export_c_func,FunctionExports};
use crate::jpeg_ycc_kernel::{self as kernel,Region,Outcome};
#[derive(Default)]pub(crate)struct State{original:Option<GuestFunction>,address:u32,prefix:[u8;8],disabled:bool,validating:bool,corpus:bool,shadows:[u64;4],fast:[u64;4],calls:[u64;4],total:u64,fallback:u64}
struct Prediction{out:Outcome,class:usize}
fn report(class:usize,kind:&str,count:u64){crate::diagnostics::event(&format!("{{\"event\":\"jpeg_ycc_native\",\"class\":{class},\"kind\":\"{kind}\",\"count\":{count}}}"));}
fn equal(a:CpuContext,b:CpuContext)->bool{a.regs==b.regs&&a.extregs==b.extregs&&a.cpsr==b.cpsr&&a.fpscr==b.fpscr}
fn original(e:&mut Environment){e.libc_state.peggle_jpeg_ycc.original.unwrap().call_without_pushing_stack_frame(e);}
fn disjoint(a:u32,n:u32,b:u32,m:u32)->bool{match(a.checked_add(n),b.checked_add(m)){(Some(ae),Some(be))=>ae<=b||be<=a,_=>false}}
fn add(e:&Environment,r:&mut Vec<Region>,base:u32,len:u32,writable:bool)->Option<usize>{base.checked_add(len)?;if !e.mem.is_mapped_range(base,len){return None;}let bytes=e.mem.get_bytes_fallible(ConstPtr::<u8>::from_bits(base).cast(),len)?.to_vec();r.push(Region{base,bytes,writable});Some(r.len()-1)}
fn word(e:&Environment,a:u32)->Option<u32>{if a&3!=0{return None;}a.checked_add(4)?;if !e.mem.is_mapped_range(a,4){return None;}Some(u32::from_le_bytes(e.mem.get_bytes_fallible(ConstPtr::<u8>::from_bits(a).cast(),4)?.try_into().unwrap()))}
fn at(b:&[u8],i:usize)->u32{u32::from_le_bytes(b[i..i+4].try_into().unwrap())}
fn inputs(e:&Environment,c:CpuContext)->Option<Prediction>{
 let sp=c.regs[13];if sp<80||sp&3!=0||c.cpsr&0x0600fc00!=0||c.regs[0]&3!=0{return None;}
 let rows=word(e,sp)?;if rows>8{return None;} // Negative and oversized signed counts remain original.
 let mut r=Vec::with_capacity(40);add(e,&mut r,sp-80,80,true)?;add(e,&mut r,sp,4,false)?;
 let ci=add(e,&mut r,c.regs[0],0x1cc,false)?;let width=at(&r[ci].bytes,0x70);if width>4096||width.checked_mul(rows)?.checked_mul(3)?>98304{return None;}
 let range=at(&r[ci].bytes,0x144);let state=at(&r[ci].bytes,0x1c8);if state&3!=0{return None;}let st=add(e,&mut r,state,24,false)?;
 for i in 0..4{let a=at(&r[st].bytes,8+i*4);if a&3!=0{return None;}let id=add(e,&mut r,a,1024,false)?;for b in r[id].bytes.chunks_exact(4){let v=i32::from_le_bytes(b.try_into().unwrap());if if i<2{!(-1024..=1023).contains(&v)}else{!(-33554432..=33554431).contains(&v)}{return None;}}}
 add(e,&mut r,range.checked_sub(1024)?,2304,false)?;
 if rows!=0{
  if c.regs[1]&3!=0||c.regs[3]&3!=0{return None;}let pp=add(e,&mut r,c.regs[1],12,false)?;let index=c.regs[2].checked_mul(4)?;let n=rows.checked_mul(4)?;
  let mut pointers=Vec::with_capacity(4);for i in 0..3{let a=at(&r[pp].bytes,i*4).checked_add(index)?;if a&3!=0{return None;}let id=add(e,&mut r,a,n,false)?;pointers.push(id);}let dest=add(e,&mut r,c.regs[3],n,false)?;
  for row in 0..rows as usize{for id in &pointers{let a=at(&r[*id].bytes,row*4);if width!=0{add(e,&mut r,a,width,false)?;}}let a=at(&r[dest].bytes,row*4);if width!=0{add(e,&mut r,a,width*3,true)?;}}
 }
 for(i,a)in r.iter().enumerate(){if !a.writable{continue;}for(j,b)in r.iter().enumerate(){if i!=j&&!disjoint(a.base,a.bytes.len()as u32,b.base,b.bytes.len()as u32){return None;}}}
 let class=if rows==0{0}else if width==0{1}else if rows==1{2}else{3};let out=kernel::calculate(c,r,width,rows)?;Some(Prediction{out,class})
}
fn compare(e:&mut Environment,p:&Prediction)->bool{original(e);equal(e.cpu.visible_context(),p.out.context)&&p.out.regions.iter().all(|r|e.mem.bytes_at(ConstPtr::<u8>::from_bits(r.base),r.bytes.len()as u32)==r.bytes)}
fn disable(e:&mut Environment,class:usize,reason:&str){let s=&mut e.libc_state.peggle_jpeg_ycc;s.disabled=true;let(a,p)=(s.address,s.prefix);e.mem.bytes_at_mut(MutPtr::<u8>::from_bits(a),8).copy_from_slice(&p);e.cpu.invalidate_cache_range(a,8);report(class,reason,0);}
// Self-contained synthetic state only: original leaf has no callbacks/allocations.
// No live decoder/caller buffer is modified or passed to the corpus.
fn corpus(e:&mut Environment)->bool{
 let saved=e.cpu.visible_context();let Some(p)=e.mem.try_alloc(32768).map(|p|p.cast::<u8>())else{return false;};let a=p.to_bits();e.libc_state.peggle_jpeg_ycc.validating=true;let mut ok=true;let mut done=0;
 for n in 0..128u32{
  e.mem.bytes_at_mut(p,32768).fill((n*73)as u8);let ci=a+256;let st=a+1024;let range=a+8192;let pp=a+10000;let dp=a+10400;let sp=a+32000;let width=if n%4==1{0}else{1+(n%65)};let rows=if n%4==0{0}else if n%4==3{2}else{1};
  for(off,v)in [(0x70,width),(0x144,range),(0x1c8,st)]{e.mem.write(MutPtr::<u32>::from_bits(ci+off),v);}for t in 0..4{let table=a+2048+t*1024;e.mem.write(MutPtr::<u32>::from_bits(st+8+t*4),table);for j in 0..256u32{let x=j as i32-128;let v=match t{0=>(91881*x+32768)>>16,1=>(116130*x+32768)>>16,2=>-46802*x,_=>-22554*x+32768};e.mem.write(MutPtr::<u32>::from_bits(table+j*4),v as u32);}}
  for j in 0..2304{let v=(j as i32-1024).clamp(0,255)as u8;e.mem.write(MutPtr::<u8>::from_bits(range-1024+j),v);}for t in 0..3{let arr=a+11000+t*128;e.mem.write(MutPtr::<u32>::from_bits(pp+t*4),arr);for row in 0..2{let b=a+12000+t*1024+row*256;e.mem.write(MutPtr::<u32>::from_bits(arr+row*4),b);for j in 0..width{e.mem.write(MutPtr::<u8>::from_bits(b+j),(j*37+t*71+n)as u8);}}}for row in 0..2{e.mem.write(MutPtr::<u32>::from_bits(dp+row*4),a+17000+row*512);}
  let mut c=saved;for(i,v)in c.regs.iter_mut().enumerate(){*v=n.wrapping_mul(0x9e3779b9).wrapping_add(i as u32*73);}c.regs[0]=ci;c.regs[1]=pp;c.regs[2]=0;c.regs[3]=dp;c.regs[13]=sp;c.regs[14]=saved.regs[14];c.regs[15]=saved.regs[15];c.cpsr=(saved.cpsr&!0xf600fc00)|((n&15)<<28);c.fpscr=saved.fpscr;e.mem.write(MutPtr::<u32>::from_bits(sp),rows);e.cpu.restore_visible_context(c);
  let Some(p)=inputs(e,c)else{ok=false;break;};if !compare(e,&p){ok=false;report(p.class,"corpus_mismatch_at",n as u64);break;}done+=1;
 }
 e.cpu.restore_visible_context(saved);e.mem.free(p.cast());e.libc_state.peggle_jpeg_ycc.validating=false;report(255,if ok{"corpus_passed"}else{"corpus_failed"},done);ok
}
fn jpeg_ycc(e:&mut Environment){
 let s=&e.libc_state.peggle_jpeg_ycc;if s.disabled||s.validating{return original(e);}let before=e.cpu.visible_context();let Some(p)=inputs(e,before)else{let s=&mut e.libc_state.peggle_jpeg_ycc;s.fallback+=1;let n=s.fallback;if n==1||n%65536==0{report(255,"unsupported_contract_original_forwarded",n);}return original(e);};let class=p.class;
 if !e.libc_state.peggle_jpeg_ycc.corpus{if !corpus(e){disable(e,class,"disabled_after_corpus_rejection");return original(e);}e.libc_state.peggle_jpeg_ycc.corpus=true;}
 let s=&mut e.libc_state.peggle_jpeg_ycc;s.calls[class]+=1;let shadow=s.shadows[class]<64||s.total<128||s.calls[class]%4096==0;
 if shadow{if !compare(e,&p){disable(e,class,"disabled_after_real_state_or_memory_mismatch");return;}let s=&mut e.libc_state.peggle_jpeg_ycc;s.shadows[class]+=1;s.total+=1;if s.shadows[class]==64{report(class,"real_branch_gate_passed",64);}if s.total==128{report(255,"real_global_gate_passed",128);}return;}
 for r in &p.out.regions{if r.writable{e.mem.bytes_at_mut(MutPtr::<u8>::from_bits(r.base),r.bytes.len()as u32).copy_from_slice(&r.bytes);}}e.cpu.restore_visible_context(p.out.context);let s=&mut e.libc_state.peggle_jpeg_ycc;s.fast[class]+=1;let n=s.fast[class];if n==1||n%65536==0{report(class,"fast_calls",n);}
}
const FUNCTIONS:FunctionExports=&[export_c_func!(jpeg_ycc())];
fn trampoline(first:&[u8],address:u32)->Vec<u8>{let mut b=first.to_vec();b.extend_from_slice(&0xf000f8dfu32.to_le_bytes());b.extend_from_slice(&((address+8)|1).to_le_bytes());b}
pub(crate)fn install(verified:bool,bins:&[crate::mach_o::MachO],mem:&mut crate::mem::Mem,dyld:&mut crate::dyld::Dyld,libc:&mut crate::libc::State){
 use digest::Digest;if !verified||std::env::var("PEGGLE_HD_JPEG_YCC_NATIVE").as_deref()!=Ok("1")||bins.is_empty(){return;}let address=0x2a690c;let symbol="_ycc_rgb_convert";
 if bins[0].exported_symbols.get(symbol)!=Some(&(address|1))||mem.get_bytes_fallible(ConstPtr::<u8>::from_bits(address).cast(),292).is_none(){report(255,"image_guard_rejected",0);return;}
 let body=mem.bytes_at(ConstPtr::<u8>::from_bits(address),292);if format!("{:x}",sha1::Sha1::digest(body))!="b424c7b822e4ef9599d4a567cbe810015891a926"||body[..8]!=[0x94,0xb0,0xdd,0xf8,0x50,0x90,0x13,0x90]{report(255,"image_guard_rejected",0);return;}
 let prefix=body[..8].try_into().unwrap();let bytes=trampoline(&body[..8],address);let original=mem.alloc(bytes.len()as u32).cast::<u8>();mem.bytes_at_mut(original,bytes.len()as u32).copy_from_slice(&bytes);let host=dyld.create_guest_function(mem,symbol,FUNCTIONS[0].1);libc.peggle_jpeg_ycc=State{original:Some(GuestFunction::from_addr_with_thumb_bit(original.to_bits()|1)),address,prefix,..State::default()};mem.write(MutPtr::<u32>::from_bits(address),0xf000f8dfu32);mem.write(MutPtr::<u32>::from_bits(address+4),host.addr_with_thumb_bit());report(255,"guarded_hook_installed",0);
}
