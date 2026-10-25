/* This Source Code Form is subject to the Mozilla Public License, v. 2.0. */
//! Image-guarded, opt-in native EndPoint comparator. No sort/pair side effects.
use crate::{Environment,cpu::CpuContext,abi::GuestFunction,mem::{ConstPtr,MutPtr}};
use crate::dyld::{export_c_func,FunctionExports};
use crate::endpoint_compare_kernel::{self as kernel,Outcome};
#[derive(Default)]pub(crate)struct State{
    original:Option<GuestFunction>,prefix:[u8;8],address:u32,disabled:bool,
    validating:bool,corpus_passed:bool,shadows:u64,fast:u64,calls:u64,fallback:u64,address_fallback:u64,control_fallback:u64,shadow_only:bool,
}
#[derive(Clone,Copy)]struct Inputs{left_endpoint:u32,right_endpoint:u32,sp:u32,left_pointer:u32,right_pointer:u32,left:u32,right:u32}
fn report(kind:&str,n:u64){crate::diagnostics::event(&format!("{{\"event\":\"endpoint_compare_native\",\"kind\":\"{kind}\",\"count\":{n}}}"));}
fn original(env:&mut Environment){let f=env.libc_state.peggle_endpoint_compare.original.unwrap();f.call_without_pushing_stack_frame(env);}
fn word(env:&Environment,address:u32,sp:u32)->Option<u32>{
    if !kernel::readable_span(address,sp){return None}
    env.mem.get_bytes_fallible(ConstPtr::from_bits(address),4).map(|b|u32::from_le_bytes(b.try_into().unwrap()))
}
fn inputs(env:&Environment)->Option<Inputs>{
    let left_endpoint=env.cpu.regs()[0];let right_endpoint=env.cpu.regs()[1];let sp=env.cpu.regs()[13];
    if sp<8 || sp&3!=0 || env.mem.get_bytes_fallible(ConstPtr::from_bits(sp-8),8).is_none(){return None}
    let left_pointer=word(env,left_endpoint.checked_add(4)?,sp)?;
    let right_pointer=word(env,right_endpoint.checked_add(4)?,sp)?;
    Some(Inputs{left_endpoint,right_endpoint,sp,left_pointer,right_pointer,left:word(env,left_pointer,sp)?,right:word(env,right_pointer,sp)?})
}
fn unchanged(env:&Environment,i:Inputs)->bool{
    word(env,i.left_endpoint+4,i.sp)==Some(i.left_pointer) && word(env,i.right_endpoint+4,i.sp)==Some(i.right_pointer)
        && word(env,i.left_pointer,i.sp)==Some(i.left) && word(env,i.right_pointer,i.sp)==Some(i.right)
}
fn predicted(mut c:CpuContext,i:Inputs,p:Outcome)->CpuContext{
    c.regs[0]=p.less;c.extregs[0]=i.left;c.extregs[4]=i.right;c.cpsr=p.cpsr;c.fpscr=p.fpscr;c
}
fn equal(a:CpuContext,b:CpuContext)->bool{a.regs==b.regs && a.extregs==b.extregs && a.cpsr==b.cpsr && a.fpscr==b.fpscr}
fn disable(env:&mut Environment,why:&str){let s=&mut env.libc_state.peggle_endpoint_compare;s.disabled=true;let(a,p)=(s.address,s.prefix);env.mem.bytes_at_mut(MutPtr::<u8>::from_bits(a),8).copy_from_slice(&p);env.cpu.invalidate_cache_range(a,8);report(why,0);}
fn corpus_gate(env:&mut Environment)->bool{
    let saved=env.cpu.visible_context();let Some(base)=env.mem.try_alloc(256)else{report("corpus_allocation_unavailable",0);return false};let base=base.cast::<u8>();let a=base.to_bits();
    env.libc_state.peggle_endpoint_compare.validating=true;let mut passed=true;let mut completed=0;
    for n in 0..kernel::CORPUS_CASES{
        env.mem.bytes_at_mut(base,256).fill(0xa5);let(left,right)=kernel::corpus(n);
        let i=Inputs{left_endpoint:a+16,right_endpoint:a+48,sp:a+240,left_pointer:a+80,right_pointer:a+96,left,right};
        for(p,v)in[(a+20,i.left_pointer),(a+52,i.right_pointer),(i.left_pointer,left),(i.right_pointer,right)]{env.mem.write(MutPtr::<u32>::from_bits(p),v);}
        let mut before=saved;before.regs[0]=i.left_endpoint;before.regs[1]=i.right_endpoint;before.regs[13]=i.sp;
        before.cpsr=(saved.cpsr&0x0fffffff)|((n&15)<<28);
        before.fpscr=((n&15)<<28)|if n&1==0{0x0800009f}else{0};
        let p=kernel::compare(left,right,before.cpsr,before.fpscr).unwrap();
        let mut expected=env.mem.bytes_at(base.cast_const(),256).to_vec();expected[232..240].copy_from_slice(&kernel::stack_words(i.left_endpoint,i.right_endpoint));
        env.cpu.restore_visible_context(before);original(env);
        if !equal(env.cpu.visible_context(),predicted(before,i,p)) || env.mem.bytes_at(base.cast_const(),256)!=expected{passed=false;report("corpus_state_or_memory_mismatch_at",n as u64);break}
        completed+=1;
    }
    env.cpu.restore_visible_context(saved);env.mem.free(base.cast());env.libc_state.peggle_endpoint_compare.validating=false;
    report(if passed{"corpus_passed"}else{"corpus_failed"},completed);passed
}
fn endpoint_compare(env:&mut Environment){
    let s=&env.libc_state.peggle_endpoint_compare;if s.disabled || s.validating{return original(env)}
    let compatible=kernel::supported(env.cpu.cpsr(),env.cpu.fpscr());
    let pair=if compatible{inputs(env).and_then(|i|kernel::compare(i.left,i.right,env.cpu.cpsr(),env.cpu.fpscr()).map(|p|(i,p)))}else{None};
    let Some((i,p))=pair else{
        let s=&mut env.libc_state.peggle_endpoint_compare;s.fallback+=1;
        let (kind,n)=if compatible{s.address_fallback+=1;("address_or_stack_fallback",s.address_fallback)}else{s.control_fallback+=1;("unsupported_fp_control_fallback",s.control_fallback)};
        if n==1 || n%65536==0{report(kind,n)}return original(env)
    };
    if !env.libc_state.peggle_endpoint_compare.corpus_passed{
        if !corpus_gate(env){disable(env,"disabled_after_corpus_rejection");return original(env)}
        env.libc_state.peggle_endpoint_compare.corpus_passed=true;
    }
    let s=&mut env.libc_state.peggle_endpoint_compare;s.calls+=1;let shadow=s.shadows<128 || s.calls%4096==0 || s.shadow_only;
    if shadow{
        let before=env.cpu.visible_context();original(env);
        if !equal(env.cpu.visible_context(),predicted(before,i,p)) || !unchanged(env,i)
            || env.mem.bytes_at(ConstPtr::<u8>::from_bits(i.sp-8),8)!=kernel::stack_words(i.left_endpoint,i.right_endpoint){disable(env,"disabled_after_real_input_mismatch");return}
        let s=&mut env.libc_state.peggle_endpoint_compare;s.shadows+=1;
        if s.shadows<=16{crate::diagnostics::event(&format!("{{\"event\":\"endpoint_compare_real_input\",\"index\":{},\"left_bits\":{},\"right_bits\":{},\"cpsr_before\":{},\"fpscr_before\":{}}}",s.shadows,i.left,i.right,before.cpsr,before.fpscr));}
        if s.shadows==128{report("real_input_gate_passed",128)}return
    }
    env.mem.bytes_at_mut(MutPtr::<u8>::from_bits(i.sp-8),8).copy_from_slice(&kernel::stack_words(i.left_endpoint,i.right_endpoint));
    env.cpu.regs_mut()[0]=p.less;let fp=env.cpu.extregs_mut();fp[0]=i.left;fp[4]=i.right;env.cpu.set_cpsr(p.cpsr);env.cpu.set_fpscr(p.fpscr);
    let s=&mut env.libc_state.peggle_endpoint_compare;s.fast+=1;if s.fast==1 || s.fast%1048576==0{report("fast_calls",s.fast)}
}
const FUNCTIONS:FunctionExports=&[export_c_func!(endpoint_compare())];
pub(crate)fn install(verified_hd:bool,bins:&[crate::mach_o::MachO],mem:&mut crate::mem::Mem,dyld:&mut crate::dyld::Dyld,libc:&mut crate::libc::State){
    use digest::Digest;
    if !verified_hd || std::env::var("PEGGLE_HD_ENDPOINT_COMPARE_NATIVE").as_deref()!=Ok("1"){return}
    let address=0x615ec;let symbol="__ZNK4Sexy12CollisionMgr8EndPointltERKS1_";
    let prefix=mem.bytes_at(ConstPtr::<u8>::from_bits(address),8);
    let valid=bins[0].exported_symbols.get(symbol)==Some(&(address|1))
        && format!("{:x}",sha1::Sha1::digest(mem.bytes_at(ConstPtr::<u8>::from_bits(address),0x30)))==FUNCTION_SHA1
        && prefix==[0x82,0xb0,0x01,0x90,0x00,0x91,0x01,0x98];
    if !valid{report("image_guard_rejected",0);return}
    let prefix:[u8;8]=prefix.try_into().unwrap();let trampoline=mem.alloc(16).cast::<u32>();
    mem.bytes_at_mut(trampoline.cast::<u8>(),8).copy_from_slice(&prefix);mem.write(trampoline+2,0xf000_f8dfu32);mem.write(trampoline+3,(address+8)|1);
    let target=dyld.create_guest_function(mem,"HD_endpoint_compare",FUNCTIONS[0].1);
    libc.peggle_endpoint_compare=State{original:Some(GuestFunction::from_addr_with_thumb_bit(trampoline.to_bits()|1)),prefix,address,shadow_only:std::env::var("PEGGLE_HD_ENDPOINT_COMPARE_SHADOW_ONLY").as_deref()==Ok("1"),..State::default()};
    let ptr=MutPtr::<u32>::from_bits(address);mem.write(ptr,0xf000_f8dfu32);mem.write(ptr+1,target.addr_with_thumb_bit());report("guarded_hook_installed",0);
}
const FUNCTION_SHA1:&str="0a4db24f7a44174620926757416f70919c892430";
