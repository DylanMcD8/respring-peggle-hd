/* This Source Code Form is subject to the Mozilla Public License,v2.0. */
//! Darwin wide date formatting, with explicit guest/host layouts and C locale.
use crate::{Environment,mem::{ConstPtr,ConstVoidPtr,MutPtr,GuestUSize}};
use crate::dyld::{export_c_func,FunctionExports};
use super::wchar::wchar_t;
use std::sync::atomic::{AtomicU64,Ordering};
const MAX_FORMAT:usize=65536;
const MAX_OUTPUT:usize=1048576;
const MAX_ZONE:usize=128;
static CALLS:AtomicU64=AtomicU64::new(0);
#[derive(Clone,Copy,Debug)]pub(crate)struct Calendar{pub fields:[i32;10]}
impl Calendar{fn valid(self)->bool{let t=self.fields;(0..=60).contains(&t[0])&&(0..60).contains(&t[1])&&(0..24).contains(&t[2])&&(1..=31).contains(&t[3])&&(0..12).contains(&t[4])&&t[5].checked_add(1900).is_some_and(|y|(0..=9999).contains(&y))&&(0..7).contains(&t[6])&&(0..366).contains(&t[7])}}
fn mapped(mem:&crate::mem::Mem,a:u32,n:u32)->bool{a>=mem.null_segment_size()&&a.checked_add(n).is_some()&&mem.is_mapped_range(a,n)}
fn read_format(mem:&crate::mem::Mem,ptr:ConstPtr<wchar_t>)->Option<Vec<i32>>{let mut v=Vec::new();for i in 0..MAX_FORMAT{let a=ptr.to_bits().checked_add((i as u32).checked_mul(4)?)?;if !mapped(mem,a,4){return None;}let x=mem.read(ConstPtr::<i32>::from_bits(a));if x==0{return Some(v);}if char::from_u32(u32::try_from(x).ok()?).is_none(){return None;}v.push(x);}None}
fn read_calendar(mem:&crate::mem::Mem,ptr:ConstVoidPtr)->Option<(Calendar,Vec<u8>)>{if !mapped(mem,ptr.to_bits(),44){return None;}let b=mem.bytes_at(ptr.cast::<u8>(),44);let mut fields=[0;10];for(i,x)in fields.iter_mut().enumerate(){*x=i32::from_le_bytes(b[i*4..i*4+4].try_into().ok()?);}let calendar=Calendar{fields};if !calendar.valid(){return None;}let zone=u32::from_le_bytes(b[40..44].try_into().ok()?);if zone==0{return Some((calendar,b"GMT".to_vec()));}let mut bytes=Vec::new();for i in 0..MAX_ZONE{let a=zone.checked_add(i as u32)?;if !mapped(mem,a,1){return None;}let c=mem.read(ConstPtr::<u8>::from_bits(a));if c==0{return Some((calendar,bytes));}if !c.is_ascii(){return None;}bytes.push(c);}None}
// Darwin's native %z can consult the host process timezone instead of the
// supplied tm_gmtoff. Expand guest timezone directives to escaped literals;
// never mutate TZ/tzset/uselocale or let a guest zone become a format directive.
fn prepare_format(format:&[i32],calendar:Calendar,zone:&[u8])->Option<Vec<i32>>{
 let mut out=Vec::new();let mut i=0;
 while i<format.len(){let c=format[i];i+=1;if c!=37{out.push(c);}else{
  if i==format.len(){out.push(c);break;}let mut end=i;if matches!(format[end],69|79){end+=1;if end==format.len(){out.extend_from_slice(&format[i-1..]);break;}}
  let spec=format[end];if spec==122||spec==90{let literal=if spec==90{zone.to_vec()}else if calendar.fields[8]<0{Vec::new()}else{let seconds=i64::from(calendar.fields[9]);let a=seconds.abs();format!("{}{:02}{:02}",if seconds<0{"-"}else{"+"},a/3600,a%3600/60).into_bytes()};for c in literal{out.push(i32::from(c));if c==b'%'{out.push(37);}}i=end+1;
  }else if spec==115{return None;}else{out.push(37);out.push(format[i]);i+=1;}
 }if out.len()>=MAX_FORMAT{return None;}}
 Some(out)
}
/// No guest addresses or callbacks enter libc. Locale is private to this call;
/// process/thread locale and guest calendar/timezone policy are unchanged.
#[cfg(any(target_os="ios",target_os="macos"))]
pub(crate)fn format_native(format:&[i32],max:usize,calendar:Calendar,zone:&[u8])->Option<Vec<i32>>{
 use ::libc as host;
 unsafe extern "C"{fn wcsftime_l(out:*mut host::wchar_t,max:usize,format:*const host::wchar_t,time:*const host::tm,locale:host::locale_t)->usize;}
 if max==0||max>MAX_OUTPUT||format.len()>=MAX_FORMAT||!calendar.valid()||format.iter().any(|x|*x==0||u32::try_from(*x).ok().and_then(char::from_u32).is_none()){return None;}
 let prepared=prepare_format(format,calendar,zone)?;
 let zone=std::ffi::CString::new(zone).ok()?;
 let mut tm:host::tm=unsafe{std::mem::zeroed()};let t=calendar.fields;
 tm.tm_sec=t[0];tm.tm_min=t[1];tm.tm_hour=t[2];tm.tm_mday=t[3];tm.tm_mon=t[4];tm.tm_year=t[5];tm.tm_wday=t[6];tm.tm_yday=t[7];tm.tm_isdst=t[8];tm.tm_gmtoff=t[9]as host::c_long;tm.tm_zone=zone.as_ptr().cast_mut();
 let mut f=prepared.clone();f.push(0);let mut out=vec![i32::MAX;max];
 let locale=unsafe{host::newlocale(host::LC_ALL_MASK,c"C".as_ptr(),std::ptr::null_mut())};if locale.is_null(){return None;}
 struct Locale(host::locale_t);impl Drop for Locale{fn drop(&mut self){unsafe{host::freelocale(self.0);}}}let locale=Locale(locale);
 let n=unsafe{wcsftime_l(out.as_mut_ptr(),max,f.as_ptr(),&tm,locale.0)};
 if n==0{if prepared.is_empty(){return Some(vec![0]);}return None;}
 if n>=max||out[n]!=0||out[..n].contains(&0){return None;}out.truncate(n+1);Some(out)
}
#[cfg(not(any(target_os="ios",target_os="macos")))]
pub(crate)fn format_native(_format:&[i32],_max:usize,_calendar:Calendar,_zone:&[u8])->Option<Vec<i32>>{None}
fn report(kind:&str,capacity:u32,format_units:usize,result:u32){let n=CALLS.fetch_add(1,Ordering::Relaxed)+1;if n<=64{crate::diagnostics::event(&format!("{{\"event\":\"wide_time_format\",\"kind\":\"{kind}\",\"call\":{n},\"capacity_wchars\":{capacity},\"format_units\":{format_units},\"result_wchars\":{result},\"text_contents_logged\":false}}"));}}
pub(crate)fn wcsftime(env:&mut Environment,dest:MutPtr<wchar_t>,max:GuestUSize,format:ConstPtr<wchar_t>,time:ConstVoidPtr)->GuestUSize{
 // C maxsize counts wchar_t elements, not bytes. A zero capacity never touches
 // caller memory. All input is snapshotted before any successful output commit.
 if max==0{report("zero_capacity",max,0,0);return 0;}
 let Some(bytes)=max.checked_mul(4).filter(|_|max as usize<=MAX_OUTPUT)else{report("capacity_declined",max,0,0);return 0;};
 if !mapped(&env.mem,dest.to_bits(),bytes){report("destination_declined",max,0,0);return 0;}
 let Some(format)=read_format(&env.mem,format)else{report("format_declined",max,0,0);return 0;};let units=format.len();
 let Some((calendar,zone))=read_calendar(&env.mem,time)else{report("calendar_declined",max,units,0);return 0;};
 let Some(out)=format_native(&format,max as usize,calendar,&zone)else{report("format_or_capacity_declined",max,units,0);return 0;};
 let n=(out.len()-1)as u32;for(i,c)in out.into_iter().enumerate(){env.mem.write(dest+i as u32,c);}report("formatted",max,units,n);n
}
pub const FUNCTIONS:FunctionExports=&[export_c_func!(wcsftime(_,_,_,_))];
