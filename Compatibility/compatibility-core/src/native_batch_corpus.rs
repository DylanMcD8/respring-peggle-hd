use crate::mem::{MutPtr,Mem};
fn put(m:&mut Mem,a:u32,v:u32){m.write(MutPtr::<u32>::from_bits(a),v)}
fn random(s:&mut u32)->u32{*s=s.wrapping_mul(1664525).wrapping_add(1013904223);*s}
pub(crate)fn seed(mem:&mut crate::mem::Mem,mut c:crate::cpu::CpuContext,r:usize,n:u32,base:u32,sp:u32)->crate::cpu::CpuContext{let mut seed=0xc3512345u32.wrapping_add(n);mem.bytes_at_mut(MutPtr::<u8>::from_bits(base),131072).fill((n*73)as u8);mem.bytes_at_mut(MutPtr::<u8>::from_bits(sp-256),512).fill((n*91)as u8);for x in c.regs.iter_mut().chain(c.extregs.iter_mut()){*x=random(&mut seed);}c.cpsr=0x08000010|((n&15)<<28);c.fpscr=random(&mut seed)&0xf800009f;c.regs[13]=sp;c.regs[14]=random(&mut seed)|1;c.regs[15]=0x3004;
 match r{
  0=>{c.regs[0]=base+256;c.regs[3]=base+512;put(mem,sp,base+516);for off in [28,32]{put(mem,c.regs[0]+off,if n%5==0{0}else{((n as f32-125.0)/129.0).to_bits()});}for off in [60,64]{put(mem,c.regs[0]+off,n%1024+1);}for off in [36,40]{put(mem,c.regs[0]+off,random(&mut seed));}}
  1=>{if n%5==0{c.regs[0]&=0xffffff;}},
  2=>{c.regs[0]=base+256;for off in [0,4,8,12]{put(mem,c.regs[0]+off,random(&mut seed));}},
  3=>{c.regs[0]=base+256;mem.write(MutPtr::<u8>::from_bits(c.regs[0]+49),(n as u8)|1);}
  4..=6=>{c.regs[0]=base+256;c.regs[1]=if n%7==0{c.regs[0]}else{base+512};for a in [c.regs[0],c.regs[1]]{put(mem,a,random(&mut seed));}}
  7..=13=>{c.regs[0]=base+256;c.regs[1]=base+8192;c.regs[2]=n.wrapping_mul(73);c.regs[3]=if n&8==0{0}else{128};if r!=7{c.regs[2]=c.regs[3];}let(color,channels)=match n%4{0=>(0u8,1u8),1=>(2,3),2=>(4,2),_=>(6,4)};let bits=if r==7||n&4==0{8u8}else{16};let width=n%129+1;put(mem,c.regs[0],width);put(mem,c.regs[0]+4,width*channels as u32*(bits/8)as u32);for(off,value)in[(8,color),(9,bits),(10,channels),(11,channels*bits)]{mem.write(MutPtr::<u8>::from_bits(c.regs[0]+off),value);}for i in 0..width*8{mem.write(MutPtr::<u8>::from_bits(c.regs[1]+i),random(&mut seed)as u8);}}
  14=>{c.regs[0]=base+256;c.regs[1]=base+512;let a=base+1024;let b=if n%3==0{base+1280}else{a};let x=base+1536;let y=base+1792;for(p,v)in[(c.regs[0],a),(c.regs[1],b),(c.regs[0]+4,x),(c.regs[1]+4,y)]{put(mem,p,v);}for obj in [a,b,x,y]{put(mem,obj+16,if n%3==2{73}else{random(&mut seed)});put(mem,obj+88,random(&mut seed));}}
  _=>panic!()
 }c}
