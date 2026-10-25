/* MPL-2.0. Wrapper raster geometry; independent of guest timing and assets. */
#[derive(Clone,Copy,Debug,PartialEq,Eq)]pub enum Mode{Original,IPadRetina,Device}
impl Mode{pub fn parse(s:&str)->Result<Self,&'static str>{match s{"original"=>Ok(Self::Original),"retina"=>Ok(Self::IPadRetina),"device"=>Ok(Self::Device),_=>Err("Resolution must be original, retina or device")}}pub fn name(self)->&'static str{match self{Self::Original=>"original",Self::IPadRetina=>"retina",Self::Device=>"device"}}}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]pub struct Size{pub width:u32,pub height:u32}
impl Size{pub const fn new(width:u32,height:u32)->Self{Self{width,height}}pub fn portrait(self)->Self{Self::new(self.width.min(self.height),self.width.max(self.height))}pub fn valid(self)->bool{self.width>0&&self.height>0&&self.width<=8192&&self.height<=8192&&u64::from(self.width)*u64::from(self.height)<=16_777_216}}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]pub enum Aspect{IPad4x3,Phone3x2,Phone16x9}
impl Aspect{pub fn landscape(self)->(u32,u32){match self{Self::IPad4x3=>(4,3),Self::Phone3x2=>(3,2),Self::Phone16x9=>(16,9)}}}
/// Exact integer aspect, centered without stretch/crop; residual pixels are bars.
pub fn fit(available:Size,aspect:Aspect)->Option<(u32,u32,u32,u32)>{let(a,b)=aspect.landscape();let k=(available.width/a).min(available.height/b);if k==0{return None;}let(w,h)=(a*k,b*k);Some(((available.width-w)/2,(available.height-h)/2,w,h))}
pub fn hd_target(mode:Mode,drawable:Size)->Result<Size,&'static str>{let target=match mode{Mode::Original=>Size::new(768,1024),Mode::IPadRetina=>Size::new(1536,2048),Mode::Device=>{let d=drawable.portrait();let(_,_,w,h)=fit(Size::new(d.height,d.width),Aspect::IPad4x3).ok_or("Drawable too small")?;Size::new(h,w)}};if target.valid(){Ok(target)}else{Err("Render target exceeds supported8192edge/16MP budget")}}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]pub struct Surface{pub guest:Size,pub actual:Size}
impl Surface{
 pub fn hd(guest:Size,target_portrait:Size)->Option<Self>{if !matches!((guest.width,guest.height),(768,1024)|(1024,768)|(1536,2048)|(2048,1536)){return None;}let p=target_portrait.portrait();Some(Self{guest,actual:if guest.width<guest.height{p}else{Size::new(p.height,p.width)}})}
 pub fn rect(self,r:[i32;4])->Option<[i32;4]>{if r[2]<0||r[3]<0{return None;}fn edge(x:i64,n:u32,d:u32)->Option<i32>{let v=i128::from(x)*i128::from(n);let half=i128::from(d/2);let scaled=if v<0{-((-v+half)/i128::from(d))}else{(v+half)/i128::from(d)};i32::try_from(scaled).ok()}let x=edge(r[0]as i64,self.actual.width,self.guest.width)?;let y=edge(r[1]as i64,self.actual.height,self.guest.height)?;let x2=edge(r[0]as i64+r[2]as i64,self.actual.width,self.guest.width)?;let y2=edge(r[1]as i64+r[3]as i64,self.actual.height,self.guest.height)?;Some([x,y,x2.checked_sub(x)?,y2.checked_sub(y)?])}
}
/// Preserve guest-size screenshots/copies while the actual main target is larger.
/// Rows stay GL bottom-up. Pixel-center nearest sampling changes no GPU state.
pub fn logical_rgba(surface:Surface,physical:&[u8])->Option<Vec<u8>>{let a=surface.actual;let g=surface.guest;if !a.valid()||!g.valid()||physical.len()!=a.width as usize*a.height as usize*4{return None;}let mut out=vec![0;g.width as usize*g.height as usize*4];for y in 0..g.height{let sy=((u64::from(y)*2+1)*u64::from(a.height)/(u64::from(g.height)*2)).min(u64::from(a.height-1))as usize;for x in 0..g.width{let sx=((u64::from(x)*2+1)*u64::from(a.width)/(u64::from(g.width)*2)).min(u64::from(a.width-1))as usize;let src=(sy*a.width as usize+sx)*4;let dst=(y as usize*g.width as usize+x as usize)*4;out[dst..dst+4].copy_from_slice(&physical[src..src+4]);}}Some(out)}

