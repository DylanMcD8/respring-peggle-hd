/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. https://mozilla.org/MPL/2.0/ */
//! C-locale wcstol, with the guest's 32-bit wchar_t and signed long.
#[derive(Debug, PartialEq, Eq)]
pub enum Error { InvalidBase, Range }

#[derive(Debug, PartialEq, Eq)]
pub struct Conversion { pub value: i32, pub end: usize, pub error: Option<Error> }

fn digit(c: i32) -> Option<u32> {
    match c {
        48..=57 => Some((c - 48) as u32),
        65..=90 => Some((c - 65 + 10) as u32),
        97..=122 => Some((c - 97 + 10) as u32),
        _ => None,
    }
}

/// Reads only the necessary guest wide units, without UTF-8 conversion,
/// allocation, or interpreting invalid Unicode as a host Rust string.
pub fn parse(mut read: impl FnMut(usize) -> i32, base: i32) -> Conversion {
    if base != 0 && !(2..=36).contains(&base) {
        return Conversion { value: 0, end: 0, error: Some(Error::InvalidBase) };
    }
    let mut pos = 0;
    while matches!(read(pos), 9..=13 | 32) { pos += 1; }
    let negative = read(pos) == 45;
    if matches!(read(pos), 43 | 45) { pos += 1; }
    let mut radix = base as u32;
    // An 0x prefix requires a following hex digit. "0x" itself consumes
    // only the zero, so endptr points at x, as on Darwin libc.
    if (radix == 0 || radix == 16) && read(pos) == 48
        && matches!(read(pos + 1), 88 | 120)
        && digit(read(pos + 2)).is_some_and(|d| d < 16) {
        radix = 16;
        pos += 2;
    } else if radix == 0 {
        radix = if read(pos) == 48 { 8 } else { 10 };
    }
    let first_digit = pos;
    let limit = if negative { 2_147_483_648u32 } else { 2_147_483_647u32 };
    let mut value = 0u32;
    let mut overflow = false;
    while let Some(d) = digit(read(pos)).filter(|&d| d < radix) {
        if value > limit / radix || (value == limit / radix && d > limit % radix) {
            overflow = true;
        } else if !overflow {
            value = value * radix + d;
        }
        pos += 1;
    }
    if pos == first_digit {
        return Conversion { value: 0, end: 0, error: None };
    }
    let value = if overflow {
        if negative { i32::MIN } else { i32::MAX }
    } else if negative { -(value as i64) as i32 } else { value as i32 };
    Conversion { value, end: pos, error: overflow.then_some(Error::Range) }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn p(s: &str, base: i32) -> Conversion {
        let units: Vec<i32> = s.chars().map(|c| c as i32).chain([0]).collect();
        parse(|i| units[i], base)
    }
    #[test] fn fever_labels_and_empty_input() {
        for s in ["", "Extreme", "Fever", " ", " +", "-word", "\u{3000}8"] {
            assert_eq!(p(s, 10), Conversion { value: 0, end: 0, error: None });
        }
    }
    #[test] fn signed_decimal_and_endptr_in_wide_units() {
        assert_eq!(p(" \t-127 fever", 10), Conversion { value: -127, end: 6, error: None });
        assert_eq!(p("+42é", 10), Conversion { value: 42, end: 3, error: None });
    }
    #[test] fn c_prefixes_and_bases() {
        for (s, base, value, end) in [("0779",0,63,3),("0xFa!",0,250,4),
            ("-0X80",16,-128,5),("0x",0,0,1),("0xQ",16,0,1),
            ("0b10",0,0,1),("1012",2,5,3),("zZ!",36,1295,2)] {
            assert_eq!(p(s,base), Conversion { value, end, error: None });
        }
    }
    #[test] fn every_radix_has_correct_digit_boundary() {
        for base in 2..=36 {
            let n = (base - 1) as u32;
            let c = if n < 10 { char::from_u32(48+n) } else { char::from_u32(97+n-10) }.unwrap();
            assert_eq!(p(&format!("{c}!"),base).value,n as i32);
            assert_eq!(p(&format!("{c}!"),base).end,1);
        }
    }
    #[test] fn signed32_boundaries_and_complete_overflow_scan() {
        for (s,value,error) in [("2147483647",i32::MAX,None),("-2147483648",i32::MIN,None),
            ("2147483648",i32::MAX,Some(Error::Range)),("-2147483649",i32::MIN,Some(Error::Range)),
            ("99999999999999999999999999999999999999999",i32::MAX,Some(Error::Range))] {
            assert_eq!(p(&format!("{s}!"),10),Conversion {value,end:s.len(),error});
        }
        assert_eq!(p("0x80000000",0).error,Some(Error::Range));
        assert_eq!(p("-0x80000000",0).value,i32::MIN);
    }
    #[test] fn invalid_bases() {
        for b in [-1,1,37,i32::MAX] {
            assert_eq!(p("10",b),Conversion {value:0,end:0,error:Some(Error::InvalidBase)});
        }
    }
    #[test] fn non_unicode_units_and_nul_are_terminators() {
        let units = [49,50,0xD800,0];
        assert_eq!(parse(|i|units[i],10),Conversion {value:12,end:2,error:None});
        let units = [49,0,50,0];
        assert_eq!(parse(|i|units[i],10).end,1);
    }
}
