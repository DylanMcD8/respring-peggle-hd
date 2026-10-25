/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. https://mozilla.org/MPL/2.0/ */
//! Instruction-frequency sampling at existing budget boundaries. The hot
//! instruction body has no extra counter, branch, clock, allocation, or I/O.
#[derive(Debug)]
pub struct Schedule { remaining:u64, random:u32 }
impl Default for Schedule {
    fn default()->Self { Self {remaining:65536,random:0x50454747} }
}
impl Schedule {
    pub fn quota(&self,budget:u64)->u64 { budget.max(1).min(self.remaining) }
    pub fn consumed(&mut self,count:u64)->bool {
        debug_assert!(count<=self.remaining);
        self.remaining-=count;
        if self.remaining!=0 {return false}
        self.random^=self.random<<13;self.random^=self.random>>17;self.random^=self.random<<5;
        self.remaining=32768+u64::from(self.random&65535);
        true
    }
}
/// Commit precisely the executed count even when a Rust panic unwinds a batch.
/// Original behavior executes one instruction for an initial zero budget.
pub struct Block<'a> { outer:&'a mut u64, before:u64, quota:u64, pub remaining:u64 }
impl<'a> Block<'a> {
    pub fn new(outer:&'a mut u64,quota:u64)->Self {
        let before=*outer;Self {outer,before,quota,remaining:quota}
    }
    pub fn used(&self)->u64 {self.quota-self.remaining}
}
impl Drop for Block<'_> {
    fn drop(&mut self){*self.outer=self.before.saturating_sub(self.used());}
}
#[cfg(test)] mod tests {
    use super::*;
    #[test]fn quota_respects_remaining_and_zero_budget(){let s=Schedule::default();assert_eq!(s.quota(0),1);assert_eq!(s.quota(7),7);assert_eq!(s.quota(100000),65536);}
    #[test]fn only_sample_after_all_instructions_consumed(){let mut s=Schedule::default();assert!(!s.consumed(65535));assert_eq!(s.quota(100),1);assert!(s.consumed(1));assert!((32768..=98303).contains(&s.quota(u64::MAX)));}
    #[test]fn jitter_varies_within_bounds(){let mut s=Schedule::default();let mut seen=std::collections::BTreeSet::new();for _ in 0..1000{let q=s.quota(u64::MAX);assert!((32768..=98303).contains(&q));seen.insert(q);assert!(s.consumed(q));}assert!(seen.len()>900);}
    #[test]fn partial_batch_commits_exactly_on_return(){let mut b=100;{let mut block=Block::new(&mut b,50);block.remaining=47;assert_eq!(block.used(),3);}assert_eq!(b,97);}
    #[test]fn exhausted_batch_commits_and_zero_saturates(){let mut b=0;{let mut block=Block::new(&mut b,1);block.remaining=0;}assert_eq!(b,0);let mut b=10;{let mut block=Block::new(&mut b,10);block.remaining=0;}assert_eq!(b,0);}
    #[test]fn panic_unwind_preserves_completed_progress(){let mut b=100;let p=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||{let mut block=Block::new(&mut b,50);block.remaining=44;panic!("synthetic failure");}));assert!(p.is_err());assert_eq!(b,94);}
    #[test]fn zero_work_does_not_advance_sampling(){let mut s=Schedule::default();let before=s.quota(u64::MAX);assert!(!s.consumed(0));assert_eq!(s.quota(u64::MAX),before);}
}
