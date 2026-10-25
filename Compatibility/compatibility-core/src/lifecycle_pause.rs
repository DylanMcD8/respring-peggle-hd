//! Small deterministic contracts shared by the opt-in lifecycle adapter.
use std::time::{Duration, Instant};
use std::collections::HashMap;
use std::hash::Hash;

pub fn drain_interrupted_touches<K: Eq + Hash, T>(touches: &mut HashMap<K, T>) -> HashMap<K, T> {
    std::mem::take(touches)
}

pub fn group_cancelled_touches<T: Copy, V: Eq + Hash>(items: impl IntoIterator<Item = (T, V)>) -> HashMap<V, Vec<T>> {
    let mut groups = HashMap::new();
    for (touch, view) in items { groups.entry(view).or_insert_with(Vec::new).push(touch); }
    groups
}

pub fn shifted(deadline: Instant, pause: Duration) -> Instant {
    deadline.checked_add(pause).expect("Lifecycle deadline overflow")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DispatchPlan { Direct, Queued }

pub fn dispatch_plan(current: usize, target: usize, wait: bool, alive: bool) -> Option<DispatchPlan> {
    if !alive { None } else if wait && current == target { Some(DispatchPlan::Direct) }
    else { Some(DispatchPlan::Queued) }
}

pub fn audio_resume_allowed(was_playing: bool, still_exists: bool, now_paused: bool) -> bool {
    was_playing && still_exists && now_paused
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PauseSignal { Inactive, Active, Terminated, Other }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaitDecision { Continue, Resume, Terminate }
#[derive(Default)]
pub struct SuspendGate { completed: Option<WaitDecision> }
impl SuspendGate {
    pub fn observe(&mut self, signal: PauseSignal) -> WaitDecision {
        if let Some(done) = self.completed { return done; }
        let decision = match signal {
            PauseSignal::Active => WaitDecision::Resume,
            PauseSignal::Terminated => WaitDecision::Terminate,
            PauseSignal::Inactive | PauseSignal::Other => WaitDecision::Continue,
        };
        if decision != WaitDecision::Continue { self.completed = Some(decision); }
        decision
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suspension_preserves_remaining_deadline_and_guest_elapsed_time() {
        let startup = Instant::now();
        let before = startup + Duration::from_secs(20);
        let deadline = before + Duration::from_millis(16);
        let pause = Duration::from_secs(600);
        let after = before + pause;
        assert_eq!(shifted(deadline, pause).duration_since(after), Duration::from_millis(16));
        assert_eq!(after.duration_since(shifted(startup, pause)), before.duration_since(startup));
    }

    #[test]
    fn repeated_pauses_do_not_accumulate_timer_catchup() {
        let initial = Instant::now();
        let first = shifted(initial, Duration::from_secs(30));
        let second = shifted(first, Duration::from_secs(90));
        assert_eq!(second.duration_since(initial), Duration::from_secs(120));
        assert_eq!(shifted(initial, Duration::ZERO), initial);
    }

    #[test]
    fn selector_wait_on_same_thread_does_not_deadlock() {
        assert_eq!(dispatch_plan(2, 2, true, true), Some(DispatchPlan::Direct));
        assert_eq!(dispatch_plan(2, 2, false, true), Some(DispatchPlan::Queued));
        assert_eq!(dispatch_plan(2, 1, true, true), Some(DispatchPlan::Queued));
        assert_eq!(dispatch_plan(2, 1, false, false), None);
    }

    #[test]
    fn audio_restores_only_previously_playing_still_paused_sources() {
        assert!(audio_resume_allowed(true, true, true));
        assert!(!audio_resume_allowed(false, true, true));
        assert!(!audio_resume_allowed(true, false, true));
        assert!(!audio_resume_allowed(true, true, false));
    }

    #[test]
    fn repeated_lifecycle_sequences_resume_only_on_activation() {
        for _ in 0..8 {
            let mut gate = SuspendGate::default();
            assert_eq!(gate.observe(PauseSignal::Inactive), WaitDecision::Continue);
            assert_eq!(gate.observe(PauseSignal::Inactive), WaitDecision::Continue);
            assert_eq!(gate.observe(PauseSignal::Other), WaitDecision::Continue);
            assert_eq!(gate.observe(PauseSignal::Active), WaitDecision::Resume);
            assert_eq!(gate.observe(PauseSignal::Active), WaitDecision::Resume);
        }
    }

    #[test]
    fn termination_cannot_accidentally_resume_retained_state() {
        let mut gate = SuspendGate::default();
        assert_eq!(gate.observe(PauseSignal::Terminated), WaitDecision::Terminate);
        assert_eq!(gate.observe(PauseSignal::Active), WaitDecision::Terminate);
    }

    #[test]
    fn held_touches_are_cancelled_once_and_grouped_by_original_view() {
        let mut held = HashMap::from([(1, (101, 10)), (2, (102, 10)), (3, (103, 20))]);
        let cancelled = drain_interrupted_touches(&mut held);
        assert!(held.is_empty());
        let mut groups = group_cancelled_touches(cancelled.into_values());
        groups.get_mut(&10).unwrap().sort_unstable();
        assert_eq!(groups[&10], vec![101, 102]);
        assert_eq!(groups[&20], vec![103]);
        assert!(drain_interrupted_touches(&mut held).is_empty());
        held.insert(4, (104, 20));
        assert_eq!(held.len(), 1); // Fresh resumed touch is independent.
    }
}
