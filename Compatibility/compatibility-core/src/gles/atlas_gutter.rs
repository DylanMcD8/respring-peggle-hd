/* MPL-2.0. Exact static UI atlas border replication for higher-resolution sampling. */
//! Original source rectangles, pixels, geometry and UVs stay unchanged.
//! Trim gutters clamp nearest edges; the scrolling wheel wraps vertical edges.
//! Only unused transparent padding changes. At1x texel centers avoid gutters.
use digest::Digest;
use std::sync::atomic::{AtomicBool,Ordering};
static ENABLED:AtomicBool=AtomicBool::new(false);
pub fn configure(verified:bool,wrapper:bool){ENABLED.store(verified&&wrapper,Ordering::Relaxed);}
pub const RECTS:[(usize,usize,usize,usize);6]=[(2,2,130,768),(136,2,130,768),(270,2,175,144),(449,2,28,806),(2,774,284,73),(2,851,284,73)];
const HASHES:[&str;11]=["ce1dff5f283f212e616d3e85700fd7083f8ba958","780c49b7da84e7ea20d4f99fc455a61cfda99cc4","7fe868a7089e06830d8d77cc211f259bc15b2d87","563901eef7d717411d70d6857511f9d1afea900c","85416a3e2f2cce9caffd3c8cf8fd392ecc3015ed","bac303a63eb67cd3a178242dfd4e5206132f2475","adba82d7a225267381500fc98beb54ceb2eb4d99","3a1f53438fb49f358da6a5ec25c7d18f24d1498b","c10316944d64a90ce61a8a72c26e8b72da5b0139","4cb22e94a5efa892a1fc40432ed7f627df205d20","ccaffe35725be4a06f37670299066d881144d92d"];
pub fn padded(source:&[u8])->Option<Vec<u8>>{
 if source.len()!=512*1024*4||!HASHES.contains(&format!("{:x}",sha1::Sha1::digest(source)).as_str()){return None;}
 let mut out=source.to_vec();
 for(x,y,w,h)in RECTS{
  for py in y-2..y+h+2{for px in x-2..x+w+2{
   if px>=x&&px<x+w&&py>=y&&py<y+h{continue;}
   let dst=(py*512+px)*4;
   // Guard against modifying artwork or any neighboring occupied texel.
   if source[dst..dst+4]!=[0,0,0,0]{continue;}
   let sx=px.clamp(x,x+w-1);let sy=py.clamp(y,y+h-1);let src=(sy*512+sx)*4;
   out[dst..dst+4].copy_from_slice(&source[src..src+4]);
  }}
 }
 Some(out)
}
// Only this single scrolling source rectangle is changed in the larger atlas.
// DrawDial repeats this337-row image in four adjacent quads. The vertical
// padding therefore wraps opposite rows; horizontal padding clamps the edge.
pub const WHEEL_RECT:(usize,usize,usize,usize)=(906,2,75,337);
pub fn padded_wheel(source:&[u8])->Option<Vec<u8>>{
 if source.len()!=1024*2048*4||format!("{:x}",sha1::Sha1::digest(source))!="94af9d2ba976522238931f5b433c9ec203f6b606"{return None;}
 let(x,y,w,h)=WHEEL_RECT;let mut out=source.to_vec();
 for py in y-2..y+h+2{for px in x-2..x+w+2{
  if px>=x&&px<x+w&&py>=y&&py<y+h{continue;}
  let dst=(py*1024+px)*4;if source[dst..dst+4]!=[0,0,0,0]{continue;}
  let sx=px.clamp(x,x+w-1);let sy=if py<y{y+h-(y-py)}else if py>=y+h{y+(py-y-h)}else{py};let src=(sy*1024+sx)*4;
  out[dst..dst+4].copy_from_slice(&source[src..src+4]);
 }}Some(out)
}
pub fn supported_size(w:i32,h:i32)->bool{matches!((w,h),(512,1024)|(1024,2048))}
pub fn for_upload(w:i32,h:i32,rgba8:bool,source:&[u8])->Option<Vec<u8>>{
 if !ENABLED.load(Ordering::Relaxed)||!rgba8{return None;}
 match(w,h){(512,1024)=>padded(source),(1024,2048)=>padded_wheel(source),_=>None}
}
