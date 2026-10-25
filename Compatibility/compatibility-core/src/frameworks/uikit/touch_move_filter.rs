//! Remove only movement samples already suppressed by UITouch dispatch.
//! Never merge changed samples or remove unknown identities/phase boundaries.
use std::collections::HashMap;
use std::hash::Hash;

pub(super) fn discard_unchanged<K: Eq + Hash>(
    moves: &mut HashMap<K, (f32, f32)>,
    current: impl Fn(&K) -> Option<(f32, f32)>,
) -> usize {
    let before = moves.len();
    moves.retain(|finger, coords| current(finger) != Some(*coords));
    before - moves.len()
}

#[cfg(test)]
mod tests {
    use super::discard_unchanged;
    use std::collections::HashMap;

    #[test]
    fn mixed_multitouch_retains_changed_and_unknown_identities() {
        let mut moves = HashMap::from([(1, (5.0, 6.0)), (2, (9.0, 10.0)), (3, (1.0, 2.0))]);
        let current = HashMap::from([(1, (5.0, 6.0)), (2, (8.0, 10.0))]);
        assert_eq!(discard_unchanged(&mut moves, |k| current.get(k).copied()), 1);
        assert_eq!(moves, HashMap::from([(2, (9.0, 10.0)), (3, (1.0, 2.0))]));
    }

    #[test]
    fn all_duplicates_leave_no_dispatch_work() {
        let mut moves = HashMap::from([(1, (5.0, 6.0)), (2, (5.0, 6.0))]);
        assert_eq!(discard_unchanged(&mut moves, |_| Some((5.0, 6.0))), 2);
        assert!(moves.is_empty());
    }

    #[test]
    fn changed_path_samples_are_not_coalesced() {
        let mut delivered = Vec::new();
        let mut location = (0.0, 0.0);
        for next in [(1.0, 0.0), (2.0, 3.0), (1.0, 0.0), (0.0, 0.0)] {
            let mut moves = HashMap::from([(1, next)]);
            assert_eq!(discard_unchanged(&mut moves, |_| Some(location)), 0);
            delivered.push(moves[&1]);
            location = next;
        }
        assert_eq!(delivered, vec![(1.0, 0.0), (2.0, 3.0), (1.0, 0.0), (0.0, 0.0)]);
    }

    #[test]
    fn float_edges_preserve_existing_coordinate_equality() {
        let mut zero = HashMap::from([(1, (-0.0, 0.0))]);
        assert_eq!(discard_unchanged(&mut zero, |_| Some((0.0, -0.0))), 1);
        let mut nan = HashMap::from([(1, (f32::NAN, 0.0))]);
        assert_eq!(discard_unchanged(&mut nan, |_| Some((f32::NAN, 0.0))), 0);
        assert!(nan[&1].0.is_nan());
    }
}
