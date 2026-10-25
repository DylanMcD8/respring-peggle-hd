/* This Source Code Form is subject to the Mozilla Public License,v2.0. */
//! Guarded original-loop transcripts. No original exit patch, no decoder owner.
use crate::{Environment,cpu::CpuContext,abi::GuestFunction,mem::{ConstPtr,MutPtr}};
use crate::inflate_loop_kernel::{self as k,Outcome,ENTRIES,EXITS,LENGTHS};use crate::dyld::{export_c_func,FunctionExports};
#[derive(Default)]pub(crate)struct State{installed:bool,disabled:[bool;2],original:[Option<GuestFunction>;2],host:[u32;2],exit_host:[u32;2],gate:[u32;2],short:[u32;2],prefix:[[u8;10];2],active:Option<(usize,usize,u32)>,captured:Option<CpuContext>,corpus:[bool;2],shadows:[[u64;4];2],total:[u64;2],calls:[[u64;4];2],fast:[[u64;4];2]}
fn event(r:usize,class:usize,kind:&str,count:u64){crate::diagnostics::event(&format!("{{\"event\":\"inflate_loop_native\",\"routine\":{r},\"class\":{class},\"kind\":\"{kind}\",\"count\":{count}}}"));}
fn go(e:&mut Environment,a:u32){e.cpu.branch(GuestFunction::from_addr_with_thumb_bit(a));}fn tail(e:&mut Environment,r:usize){let t=e.libc_state.peggle_inflate_loops.original[r].unwrap();e.cpu.branch(t);}
fn equal(a:CpuContext,b:CpuContext)->bool{a.regs==b.regs&&a.extregs==b.extregs&&a.cpsr==b.cpsr&&a.fpscr==b.fpscr}
fn disable(e:&mut Environment,r:usize,why:&str){let s=&mut e.libc_state.peggle_inflate_loops;s.disabled[r]=true;let n=if r==0{8}else{10};e.mem.bytes_at_mut(MutPtr::<u8>::from_bits(ENTRIES[r]),n).copy_from_slice(&s.prefix[r][..n as usize]);e.cpu.invalidate_cache_range(ENTRIES[r],n);event(r,255,why,0);}
struct Input{out:Outcome,header:u32,header_before:Vec<u8>}
fn inputs(e:&Environment,mut c:CpuContext,r:usize)->Option<Input>{
 let sp=c.regs[13];if r>1||!e.mem.is_mapped_range(sp,112){return None;}c.regs[15]=ENTRIES[r];c.cpsr|=0x20;let f=e.mem.bytes_at(ConstPtr::<u8>::from_bits(sp),112);
 let(src,dst,n,header,hn)=if r==0{let k=k::word(f,0x3c);let n=if k>=20{0}else{(20-k+7)/8};let stream=k::word(f,0x4c);if !e.mem.is_mapped_range(stream,56){return None;}let h=e.mem.bytes_at(ConstPtr::<u8>::from_bits(stream),56);let start=k::word(h,0);let total=k::word(h,4);let local=k::word(f,0x34);let p=k::word(f,0x38);if total>1024*1024||local>total||start.checked_add(total-local)!=Some(p)||local<n{return None;}(p,0,n,stream,56)}else{
  let n=k::word(f,0x20);if n==0||n>258{return None;}let state=k::word(f,0x50);if !e.mem.is_mapped_range(state,56){return None;}let h=e.mem.bytes_at(ConstPtr::<u8>::from_bits(state),56);let start=k::word(h,0x28);let end=k::word(h,0x2c);let len=end.checked_sub(start)?;if len==0||len>32768||!e.mem.is_mapped_range(start,len)||!k::disjoint(start,len,sp,112)||!k::disjoint(state,56,start,len){return None;}let src=k::word(f,0x18);let dst=k::word(f,0x30);if src<start||dst<start||src.checked_add(n)?>end||dst.checked_add(n)?>end{return None;}(src,dst,n,state,56)};
 if !k::disjoint(header,hn,sp,112)||n>0&&!e.mem.is_mapped_range(src,n)||r==1&&!e.mem.is_mapped_range(dst,n){return None;}
 let state=&e.libc_state.peggle_inflate_loops;for i in 0..2{if let Some(t)=state.original[i]{for(a,m)in [(sp,112),(src,n),(dst,if r==1{n}else{0})]{if m>0&&(!k::disjoint(a,m,t.addr_without_thumb_bit(),LENGTHS[i]+8)||!k::disjoint(a,m,state.host[i],8)||!k::disjoint(a,m,state.exit_host[i],8)||state.gate[i]!=0&&!k::disjoint(a,m,state.gate[i],24)||state.short[i]!=0&&!k::disjoint(a,m,state.short[i],LENGTHS[i]+8)){return None;}}}}
 let source=if n==0{&[][..]}else{e.mem.bytes_at(ConstPtr::<u8>::from_bits(src),n)};let destination=if r==0{&[][..]}else{e.mem.bytes_at(ConstPtr::<u8>::from_bits(dst),n)};
 Some(Input{out:k::calculate(c,f,source,destination,r)?,header,header_before:e.mem.bytes_at(ConstPtr::<u8>::from_bits(header),hn).to_vec()})
}
fn finish(e:&mut Environment,r:usize){let s=&e.libc_state.peggle_inflate_loops;if s.active==Some((r,e.current_thread,e.cpu.regs()[13])){let mut c=e.cpu.visible_context();c.regs[15]=EXITS[r];c.cpsr|=0x20;e.libc_state.peggle_inflate_loops.captured=Some(c);let t=e.dyld.return_to_host_routine();e.cpu.branch(t);}else{go(e,EXITS[r]|1);}}
fn compare(e:&mut Environment,p:Input,r:usize)->bool{let sp=p.out.context.regs[13];e.libc_state.peggle_inflate_loops.active=Some((r,e.current_thread,sp));e.libc_state.peggle_inflate_loops.captured=None;tail(e,r);e.run_call();let actual=e.libc_state.peggle_inflate_loops.captured.take();e.libc_state.peggle_inflate_loops.active=None;
 let ok=actual.is_some_and(|c|equal(c,p.out.context))&&e.mem.bytes_at(ConstPtr::<u8>::from_bits(sp),112)==p.out.frame&&e.mem.bytes_at(ConstPtr::<u8>::from_bits(p.header),p.header_before.len()as u32)==p.header_before&&(p.out.source_after.is_empty()||e.mem.bytes_at(ConstPtr::<u8>::from_bits(p.out.source),p.out.source_after.len()as u32)==p.out.source_after)&&(p.out.output.is_empty()||e.mem.bytes_at(ConstPtr::<u8>::from_bits(p.out.dest),p.out.output.len()as u32)==p.out.output);
 if let Some(c)=actual{e.cpu.restore_visible_context(c);}else{panic!("inflate primitive reference missed private exit");}ok
}
fn corpus(e:&mut Environment,r:usize)->bool{let saved=e.cpu.visible_context();let Some(a)=e.mem.try_alloc(65536).map(|p|p.cast::<u8>())else{return false;};let base=a.to_bits();let(mut ok,mut done)=(true,0);
 for case in 0..128u32{for(i,x)in e.mem.bytes_at_mut(a,65536).iter_mut().enumerate(){*x=((i as u32).wrapping_mul(73)^case.wrapping_mul(0x10291))as u8;}let(sp,header,window,input)=(base+4096,base+256,base+8192,base+45056);
  let write=|e:&mut Environment,a,v|e.mem.write(MutPtr::<u32>::from_bits(a),v);
  write(e,sp+0x4c,header);write(e,sp+0x50,header);
  if r==0{let k=if case%4==0{20+case}else{case%20};write(e,sp+0x3c,k);write(e,sp+0x40,case.wrapping_mul(0x10203451));write(e,sp+0x34,8);write(e,sp+0x38,input);write(e,header,input);write(e,header+4,8);}else{let n=case%258+1;let dst=window+1024;let src=match case%4{0=>window+4096,1=>dst-1,2=>dst,_=>dst+1};write(e,sp+0x20,n);write(e,sp+0x18,src);write(e,sp+0x30,dst);write(e,header+0x28,window);write(e,header+0x2c,window+32768);}
  let mut c=saved;c.regs[13]=sp;c.cpsr=(saved.cpsr&0x0fffffff)|((case&15)<<28);e.cpu.restore_visible_context(c);let Some(p)=inputs(e,c,r)else{ok=false;break;};if !compare(e,p,r){ok=false;break;}done+=1;
 }e.cpu.restore_visible_context(saved);e.mem.free(a.cast());event(r,255,if ok{"corpus_passed"}else{"corpus_failed"},done);ok}
