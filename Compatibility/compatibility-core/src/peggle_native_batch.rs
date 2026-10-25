/* This Source Code Form is subject to the Mozilla Public License,v2.0. */
//! C35 guarded loading/UI batch. No persistent cache or owning tree mutation.
use crate::{Environment,cpu::CpuContext,abi::GuestFunction,mem::{ConstPtr,MutPtr}};
use crate::dyld::{export_c_func,FunctionExports};use crate::native_batch_kernel::{self as k,Outcome};
#[derive(Default)]struct Hook{original:Option<GuestFunction>,address:u32,host:u32,prefix:[u8;8],disabled:bool,validating:bool,corpus:bool,total:u64,shadows:[u64;16],calls:[u64;16],fast:[u64;16]}
#[derive(Default)]pub(crate)struct State{guard_min:u32,guard_max:u32,hooks:[Hook;15]}
fn event(r:usize,class:usize,kind:&str,count:u64){crate::diagnostics::event(&format!("{{\"event\":\"native_ui_loading_batch\",\"routine\":{r},\"class\":{class},\"kind\":\"{kind}\",\"count\":{count}}}"));}
fn equal(a:CpuContext,b:CpuContext)->bool{a.regs==b.regs&&a.extregs==b.extregs&&a.cpsr==b.cpsr&&a.fpscr==b.fpscr}
fn original(e:&mut Environment,r:usize){let(sp,lr)=(e.cpu.regs()[13],e.cpu.regs()[14]);e.libc_state.peggle_native_batch.hooks[r].original.unwrap().call_without_pushing_stack_frame(e);if matches!(r,1|2|3){e.mem.write(MutPtr::<u32>::from_bits(sp-4),lr);}}
fn disable(e:&mut Environment,r:usize,reason:&str){let h=&mut e.libc_state.peggle_native_batch.hooks[r];h.disabled=true;e.mem.bytes_at_mut(MutPtr::<u8>::from_bits(h.address),8).copy_from_slice(&h.prefix);e.cpu.invalidate_cache_range(h.address,8);event(r,255,reason,0);}
fn predict(e:&Environment,r:usize)->Option<Outcome>{let m=k::snapshot(&e.mem,e.cpu.visible_context(),r)?;let s=&e.libc_state.peggle_native_batch;for input in &m.regions{if s.guard_min!=0&&!k::disjoint(input.address,input.bytes.len()as u32,s.guard_min,s.guard_max-s.guard_min){return None;}}crate::native_batch_specialized::calculate(m,r)}
fn compare(e:&mut Environment,r:usize,p:Outcome)->bool{original(e,r);equal(e.cpu.visible_context(),p.context)&&p.regions.iter().all(|a|e.mem.bytes_at(ConstPtr::<u8>::from_bits(a.address),a.bytes.len()as u32)==&a.bytes[..])}
fn linked(e:&Environment,r:usize)->bool{if !matches!(r,1|2){return true;}let a=if r==1{0x2f7cbc}else{0x2f7bec};e.mem.is_mapped_range(a,8)&&{let b=e.mem.bytes_at(ConstPtr::<u8>::from_bits(a),8);let n=u32::from_le_bytes(b[..4].try_into().unwrap());n>=0xef000003&&n<0xef800000&&b[4..8]==0xe12fff1eu32.to_le_bytes()}}
fn corpus(e:&mut Environment,r:usize)->bool{let saved=e.cpu.visible_context();let Some(a)=e.mem.try_alloc(139264).map(|p|p.cast::<u8>())else{return false;};let base=a.to_bits();e.libc_state.peggle_native_batch.hooks[r].validating=true;let mut passed=0;
 for n in 0..128{let c=crate::native_batch_corpus::seed(&mut e.mem,saved,r,n,base,base+135168);e.cpu.restore_visible_context(c);let Some(p)=predict(e,r)else{break;};if !compare(e,r,p){break;}passed+=1;}
 e.cpu.restore_visible_context(saved);e.mem.free(a.cast());e.libc_state.peggle_native_batch.hooks[r].validating=false;event(r,255,if passed==128{"corpus_passed"}else{"corpus_failed"},passed);passed==128
}
fn run(e:&mut Environment,r:usize){
 let h=&e.libc_state.peggle_native_batch.hooks[r];if r==0&&!h.disabled&&h.corpus&&h.total>=128&&h.shadows[0]>=16&&(h.calls[0]+1)%1024!=0{let s=&e.libc_state.peggle_native_batch;if let Some(p)=k::coordinates(&e.mem,e.cpu.visible_context(),(s.guard_min,s.guard_max)){e.mem.bytes_at_mut(MutPtr::<u8>::from_bits(p.frame_address),20).copy_from_slice(&p.frame);e.mem.write(MutPtr::<u32>::from_bits(p.x),p.vx);e.mem.write(MutPtr::<u32>::from_bits(p.y),p.vy);e.cpu.restore_visible_context(p.context);let h=&mut e.libc_state.peggle_native_batch.hooks[0];h.calls[0]+=1;h.fast[0]+=1;let n=h.fast[0];if n==1||n%65536==0{event(0,0,"fast_calls",n);}return;}}
let s=&e.libc_state.peggle_native_batch.hooks[r];if s.disabled||s.validating||!linked(e,r){return original(e,r);}let Some(p)=predict(e,r)else{return original(e,r);};let class=p.class;
 if !s.corpus{if !corpus(e,r){disable(e,r,"disabled_after_corpus_failure");return original(e,r);}e.libc_state.peggle_native_batch.hooks[r].corpus=true;}
 let s=&mut e.libc_state.peggle_native_batch.hooks[r];s.calls[class]+=1;if s.total<128||s.shadows[class]<16||s.calls[class]%1024==0{if !compare(e,r,p){disable(e,r,"disabled_after_real_mismatch");}else{let s=&mut e.libc_state.peggle_native_batch.hooks[r];s.total+=1;s.shadows[class]+=1;if s.total==128{event(r,255,"real_global_gate_passed",128);}if s.shadows[class]==16{event(r,class,"real_class_gate_passed",16);}}return;}
 for a in &p.regions{if a.writable{e.mem.bytes_at_mut(MutPtr::<u8>::from_bits(a.address),a.bytes.len()as u32).copy_from_slice(&a.bytes);}}e.cpu.restore_visible_context(p.context);let s=&mut e.libc_state.peggle_native_batch.hooks[r];s.fast[class]+=1;if s.fast[class]==1||s.fast[class]%65536==0{event(r,class,"fast_calls",s.fast[class]);}
}
fn routine_0(e:&mut Environment){run(e,0)}
fn routine_1(e:&mut Environment){run(e,1)}
fn routine_2(e:&mut Environment){run(e,2)}
fn routine_3(e:&mut Environment){run(e,3)}
fn routine_4(e:&mut Environment){run(e,4)}
fn routine_5(e:&mut Environment){run(e,5)}
fn routine_6(e:&mut Environment){run(e,6)}
fn routine_7(e:&mut Environment){run(e,7)}
fn routine_8(e:&mut Environment){run(e,8)}
fn routine_9(e:&mut Environment){run(e,9)}
fn routine_10(e:&mut Environment){run(e,10)}
fn routine_11(e:&mut Environment){run(e,11)}
fn routine_12(e:&mut Environment){run(e,12)}
fn routine_13(e:&mut Environment){run(e,13)}
fn routine_14(e:&mut Environment){run(e,14)}
const FUNCTIONS:FunctionExports=&[export_c_func!(routine_0()),export_c_func!(routine_1()),export_c_func!(routine_2()),export_c_func!(routine_3()),export_c_func!(routine_4()),export_c_func!(routine_5()),export_c_func!(routine_6()),export_c_func!(routine_7()),export_c_func!(routine_8()),export_c_func!(routine_9()),export_c_func!(routine_10()),export_c_func!(routine_11()),export_c_func!(routine_12()),export_c_func!(routine_13()),export_c_func!(routine_14())];
pub(crate)const META:&[(&str,u32,u32,&str,bool)]=&[("__ZN4Sexy5Image11GetTexCoordEiiPfS1_",2330352,108,"928fd6dc4d421ccd0e7004ae62695d905f4a9945",false),("__ZN4Sexy16PremultiplyColorEj",2330164,160,"358ae6f42a1248bfcd7654d8e040785bf4dcf5d0",true),("__ZN4Sexy16PremultiplyColorERKNS_5ColorE",2311296,104,"ba5a676e407f3f0a2cb538c3b1aae0ee9eea3ac7",true),("__ZN4Sexy9ImageFont7PrepareEv",2387708,56,"f1a6933d0e5830168cb70f1be031bf231ea0e41a",true),("__Z19CompareKerningPairsRKN4Sexy8KernPairES2_",2331576,32,"c2534a97e60ade69ad0f6660b02693e772869fb3",false),("__Z15CompareCharDataRKN4Sexy8CharDataES2_",2331608,32,"4ebfd73999b7a558a61a436b9a3b1b20ddd9a670",false),("__Z14CompareCharMapRKN4Sexy11CharMappingES2_",2331640,32,"4ebfd73999b7a558a61a436b9a3b1b20ddd9a670",false),("_png_do_read_filler",2719192,1628,"e3d6d9a66a66a6c4d58d42eabbf454cf46dffd44",false),("_png_do_strip_filler",2688056,920,"38ed5c7951e5e8d4a8060e15e55af6f0b4628104",false),("_png_do_bgr",2688976,380,"a2a0c831dfd2618098eba90cb59e983d6137d2ec",false),("_png_do_read_invert_alpha",2720820,688,"f5132964132c4fd4cd74fc787298aa3927550b5b",false),("_png_do_read_swap_alpha",2721508,700,"b5f7d43aa3e9561abe8c7f65cd6e4197e044c306",false),("_png_do_swap",2687828,92,"a2b7d04c7a7e71ff0bb6f275117ea9a6e153e384",false),("_png_do_invert",2687748,80,"4c71abaf701a29f7bb3f5a3b63ecbd5bfcfe25cd",false),("__ZNK4Sexy13CollisionPairltERKS0_",410584,144,"ed5e2d289ca4461d61ff6d2699b738a410d2e3f4",false)];
pub(crate)fn install(verified:bool,bins:&[crate::mach_o::MachO],mem:&mut crate::mem::Mem,dyld:&mut crate::dyld::Dyld,libc:&mut crate::libc::State){use digest::Digest;if !verified{return;}for(r,&(name,address,size,hash,_))in META.iter().enumerate(){if !ACTIVE.contains(&r){event(r,255,"prototype_retained_original_host_overhead",0);continue;}let flag=if r<=6{"PEGGLE_HD_UI_HELPERS_NATIVE"}else if r<=13{"PEGGLE_HD_ROW_TRANSFORMS_NATIVE"}else{"PEGGLE_HD_PAIR_COMPARE_NATIVE"};if std::env::var(flag).as_deref()!=Ok("1"){continue;}
 if bins.first().and_then(|b|b.exported_symbols.get(name))!=Some(&(address|1))||!mem.is_mapped_range(address,size)||format!("{:x}",sha1::Sha1::digest(mem.bytes_at(ConstPtr::<u8>::from_bits(address),size)))!=hash{event(r,255,"image_guard_rejected",0);continue;}install_entry(r,name,address,mem,dyld,libc);}}
