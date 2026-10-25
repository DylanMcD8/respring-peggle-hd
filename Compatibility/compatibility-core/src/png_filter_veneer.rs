/* This Source Code Form is subject to the Mozilla Public License, v. 2.0. */
//! Full-state PNG dispatcher: two unconditional original push slots hold only
//! transient control values, restored before native return or original tail.
pub(crate)fn words(svc:u32,flags_svc:u32,original_thumb:u32)->[u32;15]{
 assert_eq!(svc&0xff000000,0xef000000);assert_eq!(svc&0x00800000,0);
 assert_eq!(flags_svc&0xff800000,0xef000000);
 [0xe50dc004,svc,0xe35c0002,0x0a000005,0xe51dc008,flags_svc,
  0xe51dc004,0xe50de004,0xe50d7008,0xe12fff1e,
  0xe51dc004,0xe50de004,0xe50d7008,0xe51ff004,original_thumb]
}