/// Legacy coordinate-limit helper; inputs are not evidence of physical pixels.
/// Full Device uses the active system backing, respecting Display Zoom.
pub fn native_available(drawable:Size,panel:Size)->Size{let d=drawable.portrait();if panel.width==0||panel.height==0{return Size::new(d.height,d.width);}let p=panel.portrait();Size::new(d.height.min(p.height),d.width.min(p.width))}

pub fn packed_image_size(row_bytes:u32,height:u32,alignment:u32)->Option<u32>{if !matches!(alignment,1|2|4|8){return None;}if height==0{return Some(0);}let stride=row_bytes.checked_add(alignment-1)?/alignment*alignment;stride.checked_mul(height-1)?.checked_add(row_bytes)}

/// Resolve only after the host GL backing has been created and made current.
/// SDL's pre-context drawable query may return points rather than pixels.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct BackingPlan {
    pub target: Size,
    pub available: Size,
    pub expected_from_scale: Option<Size>,
    pub driver_matches_drawable: bool,
    pub driver_matches_native_scale: bool,
    pub scale_tolerance_pixels: u32,
    pub drawable_matches_native_scale: bool,
    pub authority: &'static str,
}
pub fn after_backing(
    mode: Mode, context_ready: bool, drawable: Size, points: Size,
    driver: Option<Size>, native_panel: Size, native_scale: f64,
) -> Result<BackingPlan,&'static str> {
    if !context_ready { return Err("Wrapper target requested before host GL context/backing"); }
    fn known(s: Size)->bool { s.width>0 && s.height>0 }
    fn close(a: Size,b: Size,tolerance:u32)->bool {
        let a=a.portrait(); let b=b.portrait();
        a.width.abs_diff(b.width)<=tolerance && a.height.abs_diff(b.height)<=tolerance
    }
    let valid_scale=native_scale.is_finite() && native_scale>0.0 && native_scale<=8.0;
    let expected_from_scale=if valid_scale && known(points) {
        let w=(points.width as f64*native_scale).round();
        let h=(points.height as f64*native_scale).round();
        if w>=1.0 && h>=1.0 && w<=u32::MAX as f64 && h<=u32::MAX as f64 {
            Some(Size::new(w as u32,h as u32))
        } else { None }
    } else { None };
    // SDL point sizes are integral; a fractional UIKit bound can account for
    // up to one point of rounding at the display's scale, not a 1x/2x mistake.
    let scale_tolerance_pixels=if valid_scale {native_scale.ceil()as u32}else{0};
    let driver_matches_drawable=driver.is_some_and(|s|known(s)&&known(drawable)&&close(s,drawable,1));
    let driver_matches_native_scale=driver.zip(expected_from_scale).is_some_and(|(s,e)|close(s,e,scale_tolerance_pixels));
    let drawable_matches_native_scale=expected_from_scale.is_some_and(|e|drawable.valid()&&close(drawable,e,scale_tolerance_pixels));
    let authority=if mode==Mode::Device {
        if !drawable.valid(){return Err("Full Device drawable is missing or exceeds the supported pixel budget");}
        if driver.is_some() {
            if !driver_matches_drawable{return Err("Full Device current-context backing disagrees with SDL pixels");}
            if expected_from_scale.is_some()&&!driver_matches_native_scale{return Err("Full Device backing disagrees with window points and native scale");}
            "current_context_renderbuffer"
        } else if drawable_matches_native_scale {
            // SDL_GetDrawableSize uses current.sdlView.backingWidth/Height in
            // the pinned local SDL build. Independent points*scale confirms
            // pixel units, rejecting the pre-context1x size seen in C45.
            "scaled_sdl_drawable_native_query_unavailable"
        } else {return Err("Full Device has no validated current backing or scaled SDL drawable");}
    } else if mode==Mode::Original {"guest_requested_raster"} else {"fixed_retina"};
    // The active drawable is authoritative. UIKit nativeBounds is diagnostic
    // metadata, not a physical-panel cap: More Space may render above the panel.
    let authoritative=driver.filter(|_|driver_matches_drawable).unwrap_or(drawable).portrait();
    let available=Size::new(authoritative.height,authoritative.width);
    let _=native_panel; // retained API input for diagnostic/source compatibility
    Ok(BackingPlan {target:hd_target(mode,available)?,available,expected_from_scale,
        driver_matches_drawable,driver_matches_native_scale,scale_tolerance_pixels,
        drawable_matches_native_scale,authority})
}

/// Original mode keeps high-DPI presentation but never overrides guest storage.
pub fn raster_target(mode:Mode,target:Size)->Option<Size>{if mode==Mode::Original{None}else{Some(target)}}
pub fn virtualizes_raster(mode:Option<Mode>)->bool{mode.is_some_and(|m|m!=Mode::Original)}
