/* This Source Code Form is subject to the Mozilla Public License, v. 2.0. */
//! HD-only guarded native matrix experiment. Original files remain untouched.
use crate::{Environment,cpu::CpuContext,abi::{GuestFunction,CallFromHost},mem::{ConstVoidPtr,MutVoidPtr,ConstPtr,MutPtr}};
use crate::matrix3_kernel::{self,Product};
use crate::dyld::{export_c_func,FunctionExports};
#[derive(Default)]
pub(crate) struct State { original:Option<GuestFunction>, prefix:[u8;8], address:u32, disabled:bool, validating:bool, corpus_passed:bool, shadows:u64, fast:u64, fallback:u64, calls:u64, shadow_only:bool }
fn report(kind:&str,n:u64){crate::diagnostics::event(&format!("{{\"event\":\"matrix_native\",\"kind\":\"{kind}\",\"count\":{n}}}"));}
fn read(env:&Environment,p:ConstVoidPtr)->Option<[f32;9]>{let bytes=env.mem.get_bytes_fallible(p,36)?;let mut a=[0.;9];for(i,v)in a.iter_mut().enumerate(){*v=f32::from_bits(u32::from_le_bytes(bytes[4*i..4*i+4].try_into().ok()?));}Some(a)}
fn bytes(p:&Product)->[u8;36]{let mut b=[0;36];for(i,v)in p.values.iter().enumerate(){b[4*i..4*i+4].copy_from_slice(&v.to_bits().to_le_bytes());}b}
fn predicted(mut c:CpuContext,out:u32,p:&Product)->CpuContext {c.regs[0]=3;c.regs[1]=out+24;c.regs[2]=out;c.regs[3]=12;c.cpsr=(c.cpsr&0x0fff_ffff)|0x6000_0000;c.extregs[0]=p.values[8].to_bits();c.extregs[4]=p.last_prefix.to_bits();c}
fn equal(a:CpuContext,b:CpuContext)->bool{a.regs==b.regs && a.extregs==b.extregs && a.cpsr==b.cpsr && a.fpscr==b.fpscr}
fn original(env:&mut Environment,out:MutVoidPtr,a:ConstVoidPtr,b:ConstVoidPtr){let f=env.libc_state.peggle_matrix.original.unwrap();let _:()=f.call_from_host(env,(out,a,b));}
fn disable(env:&mut Environment,why:&str){let s=&mut env.libc_state.peggle_matrix;s.disabled=true;let addr=s.address;let prefix=s.prefix;env.mem.bytes_at_mut(MutPtr::<u8>::from_bits(addr),8).copy_from_slice(&prefix);env.cpu.invalidate_cache_range(addr,8);report(why,0);}
/// Compare original instructions on bounded synthetic data using a dedicated
/// guest stack. Original game stack and input matrices are untouched.
fn corpus_gate(env:&mut Environment)->bool {
    env.libc_state.peggle_matrix.validating=true;
    let saved=env.cpu.visible_context();let base=env.mem.alloc(648).cast::<u8>();let addr=base.to_bits();let mut passed=true;let mut completed=0;
    for i in 0..32 {
        env.mem.bytes_at_mut(base,648).fill(0xa5);
        let(a,b)=matrix3_kernel::corpus(i);let Some(p)=matrix3_kernel::multiply(&a,&b,saved.fpscr)else{passed=false;break};
        let ap=ConstVoidPtr::from_bits(addr+4);let bp=ConstVoidPtr::from_bits(addr+48);let out=MutVoidPtr::from_bits(addr+92);
        for (offset,m) in [(4,a),(48,b)]{for(j,v)in m.iter().enumerate(){env.mem.write((base+offset+4*j as u32).cast::<u32>(),v.to_bits());}}
        let mut expected=env.mem.bytes_at(base.cast_const(),132).to_vec();expected[92..128].copy_from_slice(&bytes(&p));
        let mut before=saved;before.regs[13]=addr+640;before.regs[0]=out.to_bits();before.regs[1]=ap.to_bits();before.regs[2]=bp.to_bits();env.cpu.restore_visible_context(before);
        original(env,out,ap,bp);
        if !equal(env.cpu.visible_context(),predicted(before,out.to_bits(),&p)) || env.mem.bytes_at(base.cast_const(),132)!=expected {passed=false;report("corpus_state_or_memory_mismatch_at",i as u64);break}
        completed+=1;
    }
    env.cpu.restore_visible_context(saved);env.mem.free(base.cast());env.libc_state.peggle_matrix.validating=false;
    report(if passed{"corpus_passed"}else{"corpus_failed"},completed);passed
}
fn matrix_multiply(env:&mut Environment,out:MutVoidPtr,a:ConstVoidPtr,b:ConstVoidPtr){
    if env.libc_state.peggle_matrix.disabled || env.libc_state.peggle_matrix.validating {return original(env,out,a,b)}
    let before=env.cpu.visible_context();
    let product=if matrix3_kernel::eligible_addresses(out.to_bits(),a.to_bits(),b.to_bits()) && env.mem.get_bytes_fallible(out.cast_const(),36).is_some(){read(env,a).zip(read(env,b)).and_then(|(a,b)|matrix3_kernel::multiply(&a,&b,before.fpscr))}else{None};
    let Some(p)=product else{env.libc_state.peggle_matrix.fallback+=1;return original(env,out,a,b)};
    if !env.libc_state.peggle_matrix.corpus_passed {
        if !corpus_gate(env){disable(env,"disabled_after_corpus_mismatch");return original(env,out,a,b)}
        env.libc_state.peggle_matrix.corpus_passed=true;
    }
    env.libc_state.peggle_matrix.calls+=1;
    let state=&env.libc_state.peggle_matrix;
    let shadow=state.shadows<64 || state.calls%1024==0 || state.shadow_only;
    if shadow {
        let a_before=env.mem.bytes_at(a.cast::<u8>(),36).to_vec();let b_before=env.mem.bytes_at(b.cast::<u8>(),36).to_vec();
        original(env,out,a,b);
        if !equal(env.cpu.visible_context(),predicted(before,out.to_bits(),&p)) || env.mem.bytes_at(out.cast::<u8>().cast_const(),36)!=bytes(&p) || env.mem.bytes_at(a.cast::<u8>(),36)!=a_before || env.mem.bytes_at(b.cast::<u8>(),36)!=b_before {disable(env,"disabled_after_real_input_mismatch");return}
        let s=&mut env.libc_state.peggle_matrix;s.shadows+=1;
        if s.shadows<=16 {let n=s.shadows;crate::diagnostics::event(&format!("{{\"event\":\"matrix_real_input\",\"index\":{n},\"left_bits\":{:?},\"right_bits\":{:?}}}",read(env,a).unwrap().map(f32::to_bits),read(env,b).unwrap().map(f32::to_bits)));}
        if env.libc_state.peggle_matrix.shadows==64 {report("real_input_gate_passed",64)}
        // Every successful shadow leaves the original routine's result in place.
        return
    }
    env.mem.bytes_at_mut(out.cast::<u8>(),36).copy_from_slice(&bytes(&p));env.cpu.restore_visible_context(predicted(before,out.to_bits(),&p));
    let s=&mut env.libc_state.peggle_matrix;s.fast+=1;if s.fast==1 || s.fast%65536==0{report("fast_calls",s.fast)}
}
const FUNCTIONS:FunctionExports=&[export_c_func!(matrix_multiply(_,_,_))];
pub(crate) fn install(verified_hd:bool,bundle:&crate::bundle::Bundle,bins:&[crate::mach_o::MachO],mem:&mut crate::mem::Mem,dyld:&mut crate::dyld::Dyld,libc:&mut crate::libc::State){
    use digest::Digest;
    if !verified_hd || bundle.bundle_version()!="4840.14.07.16.01" || std::env::var("PEGGLE_HD_MATRIX_FAST").as_deref()==Ok("0"){return}
    let address=0x2765c4;let symbol="__ZNK4Sexy11SexyMatrix3mlERKS0_";
    let valid=bins[0].exported_symbols.get(symbol)==Some(&(address|1)) && bins[0].exported_symbols.get("__ZN4Sexy11SexyMatrix3C1Ev")==Some(&0x276551) && bins[0].exported_symbols.get("__ZN4Sexy11SexyMatrix3C2Ev")==Some(&0x27656d)
        && format!("{:x}",sha1::Sha1::digest(mem.bytes_at(ConstPtr::<u8>::from_bits(address),0xd0)))==MATRIX_SHA1
        && format!("{:x}",sha1::Sha1::digest(mem.bytes_at(ConstPtr::<u8>::from_bits(0x276550),0x28)))==CONSTRUCTORS_SHA1;
    if !valid{report("image_guard_rejected",0);return}
    let prefix=mem.bytes_at(ConstPtr::<u8>::from_bits(address),16).to_vec();
    // Audited complete Thumb instructions, no branches or PC-relative operands.
    let trampoline=mem.alloc(24).cast::<u32>();mem.bytes_at_mut(trampoline.cast::<u8>(),16).copy_from_slice(&prefix);mem.write(trampoline+4,0xf000_f8dfu32);mem.write(trampoline+5,(address+16)|1);
    let target=dyld.create_guest_function(mem,"HD_matrix_multiply",FUNCTIONS[0].1);
    libc.peggle_matrix=State{original:Some(GuestFunction::from_addr_with_thumb_bit(trampoline.to_bits()|1)),prefix:prefix[..8].try_into().unwrap(),address,shadow_only:std::env::var("PEGGLE_HD_MATRIX_SHADOW_ONLY").as_deref()==Ok("1"),..State::default()};
    let ptr=MutPtr::<u32>::from_bits(address);mem.write(ptr,0xf000_f8dfu32);mem.write(ptr+1,target.addr_with_thumb_bit());report("guarded_hook_installed",0);
}
// Generated from the preserved original ARMv7 image; the full fat image SHA1
// and HD-specific anchor symbols are checked independently by environment.rs.
const MATRIX_SHA1:&str="049947f31fd8ba9d0426a9f301abbcd658047262";
const CONSTRUCTORS_SHA1:&str="97a78bea89b63eca5b0c10cb462eccdd557b8db4";
