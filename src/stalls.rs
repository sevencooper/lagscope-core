//! Gaps in a steady packet stream (LagScope's `stream_gaps`, `docs/RULES.md`),
//! plus the check that the sender itself was not late, which a website can do because every
//! packet carries when it was meant to leave and when it did.

use std::collections::HashMap;

/// Longer than this is a pause (a tab asleep, a dropped connection), not a stall.
const PAUSE_MS: f64 = 5_000.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Gap {
    /// Arrival time of the last packet before the gap (receiver's clock, ms).
    pub start: f64,
    /// Length of the gap (ms).
    pub gap: f64,
    /// Packets arrived in a bunch after the gap: they were held, then released.
    pub held: bool,
    /// The sender stopped too (its own send times have the same hole): not the network.
    pub sender_late: bool,
}

/// One packet as the receiver saw it: sequence number and arrival time (ms).
#[derive(Debug, Clone, Copy)]
pub struct Arrival {
    pub seq: u32,
    pub at: f64,
}

/// Gaps longer than max(`factor` x the local median spacing, `min_ms`) in `arrivals`
/// (sorted by arrival time). `sent` maps a sequence number to its (planned, actual) send
/// times on the sender's clock (ms).
pub fn find(arrivals: &[Arrival], sent: &HashMap<u32, (f64, f64)>, min_ms: f64, factor: f64) -> Vec<Gap> {
    let times: Vec<f64> = arrivals.iter().map(|a| a.at).collect();
    let gaps: Vec<f64> = times.windows(2).map(|w| w[1] - w[0]).collect();
    let median = median_of(gaps.iter().copied().filter(|&g| g < 1_000.0).collect()).max(0.001);
    let mut out = Vec::new();
    for (i, &g) in gaps.iter().enumerate() {
        if g <= min_ms || g > PAUSE_MS {
            continue;
        }
        // Only a live stream can stall: 30+ packets in the 3 s before and after the gap.
        let since = times[i] - 3_000.0;
        let until = times[i + 1] + 3_000.0;
        if i + 1 - times[..=i].partition_point(|&x| x < since) < 30
            || times[i + 1..].partition_point(|&x| x < until) < 30
        {
            continue;
        }
        let recent: Vec<f64> = gaps[i.saturating_sub(64)..i].iter().copied().filter(|&x| x < 1_000.0).collect();
        let local = if recent.len() >= 16 { median_of(recent) } else { median };
        if g <= (factor * local).max(min_ms) {
            continue;
        }
        // Packets arriving within 20 ms after the gap: a large burst means they were held
        // and released together; few means they were lost (or never sent).
        let end = times[i + 1];
        let after: Vec<&Arrival> = arrivals[i + 1..].iter().take_while(|a| a.at - end <= 20.0).collect();
        let burst = after.len();
        let held = burst >= 3 && burst as f64 >= 0.3 * (g / local);
        // Packets can arrive out of order after a freeze: the first one the sender sent after
        // the gap is the lowest sequence number in the burst. If the sender's own clock shows
        // the same hole before it (or it left late), the sender paused, not the network.
        let first = after.iter().map(|a| a.seq).min().unwrap_or(arrivals[i + 1].seq);
        let sender_late = match (sent.get(&first), first.checked_sub(1).and_then(|p| sent.get(&p))) {
            (Some(&(planned, actual)), Some(&(_, prev_actual))) => {
                actual - prev_actual >= 0.5 * g || actual - planned >= 0.5 * g
            }
            _ => false,
        };
        out.push(Gap { start: times[i], gap: g, held, sender_late });
    }
    out
}

pub fn median_of(mut v: Vec<f64>) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

/// Value at quantile `p` (0..1) of a sorted slice, rounding to the nearest rank (LagScope's
/// `percentile`).
pub fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let i = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[i.min(sorted.len() - 1)]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 64 packets a second for `secs`, sent on time, arriving after 20 ms; `freeze` holds
    /// everything sent in [at, at + ms) until at + ms.
    fn stream(secs: f64, freeze: &[(f64, f64)], lose: bool) -> (Vec<Arrival>, HashMap<u32, (f64, f64)>) {
        let step = 1000.0 / 64.0;
        let n = (secs * 64.0) as u32;
        let mut arr = Vec::new();
        let mut sent = HashMap::new();
        for seq in 0..n {
            let t = seq as f64 * step;
            sent.insert(seq, (t, t));
            let mut at = t + 20.0;
            if let Some(&(f, ms)) = freeze.iter().find(|&&(f, ms)| t >= f && t < f + ms) {
                if lose {
                    continue;
                }
                at = f + ms + 20.0 + (seq % 3) as f64 * 0.1;
            }
            arr.push(Arrival { seq, at });
        }
        arr.sort_by(|a, b| a.at.total_cmp(&b.at));
        (arr, sent)
    }

    #[test]
    fn steady_stream_has_no_gaps() {
        let (a, s) = stream(30.0, &[], false);
        assert!(find(&a, &s, 80.0, 4.0).is_empty());
    }

    #[test]
    fn held_freeze_is_found() {
        let (a, s) = stream(30.0, &[(10_000.0, 150.0)], false);
        let g = find(&a, &s, 80.0, 4.0);
        assert_eq!(g.len(), 1);
        assert!(g[0].held && !g[0].sender_late);
        assert!((g[0].gap - 150.0).abs() < 20.0, "{}", g[0].gap);
    }

    #[test]
    fn lost_packets_are_not_held() {
        let (a, s) = stream(30.0, &[(10_000.0, 150.0)], true);
        let g = find(&a, &s, 80.0, 4.0);
        assert_eq!(g.len(), 1);
        assert!(!g[0].held);
    }

    #[test]
    fn late_sender_is_not_the_network() {
        let (mut a, mut s) = stream(30.0, &[], false);
        // The sender paused 200 ms at 10 s: packets meant for 10.0-10.2 s left at 10.2 s.
        let step = 1000.0 / 64.0;
        for x in a.iter_mut() {
            let planned = x.seq as f64 * step;
            if (10_000.0..10_200.0).contains(&planned) {
                let actual = 10_200.0 + (x.seq % 3) as f64 * 0.1;
                s.insert(x.seq, (planned, actual));
                x.at = actual + 20.0;
            }
        }
        a.sort_by(|p, q| p.at.total_cmp(&q.at));
        let g = find(&a, &s, 80.0, 4.0);
        assert_eq!(g.len(), 1);
        assert!(g[0].sender_late);
    }
}
