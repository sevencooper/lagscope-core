//! Timers: freezes that come back on a schedule.
//!
//! `beat` and `second_timer` come from LagScope 0.6.0 (`src/analyze/detect.rs`), where they
//! were tuned on real recordings. Times are milliseconds (LagScope used microseconds); the
//! rules and numbers are unchanged (`docs/RULES.md`).

/// A beat that may skip: a timer fires every p seconds but only some firings show up, so
/// events come p, 2p, 3p ... apart. Judged on the gaps between consecutive events (events
/// within 2 s merged: one freeze can cut a stream twice), against the chance that a random
/// gap lands near a whole multiple. Returns (period s, share of gaps on the beat, events).
pub fn beat(times_ms: &[f64]) -> Option<(f64, f64, usize)> {
    let ev = merged_s(times_ms);
    if ev.len() < 8 {
        return None;
    }
    let gaps: Vec<f64> = ev.windows(2).map(|w| w[1] - w[0]).collect();
    // Whole multiples 1..=6 of p count; longer gaps count as off the beat.
    let fit = |p: f64| {
        let score = gaps.iter().filter(|&&g| on_beat(p, g)).count() as f64 / gaps.len() as f64;
        (score, score - (2.0 * tol_of(p) / p).min(1.0))
    };
    let mut best: Option<(f64, f64, f64)> = None;
    let mut p = 2.0f64;
    while p <= 60.0 {
        let (score, margin) = fit(p);
        if best.is_none_or(|b| margin > b.2) {
            best = Some((p, score, margin));
        }
        p += 0.02;
    }
    let (mut p, mut score, margin) = best?;
    if score < 0.65 || margin < 0.45 {
        return None;
    }
    // A 10 s beat also fits 5 s and 3.33 s (every gap is a multiple of those too): take the
    // longest whole multiple that fits about as well.
    let base = p;
    for k in 2..=6 {
        let q = base * k as f64;
        let (s_q, _) = fit(q);
        if q <= 60.0 && s_q >= score * 0.9 {
            p = q;
            score = s_q;
        }
    }
    // Refine on the gaps themselves: total time over total periods.
    let (sum, k): (f64, f64) =
        gaps.iter().filter(|&&g| on_beat(p, g)).fold((0.0, 0.0), |(s, k), &g| (s + g, k + (g / p).round()));
    let period = if k > 0.0 { sum / k } else { p };
    Some(((period * 100.0).round() / 100.0, score, ev.len()))
}

/// Events that start at the same point within a second (+-75 ms), e.g. always at x.88 s: a
/// 1-second timer somewhere (router, adapter firmware, driver) sets them off. Returns (how
/// many share that point, total) when at least 60% do; by chance it would be about 15%.
pub fn second_timer(times_ms: &[f64]) -> Option<(usize, usize)> {
    if times_ms.len() < 8 {
        return None;
    }
    let mut bins = [0usize; 20];
    for &t in times_ms {
        bins[(t.rem_euclid(1000.0) / 50.0) as usize % 20] += 1;
    }
    let best = (0..20).map(|i| bins[(i + 19) % 20] + bins[i] + bins[(i + 1) % 20]).max().unwrap_or(0);
    (best * 10 >= times_ms.len() * 6).then_some((best, times_ms.len()))
}

/// Event times in seconds, sorted, with events within 2 s of the previous one merged.
pub fn merged_s(times_ms: &[f64]) -> Vec<f64> {
    let mut sorted_t = times_ms.to_vec();
    sorted_t.sort_by(f64::total_cmp);
    let mut ev: Vec<f64> = Vec::new();
    for t in sorted_t {
        let s = t / 1000.0;
        if ev.last().is_none_or(|&l| s - l > 2.0) {
            ev.push(s);
        }
    }
    ev
}

fn tol_of(p: f64) -> f64 {
    (0.04 * p).max(0.4)
}

/// Whether a gap between two events (s) is a whole multiple (1..=6) of the period p (s).
pub fn on_beat(p: f64, g: f64) -> bool {
    let k = (g / p).round();
    (1.0..=6.0).contains(&k) && (g - k * p).abs() <= tol_of(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_a_ten_second_beat() {
        let t: Vec<f64> = (0..18).map(|i| 6_400.0 + i as f64 * 10_000.0).collect();
        let (p, score, n) = beat(&t).unwrap();
        assert_eq!(p, 10.0);
        assert_eq!(score, 1.0);
        assert_eq!(n, 18);
    }

    #[test]
    fn tolerates_skipped_beats() {
        // Every 10 s, but some firings missing.
        let t: Vec<f64> =
            [0, 1, 2, 4, 5, 8, 9, 10, 13, 14, 15].iter().map(|&k| 3_000.0 + k as f64 * 10_000.0).collect();
        assert_eq!(beat(&t).unwrap().0, 10.0);
    }

    #[test]
    fn random_events_have_no_beat() {
        // Fixed pseudo-random times over 3 minutes.
        let mut s: u64 = 7;
        let mut t: Vec<f64> = (0..12)
            .map(|_| {
                s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                (s >> 11) as f64 / (1u64 << 53) as f64 * 180_000.0
            })
            .collect();
        t.sort_by(f64::total_cmp);
        assert!(beat(&t).is_none());
    }

    #[test]
    fn needs_eight_events() {
        let t: Vec<f64> = (0..7).map(|i| i as f64 * 10_000.0).collect();
        assert!(beat(&t).is_none());
    }

    #[test]
    fn second_timer_locks_to_a_point_in_the_second() {
        let t: Vec<f64> = (0..10).map(|i| i as f64 * 3_000.0 + 880.0).collect();
        assert_eq!(second_timer(&t), Some((10, 10)));
        let spread: Vec<f64> = (0..10).map(|i| i as f64 * 3_100.0 + i as f64 * 97.0).collect();
        assert!(second_timer(&spread).is_none());
    }
}
