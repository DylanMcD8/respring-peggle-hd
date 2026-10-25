//! Separate the presentation theme from the discontinued Nights entitlement.
//! The original program initializes a presentation-only byte to false, then
//! conditionally sets it true after its store/date check. Suppress only that
//! second store for classic mode. No entitlement checks, profiles, gameplay,
//! resources on disk, registers or flags are changed. Applied before guest code
//! executes, guarded by full executable identity plus the complete routine hash.
use crate::mem::{ConstPtr, MutPtr};
use digest::Digest;

pub(crate) fn install(verified: bool, bins: &[crate::mach_o::MachO], mem: &mut crate::mem::Mem) {
    if !verified { return; }
    let show = std::env::var("PEGGLE_SHOW_NIGHTS_THEME").as_deref() != Ok("0");
    if show {
        crate::diagnostics::event(r#"{"event":"nights_theme","show":true,"mode":"original","entitlement_unchanged":true}"#);
        return;
    }
    const SYMBOL: &str = "__ZN4Sexy14ThunderballAppC2Ev";
    const START: u32 = 0x1135d0;
    const LENGTH: u32 = 6988;
    const STORE: u32 = 0x1139fa;
    const SHA1: &str = "68a4951d655c30cd99fb97f052992721b91f2d0a";
    const EXPECTED: [u8; 4] = [0x81, 0xf8, 0x2c, 0x2];
    let valid = bins.first().and_then(|b| b.exported_symbols.get(SYMBOL)) == Some(&(START | 1))
        && mem.is_mapped_range(START, LENGTH)
        && format!("{:x}", sha1::Sha1::digest(mem.bytes_at(ConstPtr::<u8>::from_bits(START), LENGTH))) == SHA1
        && mem.bytes_at(ConstPtr::<u8>::from_bits(STORE), 4) == EXPECTED;
    if !valid {
        crate::diagnostics::event(r#"{"event":"nights_theme","show":true,"mode":"guard_rejected_original","entitlement_unchanged":true}"#);
        return;
    }
    // Two Thumb NOPs replace one four-byte STRB.W; neither changes flags.
    mem.bytes_at_mut(MutPtr::<u8>::from_bits(STORE), 4).copy_from_slice(&[0x00, 0xbf, 0x00, 0xbf]);
    crate::diagnostics::event(r#"{"event":"nights_theme","show":false,"mode":"classic","entitlement_unchanged":true}"#);
}
