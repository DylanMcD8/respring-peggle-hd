//! Wire contract for kAudioOutputUnitProperty_IsRunning (2001).
//! Global scope, element zero, read-only UInt32; uses the existing started flag.

pub(super) fn response(started: bool, scope: u32, element: u32, capacity: u32) -> Result<[u8; 4], ()> {
    if scope != 0 || element != 0 || capacity < 4 {
        return Err(());
    }
    Ok(u32::from(started).to_le_bytes())
}

#[cfg(test)]
mod tests {
    use super::response;

    #[test]
    fn output_state_has_guest_uint32_wire_format() {
        assert_eq!(response(false, 0, 0, 4), Ok([0, 0, 0, 0]));
        assert_eq!(response(true, 0, 0, 4), Ok([1, 0, 0, 0]));
        assert_eq!(response(false, 0, 0, 8), Ok([0, 0, 0, 0]));
    }

    #[test]
    fn invalid_scope_element_and_short_buffer_are_errors() {
        for scope in [1, 2, u32::MAX] {
            assert_eq!(response(true, scope, 0, 4), Err(()));
        }
        for element in [1, u32::MAX] {
            assert_eq!(response(true, 0, element, 4), Err(()));
        }
        for capacity in 0..4 {
            assert_eq!(response(true, 0, 0, capacity), Err(()));
        }
    }

    #[test]
    fn oversized_output_preserves_bytes_after_uint32() {
        let mut output = [0xa5; 12];
        let value = response(true, 0, 0, output.len() as u32).unwrap();
        output[..4].copy_from_slice(&value);
        assert_eq!(&output[..4], &[1, 0, 0, 0]);
        assert_eq!(&output[4..], &[0xa5; 8]);
    }
}
