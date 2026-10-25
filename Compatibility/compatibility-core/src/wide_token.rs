/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
//! Offset contract shared by the guest's reentrant 32-bit wcstok bridge.
pub fn next_token_range(text: &[i32], separators: &[i32]) -> Option<(usize, usize)> {
    let delimiter = |c| separators.iter().take_while(|&&x| x != 0).any(|&x| x == c);
    let start = text.iter().position(|&c| c == 0 || !delimiter(c))?;
    if text[start] == 0 { return None; }
    let end = (start..text.len()).find(|&i| text[i] == 0 || delimiter(text[i]))?;
    Some((start, end))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn wide(s: &str) -> Vec<i32> { s.chars().map(|c| c as i32).chain([0]).collect() }
    fn tokens(s: &str, sep: &str) -> Vec<String> {
        let mut s=wide(s); let d=wide(sep); let mut offset=0; let mut result=Vec::new();
        while let Some((a,b))=next_token_range(&s[offset..],&d) {
            result.push(s[offset+a..offset+b].iter().map(|&c| char::from_u32(c as u32).unwrap()).collect());
            let split=s[offset+b]!=0;s[offset+b]=0;if !split { break; }offset+=b+1;
        }result
    }
    #[test] fn fever_words_and_repeated_delimiters() { assert_eq!(tokens("  EXTREME  FEVER! "," "),["EXTREME","FEVER!"]); }
    #[test] fn empty_and_only_separators_have_no_empty_tokens() { assert!(tokens("",",").is_empty());assert!(tokens(",,,",",").is_empty()); }
    #[test] fn empty_separator_keeps_whole_string() { assert_eq!(tokens("A B",""),["A B"]); }
    #[test] fn wide_unicode_is_codepoints_not_utf16_or_bytes() { assert_eq!(tokens("星😀 星"," "),["星😀","星"]); }
    #[test] fn separator_set_can_change_between_calls() { let t=wide("a,b;c");assert_eq!(next_token_range(&t,&wide(",")),Some((0,1)));assert_eq!(next_token_range(&t[2..],&wide(";")),Some((0,1))); }
    #[test] fn independent_streams_have_no_global_state() { assert_eq!(tokens("a:b",":"),["a","b"]);assert_eq!(tokens("c,d",","),["c","d"]); }
}
