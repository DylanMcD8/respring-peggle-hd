/* This Source Code Form is subject to the Mozilla Public License, v. 2.0. */
//! Opt-in, image-guarded native EndPoint vector indexing, with original gates.
use crate::{Environment,cpu::CpuContext,abi::GuestFunction,mem::{ConstPtr,MutPtr}};
use crate::dyld::{export_c_func,FunctionExports};
use crate::endpoint_index_kernel::{self as kernel,Outcome};
#[derive(Default)]pub(crate) struct State {
    original:Option<GuestFunction>,prefix:[u8;8],address:u32,disabled:bool,
    validating:bool,corpus_passed:bool,shadows:u64,fast:u64,calls:u64,fallback:u64,shadow_only:bool,
}
fn report(kind:&str,n:u64){crate::diagnostics::event(&format!("{{\"event\":\"endpoint_index_native\",\"kind\":\"{kind}\",\"count\":{n}}}"));}
fn original(env:&mut Environment){
    // Preserve the original entry SP and its exact two dead-stack stores.
    // Typed CallFromHost would insert an additional guest frame here.
    let f=env.libc_state.peggle_endpoint_index.original.unwrap();
    f.call_without_pushing_stack_frame(env);
}
fn predicted(mut c:CpuContext,p:Outcome)->CpuContext {
    c.regs[0]=p.address;c.regs[1]=p.scaled;c.regs[2]=12;c.cpsr=p.cpsr;c
}
fn equal(a:CpuContext,b:CpuContext)->bool{a.regs==b.regs && a.extregs==b.extregs && a.cpsr==b.cpsr && a.fpscr==b.fpscr}
fn disable(env:&mut Environment,why:&str){
    let s=&mut env.libc_state.peggle_endpoint_index;s.disabled=true;
    let (addr,prefix)=(s.address,s.prefix);
    env.mem.bytes_at_mut(MutPtr::<u8>::from_bits(addr),8).copy_from_slice(&prefix);
    env.cpu.invalidate_cache_range(addr,8);report(why,0);
}
fn corpus_gate(env:&mut Environment)->bool{
    let saved=env.cpu.visible_context();
    let Some(base)=env.mem.try_alloc(160)else{report("corpus_allocation_unavailable",0);return false};
    let base=base.cast::<u8>();let addr=base.to_bits();
    env.libc_state.peggle_endpoint_index.validating=true;
    let mut passed=true;let mut completed=0;
    for i in 0..64u32 {
        env.mem.bytes_at_mut(base,160).fill(0xa5);
        let vector=addr+16;let sp=addr+144;
        let index=match i%8{0=>0,1=>1,2=>0xffffffff,3=>0x20000000,4=>0x40000000,5=>0x55555556,_=>i.wrapping_mul(0x9e3779b9)};
        let pointer=i.wrapping_mul(0x10203041);
        env.mem.write(MutPtr::<u32>::from_bits(vector),pointer);
        let mut before=saved;before.regs[0]=vector;before.regs[1]=index;before.regs[13]=sp;
        before.cpsr=(saved.cpsr&0x0fffffff)|((i&15)<<28);
        let product=kernel::calculate(pointer,index,before.cpsr);
        let mut expected=env.mem.bytes_at(base.cast_const(),160).to_vec();
        expected[136..144].copy_from_slice(&kernel::stack_words(vector,index));
        env.cpu.restore_visible_context(before);original(env);
        if !equal(env.cpu.visible_context(),predicted(before,product)) || env.mem.bytes_at(base.cast_const(),160)!=expected {
            passed=false;report("corpus_state_or_memory_mismatch_at",i as u64);break
        }
        completed+=1;
    }
    env.cpu.restore_visible_context(saved);env.mem.free(base.cast());
    env.libc_state.peggle_endpoint_index.validating=false;
    report(if passed{"corpus_passed"}else{"corpus_failed"},completed);passed
}
fn endpoint_index(env:&mut Environment){
    let s=&env.libc_state.peggle_endpoint_index;
    if s.disabled || s.validating{return original(env)}
    let vector=env.cpu.regs()[0];let index=env.cpu.regs()[1];let sp=env.cpu.regs()[13];
    let base=if kernel::eligible(vector,sp) && env.mem.get_bytes_fallible(ConstPtr::from_bits(sp-8),8).is_some(){
        env.mem.get_bytes_fallible(ConstPtr::from_bits(vector),4).map(|b|u32::from_le_bytes(b.try_into().unwrap()))
    }else{None};
    let Some(base)=base else{
        let s=&mut env.libc_state.peggle_endpoint_index;s.fallback+=1;
        if s.fallback==1 || s.fallback%65536==0{report("address_or_stack_fallback",s.fallback)}
        return original(env)
    };
    let product=kernel::calculate(base,index,env.cpu.cpsr());
    if !env.libc_state.peggle_endpoint_index.corpus_passed {
        if !corpus_gate(env){disable(env,"disabled_after_corpus_rejection");return original(env)}
        env.libc_state.peggle_endpoint_index.corpus_passed=true;
    }
    let s=&mut env.libc_state.peggle_endpoint_index;s.calls+=1;
    let shadow=s.shadows<64 || s.calls%4096==0 || s.shadow_only;
    if shadow {
        let before=env.cpu.visible_context();original(env);
        if !equal(env.cpu.visible_context(),predicted(before,product))
            || env.mem.bytes_at(ConstPtr::<u8>::from_bits(sp-8),8)!=kernel::stack_words(vector,index)
            || env.mem.read(ConstPtr::<u32>::from_bits(vector))!=base {
            disable(env,"disabled_after_real_input_mismatch");return
        }
        let s=&mut env.libc_state.peggle_endpoint_index;s.shadows+=1;
        if s.shadows==64{report("real_input_gate_passed",64)}
        // Original result remains in place for every shadow.
        return
    }
    env.mem.bytes_at_mut(MutPtr::<u8>::from_bits(sp-8),8).copy_from_slice(&kernel::stack_words(vector,index));
    let regs=env.cpu.regs_mut();regs[0]=product.address;regs[1]=product.scaled;regs[2]=12;
    env.cpu.set_cpsr(product.cpsr);
    let s=&mut env.libc_state.peggle_endpoint_index;s.fast+=1;
    if s.fast==1 || s.fast%1048576==0{report("fast_calls",s.fast)}
}
const FUNCTIONS:FunctionExports=&[export_c_func!(endpoint_index())];
pub(crate) fn install(verified_hd:bool,bins:&[crate::mach_o::MachO],mem:&mut crate::mem::Mem,dyld:&mut crate::dyld::Dyld,libc:&mut crate::libc::State){
    use digest::Digest;
    if !verified_hd || std::env::var("PEGGLE_HD_ENDPOINT_NATIVE").as_deref()!=Ok("1"){return}
    let address=0x612f4;let symbol="__ZNSt6vectorIN4Sexy12CollisionMgr8EndPointESaIS2_EEixEm";
    let prefix=mem.bytes_at(ConstPtr::<u8>::from_bits(address),8);
    let valid=bins[0].exported_symbols.get(symbol)==Some(&(address|1))
        && format!("{:x}",sha1::Sha1::digest(mem.bytes_at(ConstPtr::<u8>::from_bits(address),0x1c)))==FUNCTION_SHA1
        && prefix==[0x82,0xb0,0x01,0x90,0x00,0x91,0x01,0x98];
    if !valid{report("image_guard_rejected",0);return}
    let prefix:[u8;8]=prefix.try_into().unwrap();
    // Four complete 16-bit Thumb instructions, no branch/PC-relative operands.
    let trampoline=mem.alloc(16).cast::<u32>();
    mem.bytes_at_mut(trampoline.cast::<u8>(),8).copy_from_slice(&prefix);
    mem.write(trampoline+2,0xf000_f8dfu32);mem.write(trampoline+3,(address+8)|1);
    let target=dyld.create_guest_function(mem,"HD_endpoint_index",FUNCTIONS[0].1);
    libc.peggle_endpoint_index=State{original:Some(GuestFunction::from_addr_with_thumb_bit(trampoline.to_bits()|1)),prefix,address,shadow_only:std::env::var("PEGGLE_HD_ENDPOINT_SHADOW_ONLY").as_deref()==Ok("1"),..State::default()};
    let ptr=MutPtr::<u32>::from_bits(address);mem.write(ptr,0xf000_f8dfu32);mem.write(ptr+1,target.addr_with_thumb_bit());
    report("guarded_hook_installed",0);
}
const FUNCTION_SHA1:&str="fd2e05191056b0e4dc422fd134951b22696379cb";
