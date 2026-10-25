/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
//! Host text contracts for the legacy little-endian NSString bridge.

/// Native-endian UTF-16 bytes on the supported little-endian ARM guest.
/// NSString's cStringUsingEncoding:10 does not prepend a byte-order mark.
/// The caller adds the two-byte C terminator using its existing allocator.
pub fn utf16_native_bytes(text: &str) -> Vec<u8> {
    text.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

/// Backspace one Unicode scalar without leaving half a UTF-16 surrogate pair.
/// This is not a complete grapheme-cluster editor (combining marks are scalars).
pub fn trailing_scalar_range(text: &str) -> Option<(u32, u32)> {
    let last = text.chars().next_back()?;
    let length = last.len_utf16() as u32;
    let total: u32 = text.encode_utf16().count().try_into().ok()?;
    Some((total - length, length))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn foundation_ascii_fixture_no_bom() {
        assert_eq!(utf16_native_bytes("AB"), [0x41, 0, 0x42, 0]);
    }
    #[test] fn foundation_bmp_fixture() {
        assert_eq!(utf16_native_bytes("é"), [0xe9, 0]);
    }
    #[test] fn foundation_surrogate_fixture() {
        assert_eq!(utf16_native_bytes("😀"), [0x3d, 0xd8, 0, 0xde]);
    }
    #[test] fn empty_encoding_and_deletion() {
        assert!(utf16_native_bytes("").is_empty());
        assert_eq!(trailing_scalar_range(""), None);
    }
    #[test] fn ascii_and_bmp_deletion_ranges() {
        assert_eq!(trailing_scalar_range("Name"), Some((3, 1)));
        assert_eq!(trailing_scalar_range("Aé"), Some((1, 1)));
    }
    #[test] fn surrogate_deletion_preserves_utf16_boundaries() {
        assert_eq!(trailing_scalar_range("A😀"), Some((1, 2)));
        assert_eq!(trailing_scalar_range("😀B"), Some((2, 1)));
    }
}
