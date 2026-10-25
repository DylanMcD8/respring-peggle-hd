/* This Source Code Form is subject to the Mozilla Public License,v2.0. */
//! Only the verified game's repeating direct render fallback is adaptable.
pub(crate)fn eligible(enabled:bool,repeats:bool,selector:&str)->bool{enabled&&repeats&&selector=="renderGLScene"}
#[cfg(test)]mod tests{use super::*;
 #[test]fn redirects_audio_gameplay_one_shots_and_unverified_timers_remain_original(){assert!(eligible(true,true,"renderGLScene"));for s in ["renderGLScene:","_touchHLE_displayLinkTimerDidFire:","update","UpdateFrames","audioQueue","animationDidStop:",""]{assert!(!eligible(true,true,s));}assert!(!eligible(false,true,"renderGLScene"));assert!(!eligible(true,false,"renderGLScene"));}
}

/// Exact verified render routes only; ordinary timers retain existing semantics.
pub(crate)fn strict_schedule(direct_adaptive:bool,display_link_render:bool,adaptive_enabled:bool)->bool{direct_adaptive||(display_link_render&&adaptive_enabled)}
#[cfg(test)]mod schedule_route_tests{use super::*;#[test]fn only_direct_or_enabled_verified_display_links_share_render_scheduler(){for direct in [false,true]{for display in [false,true]{for enabled in [false,true]{assert_eq!(strict_schedule(direct,display,enabled),direct||(display&&enabled));}}}}
}