fn install_entry(r:usize,name:&'static str,address:u32,mem:&mut crate::mem::Mem,dyld:&mut crate::dyld::Dyld,libc:&mut crate::libc::State){let prefix:[u8;8]=mem.bytes_at(ConstPtr::<u8>::from_bits(address),8).try_into().unwrap();let tr=mem.alloc(16).cast::<u32>();mem.bytes_at_mut(tr.cast::<u8>(),8).copy_from_slice(&prefix);mem.write(tr+2,0xf000f8dfu32);mem.write(tr+3,address+8|1);let host=dyld.create_guest_function(mem,name,FUNCTIONS[r].1);protect_code(libc,tr.to_bits(),16);protect_code(libc,host.addr_without_thumb_bit(),8);libc.peggle_native_batch.hooks[r]=Hook{original:Some(GuestFunction::from_addr_with_thumb_bit(tr.to_bits()|1)),address,host:host.addr_without_thumb_bit(),prefix,..Hook::default()};mem.write(MutPtr::<u32>::from_bits(address),0xf000f8dfu32);mem.write(MutPtr::<u32>::from_bits(address+4),host.addr_with_thumb_bit());event(r,255,"guarded_hook_installed",0);}

pub(crate)fn protect_code(libc:&mut crate::libc::State,address:u32,len:u32){let s=&mut libc.peggle_native_batch;s.guard_min=if s.guard_min==0{address}else{s.guard_min.min(address)};s.guard_max=s.guard_max.max(address.checked_add(len).unwrap());}

pub(crate)const ACTIVE:&[usize]=&[0,1,2,7,8,9,10,11,12];

impl State{pub(crate)fn code_guard(&self)->(u32,u32){(self.guard_min,self.guard_max)}}
