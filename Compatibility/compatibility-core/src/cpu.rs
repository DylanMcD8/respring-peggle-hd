/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! CPU facade using a software ARM interpreter. No generated executable memory.
use crate::abi::GuestFunction;
use crate::mem::{GuestUSize, Mem};
mod interpreter;
pub use interpreter::CpuContext;

pub struct Cpu { inner: Box<interpreter::InterpreterCpu> }
#[derive(Debug)]
pub enum CpuState { Normal, Svc(u32), Error(CpuError) }
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CpuError { MemoryError, UndefinedInstruction, Breakpoint }

impl Cpu {
    pub const SP: usize = 13;
    pub const LR: usize = 14;
    pub const PC: usize = 15;
    pub const CPSR_THUMB: u32 = 0x20;
    pub const CPSR_USER_MODE: u32 = 0x10;
    pub fn new(mem: Option<&mut Mem>) -> Self {
        let pages = mem.map_or(0, |m| m.null_segment_size() / 0x1000);
        Self { inner: interpreter::InterpreterCpu::new(pages) }
    }
    pub fn regs(&self) -> &[u32; 16] { self.inner.regs() }
    pub fn regs_mut(&mut self) -> &mut [u32; 16] { self.inner.regs_mut() }
    pub fn cpsr(&self) -> u32 { self.inner.cpsr() }
    pub fn set_cpsr(&mut self, v: u32) { self.inner.set_cpsr(v); }
    pub(crate) fn fpscr(&self)->u32{self.inner.fpscr()}
    pub(crate) fn set_fpscr(&mut self,v:u32){self.inner.set_fpscr(v)}
    pub(crate) fn extregs_mut(&mut self)->&mut [u32;64]{self.inner.extregs_mut()}
    pub(crate) fn visible_context(&self)->CpuContext {CpuContext{regs:*self.inner.regs(),extregs:*self.inner.extregs(),cpsr:self.inner.cpsr(),fpscr:self.inner.fpscr()}}
    pub(crate) fn restore_visible_context(&mut self,c:CpuContext) {*self.inner.regs_mut()=c.regs;*self.inner.extregs_mut()=c.extregs;self.inner.set_cpsr(c.cpsr);self.inner.set_fpscr(c.fpscr);}
    pub(crate)fn enable_collision_sort_observer(&mut self){self.inner.enable_collision_sort_observer();}
    pub fn swap_context(&mut self, ctx: &mut CpuContext) { self.inner.swap_context(ctx); }
    pub fn pc_with_thumb_bit(&self) -> GuestFunction {
        GuestFunction::from_addr_and_thumb_flag(self.regs()[Self::PC], self.cpsr() & Self::CPSR_THUMB != 0)
    }
    pub fn branch(&mut self, pc: GuestFunction) {
        self.regs_mut()[Self::PC] = pc.addr_without_thumb_bit();
        self.set_cpsr((self.cpsr() & !Self::CPSR_THUMB) | if pc.is_thumb() { Self::CPSR_THUMB } else { 0 });
    }
    pub fn branch_with_link(&mut self, pc: GuestFunction, lr: GuestFunction) -> (GuestFunction, GuestFunction) {
        let previous = (self.pc_with_thumb_bit(), GuestFunction::from_addr_with_thumb_bit(self.regs()[Self::LR]));
        self.branch(pc); self.regs_mut()[Self::LR] = lr.addr_with_thumb_bit(); previous
    }
    pub fn invalidate_cache_range(&mut self, base: u32, size: GuestUSize) { self.inner.invalidate_cache_range(base, size); }
    pub fn run_or_step(&mut self, mem: &mut Mem, ticks: Option<&mut u64>) -> CpuState {
        let _timing = crate::diagnostics::scope(crate::diagnostics::Phase::Interpreter);
        self.inner.run_or_step(mem, ticks)
    }
    pub(crate)fn configure_pipeline(&mut self,verified:bool,mem:&Mem,bins:&[crate::mach_o::MachO],flags:[bool;3]){self.inner.configure_pipeline(verified,mem,bins,flags)}
    pub fn dump_regs(&self) { Self::echo_regs(self.regs()); }
    pub fn echo_regs(regs: &[u32; 16]) {
        for row in 0..4 {
            let start = row*4;
            echo!("R{}={:#010x} R{}={:#010x} R{}={:#010x} R{}={:#010x}", start, regs[start], start+1, regs[start+1], start+2, regs[start+2], start+3, regs[start+3]);
        }
    }
}

#[cfg(test)]impl Cpu{pub(crate)fn arm_pipeline_fixture(&mut self,flags:[bool;3]){self.inner.arm_pipeline_fixture(flags)}pub(crate)fn pipeline_fixture_stats(&self)->([u64;3],[u64;3],usize){self.inner.pipeline_fixture_stats()}}

#[cfg(test)]impl Cpu{pub(crate)fn pipeline_fixture_fault(&mut self,index:usize){self.inner.pipeline_fixture_fault(index)}pub(crate)fn pipeline_fixture_exclusive(&mut self,on:bool){self.inner.pipeline_fixture_exclusive(on)}}

#[cfg(test)]impl Cpu{pub(crate)fn font_bucket_fixture(&mut self,fault:bool){self.inner.font_bucket_fixture(fault)}pub(crate)fn font_bucket_stats(&self)->(u64,[u64;2],bool){self.inner.font_bucket_stats()}}
