/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
//! Stateful linear PCM rate conversion for the legacy 3D mixer playback rate.
#[derive(Clone, Default)]
pub struct RateState { phase: f64, channels: usize, samples: Vec<f32> }
impl RateState {
    pub fn clear(&mut self) { *self=Self::default(); }
    pub fn valid_rate(rate: f64) -> bool { rate.is_finite() && (0.25..=4.0).contains(&rate) }
    fn required(&self, frames: usize, rate: f64) -> Option<usize> {
        if frames==0 || frames>16384 || !Self::valid_rate(rate) { return None; }
        let last=self.phase+(frames-1) as f64*rate;
        let lookahead=usize::from(last.fract()>1e-10);
        let sampled=last.floor() as usize+1+lookahead;
        let consumed=(self.phase+frames as f64*rate).floor() as usize;
        Some(sampled.max(consumed))
    }
    pub fn additional_frames(&mut self, frames: usize, rate: f64, channels: usize) -> Option<usize> {
        if !(1..=2).contains(&channels) { return None; }
        if self.channels!=channels { self.clear();self.channels=channels; }
        Some(self.required(frames,rate)?.saturating_sub(self.samples.len()/channels))
    }
    pub fn append(&mut self, samples: &[f32]) -> bool {
        if self.channels==0 || !samples.len().is_multiple_of(self.channels) { return false; }
        self.samples.extend_from_slice(samples);true
    }
    pub fn render(&mut self, frames: usize, rate: f64) -> Option<Vec<f32>> {
        let required=self.required(frames,rate)?;
        if self.channels==0 || self.samples.len()/self.channels<required { return None; }
        let mut result=Vec::with_capacity(frames*self.channels);
        for frame in 0..frames {
            let position=self.phase+frame as f64*rate;let index=position.floor() as usize;let fraction=position.fract() as f32;
            for channel in 0..self.channels {
                let a=self.samples[index*self.channels+channel];
                let b=self.samples.get((index+1)*self.channels+channel).copied().unwrap_or(a);
                result.push(a+(b-a)*fraction);
            }
        }
        let advance=self.phase+frames as f64*rate;let consumed=advance.floor() as usize;
        self.phase=advance-consumed as f64;if self.phase<1e-10 { self.phase=0.0; }
        self.samples.drain(..consumed*self.channels);
        Some(result)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn ramp(state:&mut RateState, frames:usize, rate:f64, cursor:&mut usize, channels:usize)->Vec<f32> {
        let need=state.additional_frames(frames,rate,channels).unwrap();let mut input=Vec::new();
        for i in *cursor..*cursor+need { for c in 0..channels { input.push(i as f32+100.0*c as f32); } }
        *cursor+=need;assert!(state.append(&input));state.render(frames,rate).unwrap()
    }
    #[test] fn unity_is_exact_without_lookahead_consumption() {let mut s=RateState::default();let mut c=0;assert_eq!(ramp(&mut s,4,1.0,&mut c,1),[0.,1.,2.,3.]);assert_eq!(c,4);assert_eq!(ramp(&mut s,2,1.0,&mut c,1),[4.,5.]);}
    #[test] fn double_pitch_advances_twice_source_frames() {let mut s=RateState::default();let mut c=0;assert_eq!(ramp(&mut s,4,2.0,&mut c,1),[0.,2.,4.,6.]);assert_eq!(c,8);assert_eq!(ramp(&mut s,2,2.0,&mut c,1),[8.,10.]);}
    #[test] fn half_pitch_keeps_lookahead_across_blocks() {let mut s=RateState::default();let mut c=0;assert_eq!(ramp(&mut s,4,0.5,&mut c,1),[0.,0.5,1.,1.5]);assert_eq!(ramp(&mut s,4,0.5,&mut c,1),[2.,2.5,3.,3.5]);}
    #[test] fn fractional_phase_is_continuous_for_stereo() {let mut s=RateState::default();let mut c=0;let a=ramp(&mut s,3,0.75,&mut c,2);let b=ramp(&mut s,3,0.75,&mut c,2);assert_eq!(a,[0.,100.,0.75,100.75,1.5,101.5]);assert_eq!(b,[2.25,102.25,3.,103.,3.75,103.75]);}
    #[test] fn rate_change_preserves_source_position() {let mut s=RateState::default();let mut c=0;let _=ramp(&mut s,3,0.75,&mut c,1);assert_eq!(ramp(&mut s,2,1.0,&mut c,1),[2.25,3.25]);}
    #[test] fn invalid_rates_and_short_pcm_are_rejected() {for r in [0.,-1.,f64::NAN,f64::INFINITY,5.] {assert!(!RateState::valid_rate(r));}let mut s=RateState::default();assert_eq!(s.additional_frames(2,1.0,1),Some(2));assert!(s.render(2,1.0).is_none());}
    #[test] fn disable_or_format_change_discards_old_voice() {let mut s=RateState::default();let mut c=0;let _=ramp(&mut s,3,0.5,&mut c,1);s.clear();c=100;assert_eq!(ramp(&mut s,1,1.0,&mut c,1),[100.]);}
    #[test] fn sine_frequency_doubles_with_rate() {let mut s=RateState::default();let frames=4410;let n=s.additional_frames(frames,2.0,1).unwrap();let input:Vec<f32>=(0..n).map(|i|(std::f32::consts::TAU*440.0*i as f32/44100.0).sin()).collect();s.append(&input);let output=s.render(frames,2.0).unwrap();let crossing=output.windows(2).filter(|x|x[0]<=0.&&x[1]>0.).count();assert!((87..=89).contains(&crossing));}
}
