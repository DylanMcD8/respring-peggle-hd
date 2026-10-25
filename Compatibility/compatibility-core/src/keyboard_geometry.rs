/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. https://mozilla.org/MPL/2.0/ */
//! Map normalized native keyboard frames through the same viewport/rotation
//! contract as touches. No native screen pixels are confused with guest points.
pub fn map_rect(rect: [f32;4], viewport: [f32;4], guest: [f32;2], turns: u8) -> [f32;4] {
    if rect.iter().chain(viewport.iter()).chain(guest.iter()).any(|x|!x.is_finite())
        || viewport[2]<=0.0 || viewport[3]<=0.0 { return [0.0;4]; }
    let [x,y,w,h]=rect;
    let mut min=[f32::INFINITY;2]; let mut max=[f32::NEG_INFINITY;2];
    for p in [[x,y],[x+w,y],[x,y+h],[x+w,y+h]] {
        let a=(p[0]-viewport[0])/viewport[2]-0.5;
        let b=(p[1]-viewport[1])/viewport[3]-0.5;
        let p=match turns%4 { 0=>[a,b],1=>[b,-a],2=>[-a,-b],_=>[-b,a] };
        for i in 0..2 { let c=(p[i]+0.5)*guest[i]; min[i]=min[i].min(c); max[i]=max[i].max(c); }
    }
    [min[0],min[1],max[0]-min[0],max[1]-min[1]]
}

/// Legacy UIKeyboardBounds is in keyboard-local, visually upright coordinates.
/// Letterbox bars do not enlarge the guest keyboard bounds.
pub fn local_bounds(rect:[f32;4], viewport:[f32;4], guest:[f32;2], turns:u8)->[f32;4] {
    let w=rect[2].max(0.0).min(viewport[2]);
    let h=rect[3].max(0.0).min(viewport[3]);
    let r=map_rect([viewport[0],viewport[1],w,h],viewport,guest,turns);
    if turns%2==0 { [0.0,0.0,r[2],r[3]] } else { [0.0,0.0,r[3],r[2]] }
}
#[cfg(test)] mod tests {
    use super::*;
    fn near(a:[f32;4],b:[f32;4]) { for i in 0..4 { assert!((a[i]-b[i]).abs()<0.001,"{a:?} != {b:?}"); } }
    #[test] fn portrait_keyboard() { near(map_rect([0.0,0.75,1.0,0.25],[0.0,0.0,1.0,1.0],[768.0,1024.0],0),[0.0,768.0,768.0,256.0]); }
    #[test] fn landscape_sides_keep_keyboard_local_size() {
        for turn in [1,3] {
            near(local_bounds([0.0,0.5,1.0,0.5],[0.0,0.0,1.0,1.0],[768.0,1024.0],turn),[0.0,0.0,1024.0,384.0]);
        }
        near(map_rect([0.0,0.5,1.0,0.5],[0.0,0.0,1.0,1.0],[768.0,1024.0],1),[384.0,0.0,384.0,1024.0]);
        near(map_rect([0.0,0.5,1.0,0.5],[0.0,0.0,1.0,1.0],[768.0,1024.0],3),[0.0,0.0,384.0,1024.0]);
    }
    #[test] fn letterbox_clips_to_guest_width() { near(local_bounds([0.0,0.5,1.0,0.5],[0.125,0.0,0.75,1.0],[768.0,1024.0],1),[0.0,0.0,1024.0,384.0]); }
    #[test] fn hidden_frame_keeps_keyboard_local_bounds() { near(local_bounds([0.0,1.0,1.0,0.25],[0.0,0.0,1.0,1.0],[768.0,1024.0],1),[0.0,0.0,1024.0,192.0]); }
    #[test] fn invalid_geometry_is_bounded() { assert_eq!(map_rect([0.0;4],[0.0;4],[768.0,1024.0],1),[0.0;4]); assert_eq!(map_rect([f32::NAN;4],[0.0,0.0,1.0,1.0],[768.0,1024.0],1),[0.0;4]); }
}
