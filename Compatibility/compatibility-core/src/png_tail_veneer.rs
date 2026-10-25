/* This Source Code Form is subject to the Mozilla Public License, v. 2.0. */
//! Pure emitted ARM veneer, tested with ARM/Thumb synthetic targets.
pub(crate) fn words(svc:u32,error_thumb:u32,original_thumb:u32)->[u32;8]{
 assert_eq!(svc&0xff000000,0xef000000);assert_eq!(svc&0x00800000,0);
 [svc,0xe35c0001,0x059ff008,0xe35c0002,0x059ff004,0xe12fff1e,error_thumb,original_thumb]
}