fn run(e:&mut Environment,r:usize){let s=&e.libc_state.peggle_inflate_loops;if s.disabled[r]||s.active.is_some()||!crate::hd_file_zlib::interpreted_stream_anchor(e){return tail(e,r);}let Some(p)=inputs(e,e.cpu.visible_context(),r)else{return tail(e,r);};let class=p.out.class;
 if !s.corpus[r]{if !corpus(e,r){disable(e,r,"disabled_after_corpus_failure");return tail(e,r);}e.libc_state.peggle_inflate_loops.corpus[r]=true;}
 let s=&mut e.libc_state.peggle_inflate_loops;s.calls[r][class]+=1;
 if s.total[r]<128||s.shadows[r][class]<32||s.calls[r][class]%1024==0{if !compare(e,p,r){disable(e,r,"disabled_after_real_mismatch");}else{let s=&mut e.libc_state.peggle_inflate_loops;s.total[r]+=1;s.shadows[r][class]+=1;if s.total[r]==128{event(r,255,"real_global_gate_passed",128);}if s.shadows[r][class]==32{event(r,class,"real_class_gate_passed",32);}}go(e,EXITS[r]|1);return;}
 if !p.out.output.is_empty(){e.mem.bytes_at_mut(MutPtr::<u8>::from_bits(p.out.dest),p.out.output.len()as u32).copy_from_slice(&p.out.output);}e.mem.bytes_at_mut(MutPtr::<u8>::from_bits(p.out.context.regs[13]),112).copy_from_slice(&p.out.frame);e.cpu.restore_visible_context(p.out.context);go(e,EXITS[r]|1);let s=&mut e.libc_state.peggle_inflate_loops;s.fast[r][class]+=1;let n=s.fast[r][class];if n==1||n%65536==0{event(r,class,"fast_calls",n);}
}
fn refill(e:&mut Environment){run(e,0)}fn copy(e:&mut Environment){run(e,1)}fn refill_exit(e:&mut Environment){finish(e,0)}fn copy_exit(e:&mut Environment){finish(e,1)}
const FUNCTIONS:FunctionExports=&[export_c_func!(refill()),export_c_func!(copy()),export_c_func!(refill_exit()),export_c_func!(copy_exit())];
pub(crate)fn install(verified:bool,bins:&[crate::mach_o::MachO],mem:&mut crate::mem::Mem,dyld:&mut crate::dyld::Dyld,libc:&mut crate::libc::State){use digest::Digest;
 if !verified||std::env::var("PEGGLE_HD_INFLATE_LOOPS_NATIVE").as_deref()!=Ok("1"){return;}
 if bins.first().and_then(|b|b.exported_symbols.get("_inflate_fast"))!=Some(&0x28b595)||!mem.is_mapped_range(0x28b594,1540)||format!("{:x}",sha1::Sha1::digest(mem.bytes_at(ConstPtr::<u8>::from_bits(0x28b594),1540)))!="50ddd905ccf384977a7e36a0c0864adf817455e4"{event(255,255,"image_guard_rejected",0);return;}
 install_entries(mem,dyld,libc,&[1]);
}
fn install_entries(mem:&mut crate::mem::Mem,dyld:&mut crate::dyld::Dyld,libc:&mut crate::libc::State,routines:&[usize]){
 for &r in routines{let original=mem.bytes_at(ConstPtr::<u8>::from_bits(ENTRIES[r]),LENGTHS[r]).to_vec();let exit=dyld.create_guest_function(mem,FUNCTIONS[r+2].0,FUNCTIONS[r+2].1);let tr=mem.alloc(LENGTHS[r]+8).cast::<u8>();mem.bytes_at_mut(tr,LENGTHS[r]).copy_from_slice(&original);mem.write(MutPtr::<u32>::from_bits(tr.to_bits()+LENGTHS[r]),0xf000f8dfu32);mem.write(MutPtr::<u32>::from_bits(tr.to_bits()+LENGTHS[r]+4),exit.addr_with_thumb_bit());let host=dyld.create_guest_function(mem,FUNCTIONS[r].0,FUNCTIONS[r].1);let s=&mut libc.peggle_inflate_loops;s.installed=true;s.original[r]=Some(GuestFunction::from_addr_with_thumb_bit(tr.to_bits()|1));s.host[r]=host.addr_without_thumb_bit();s.exit_host[r]=exit.addr_without_thumb_bit();
  // Short LZ matches remain entirely in guest code. r0 and NZCV are dead
  // on entry: the copied loop overwrites r0 and ends in CMP(count,0).
  // The private reference trampoline always exits through its capture helper;
  // this separate short trampoline exits directly to the untouched continuation.
  let target=if r==1{let short=mem.alloc(LENGTHS[r]+8).cast::<u8>();mem.bytes_at_mut(short,LENGTHS[r]).copy_from_slice(&original);mem.write(MutPtr::<u32>::from_bits(short.to_bits()+LENGTHS[r]),0xf000f8dfu32);mem.write(MutPtr::<u32>::from_bits(short.to_bits()+LENGTHS[r]+4),EXITS[r]|1);let gate=mem.alloc(24).cast::<u8>();
   for(offset,half)in [(0,0x9808u16),(2,0x2810),(4,0xd304),(6,0xf8df),(8,0xf004),(10,0xbf00),(16,0xf8df),(18,0xf000)]{mem.write(MutPtr::<u16>::from_bits(gate.to_bits()+offset),half);}mem.write(MutPtr::<u32>::from_bits(gate.to_bits()+12),host.addr_with_thumb_bit());mem.write(MutPtr::<u32>::from_bits(gate.to_bits()+20),short.to_bits()|1);s.gate[r]=gate.to_bits();s.short[r]=short.to_bits();gate.to_bits()|1
  }else{host.addr_with_thumb_bit()};let n=if r==0{8}else{10};s.prefix[r][..n].copy_from_slice(&original[..n]);
  mem.write(MutPtr::<u32>::from_bits(ENTRIES[r]),if r==0{0xf000f8dfu32}else{0xf004f8dfu32});if r==0{mem.write(MutPtr::<u32>::from_bits(ENTRIES[r]+4),host.addr_with_thumb_bit());}else{mem.write(MutPtr::<u16>::from_bits(ENTRIES[r]+4),0xbf00u16);mem.write(MutPtr::<u32>::from_bits(ENTRIES[r]+6),target);}event(r,255,"guarded_loop_installed_anchor_required",0);
 }
}
