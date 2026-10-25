/* This Source Code Form is subject to the Mozilla Public License,v2.0. */
//! Presentation deadlines only. Does not advance simulation or audio clocks.
use std::time::{Duration,Instant};
#[derive(Default)]pub(crate)struct Schedule{due:Option<Instant>,interval:Option<Duration>}
pub(crate)struct Decision{pub render:bool,pub next:Instant,pub changed:bool}
impl Schedule{
 pub fn reset(&mut self){self.due=None;self.interval=None;}
 pub fn poll(&mut self,now:Instant,interval:Duration,force:bool)->Decision{
  assert!(!interval.is_zero());
  let changed=self.interval!=Some(interval)||self.due.is_none();
  if changed{let next=now.checked_add(interval).unwrap();self.interval=Some(interval);self.due=Some(next);return Decision{render:true,next,changed:true};}
  let due=self.due.unwrap();
  if now<due{return Decision{render:force,next:due,changed:false};}
  // Strictly later than now, even at an exact boundary. Missed deadlines
  // are coalesced, never replayed as a catch-up burst after a stall/pause.
  let steps=now.duration_since(due).as_nanos()/interval.as_nanos()+1;
  let next=u32::try_from(steps).ok().and_then(|n|interval.checked_mul(n)).and_then(|d|due.checked_add(d)).unwrap_or_else(||now.checked_add(interval).unwrap());
  self.due=Some(next);Decision{render:true,next,changed:false}
 }
}
