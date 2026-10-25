/* This Source Code Form is subject to the Mozilla Public License,v2.0. */
//! Bounded NSString/CF byte conversion without string contents in diagnostics.
//! Native/explicit Unicode byte encodings do not include a BOM here.
//! Legacy encoders use strict scalar mappings: unsupported sequences fail closed;
//! no normalization, transliteration, or lossy replacement is used by NSString.
use encoding_rs::{MACINTOSH,SHIFT_JIS,WINDOWS_1252};
pub(crate)struct Encoded{pub bytes:Vec<u8>,pub units:usize,pub complete:bool}
fn supported(e:u32)->bool{matches!(e,1|4|5|8|10|12|30|0x90000100|0x94000100|0x8c000100|0x98000100|0x9c000100)}
fn scalar(c:char,e:u32)->Option<Vec<u8>>{match e{
 1=>((c as u32)<128).then(||vec![c as u8]),5=>(c as u32<=255).then(||vec![c as u8]),
 4=>{let mut b=[0;4];Some(c.encode_utf8(&mut b).as_bytes().to_vec())},
 8|12|30=>{let codec=match e{8=>SHIFT_JIS,12=>WINDOWS_1252,_=>MACINTOSH};let text=c.to_string();let(b,_,bad)=codec.encode(&text);(!bad).then(||b.into_owned())},
 0x8c000100|0x9c000100=>Some((c as u32).to_le_bytes().to_vec()),0x98000100=>Some((c as u32).to_be_bytes().to_vec()),_=>None}}
pub(crate)fn partial(input:&[u16],encoding:u32,capacity:usize,loss_byte:u8)->Option<Encoded>{
 if !supported(encoding){return None;}let mut out=Encoded{bytes:Vec::new(),units:0,complete:false};
 if matches!(encoding,10|0x90000100|0x94000100){for &unit in input{if capacity.saturating_sub(out.bytes.len())<2{break;}out.bytes.extend_from_slice(&if encoding==0x90000100{unit.to_be_bytes()}else{unit.to_le_bytes()});out.units+=1;}}
 else{for c in char::decode_utf16(input.iter().copied()){let n=c.as_ref().map_or(1,|c|c.len_utf16());let bytes=c.ok().and_then(|c|scalar(c,encoding)).or_else(||(loss_byte!=0).then(||vec![loss_byte]));let Some(bytes)=bytes else{break;};if bytes.len()>capacity.saturating_sub(out.bytes.len()){break;}out.bytes.extend_from_slice(&bytes);out.units+=n;}}
 out.complete=out.units==input.len();Some(out)
}
pub(crate)fn full(input:&[u16],encoding:u32)->Option<Vec<u8>>{let out=partial(input,encoding,usize::MAX,0)?;out.complete.then_some(out.bytes)}
/// Preserve the established all-or-nothing destination-write contract. Cocoa's
/// getCString uses a single byte terminator even for explicit wide encodings.
pub(crate)fn c_buffer(input:&[u16],encoding:u32,capacity:usize,terminate:bool)->Option<Vec<u8>>{let mut bytes=if input.is_empty(){Vec::new()}else{full(input,encoding)?};let required=bytes.len().checked_add(usize::from(terminate))?;if required>capacity{return None;}if terminate{bytes.push(0);}Some(bytes)}
pub(crate)fn from_cf(e:u32)->Option<u32>{match e{0=>Some(30),0x600=>Some(1),0x201=>Some(5),0x500=>Some(12),0x420=>Some(8),0x8000100=>Some(4),0x100=>Some(10),0x10000100=>Some(0x90000100),0x14000100=>Some(0x94000100),0xc000100=>Some(0x8c000100),0x18000100=>Some(0x98000100),0x1c000100=>Some(0x9c000100),_=>None}}
pub(crate)fn to_cf(e:u32)->Option<u32>{match e{30=>Some(0),1=>Some(0x600),5=>Some(0x201),12=>Some(0x500),8=>Some(0x420),4=>Some(0x8000100),10=>Some(0x100),0x90000100=>Some(0x10000100),0x94000100=>Some(0x14000100),0x8c000100=>Some(0xc000100),0x98000100=>Some(0x18000100),0x9c000100=>Some(0x1c000100),_=>None}}
pub(crate)fn pascal_buffer(input:&[u16],encoding:u32,capacity:usize)->Option<Vec<u8>>{let bytes=full(input,encoding)?;let count=u8::try_from(bytes.len()).ok()?;let required=bytes.len().checked_add(1)?;if required>capacity{return None;}let mut out=Vec::with_capacity(required);out.push(count);out.extend_from_slice(&bytes);Some(out)}
