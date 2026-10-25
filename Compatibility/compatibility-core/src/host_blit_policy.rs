//! Conservative eligibility contract for one opaque fullscreen host blit.
pub fn eligible(fullscreen: bool, ancestry_safe: bool, opaque: bool,
                alternate_contents: bool, width: u32, height: u32, bytes: usize) -> bool {
    fullscreen && ancestry_safe && opaque && !alternate_contents
        && width > 0 && height > 0
        && (width as usize).checked_mul(height as usize).and_then(|n| n.checked_mul(4)) == Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::eligible;
    #[test]
    fn only_complete_opaque_fullscreen_frames_use_direct_path() {
        assert!(eligible(true, true, true, false, 768, 1024, 768 * 1024 * 4));
        assert!(!eligible(false, true, true, false, 768, 1024, 768 * 1024 * 4));
        assert!(!eligible(true, false, true, false, 768, 1024, 768 * 1024 * 4));
        assert!(!eligible(true, true, false, false, 768, 1024, 768 * 1024 * 4));
        assert!(!eligible(true, true, true, true, 768, 1024, 768 * 1024 * 4));
    }
    #[test]
    fn truncated_empty_and_overflowing_buffers_fall_back() {
        assert!(!eligible(true, true, true, false, 0, 0, 0));
        assert!(!eligible(true, true, true, false, 2, 2, 15));
        assert!(!eligible(true, true, true, false, 2, 2, 17));
        assert!(!eligible(true, true, true, false, u32::MAX, u32::MAX, usize::MAX));
    }
    #[test]
    fn odd_width_rgba_rows_are_valid_without_pitch_assumptions() {
        assert!(eligible(true, true, true, false, 3, 5, 60));
    }
}
