/* This Source Code Form is subject to the Mozilla Public License, v. 2.0. */
//! Explicit UTF-32LE byte initializer support. No BOM sniffing or normalization.
//! Apple's explicit-endian NSString initializer preserves U+FEFF and embedded
//! NULs. It rejects an incomplete first scalar but ignores 1–3 trailing bytes
//! after at least one complete scalar (verified against native Foundation).
pub(super) fn decode_little_endian(bytes: &[u8]) -> Option<Vec<u16>> {
    if !bytes.is_empty() && bytes.len() < 4 {
        return None;
    }
    let mut units = Vec::with_capacity(bytes.len() / 4);
    for chunk in bytes.chunks_exact(4) {
        let scalar = char::from_u32(u32::from_le_bytes(chunk.try_into().unwrap()))?;
        let mut pair = [0; 2];
        units.extend_from_slice(scalar.encode_utf16(&mut pair));
    }
    Some(units)
}
