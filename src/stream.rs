//! The two-way stream test (LagScope Pulse's lag test; later also the app's test against
//! Pulse's server): turns one recording into stalls per direction, a timer if there is one,
//! loss, round trip, the under-load phases, a scorecard and a verdict. Returns facts and
//! statuses; the wording is in `design/wording.md`, each product renders it.
//!
//! The rules and their numbers: `docs/RULES.md`. Thresholds first used by Pulse are named
//! constants below.

use crate::beat;
use crate::stalls::{self, Arrival, Gap, median_of, percentile};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Stalls of this length count (LagScope's default game minimum).
const STALL_MS: f64 = 80.0;
/// Stalls a player feels.
const FELT_MS: f64 = 100.0;
/// Short pauses, used only for finding timers.
const SHORT_MS: f64 = 50.0;
/// A direction is amber with 3+ felt stalls and at least 0.3 a minute (LagScope's scorecard).
const FELT_MIN: usize = 3;
const FELT_PER_MIN: f64 = 0.3;
/// Loss above this share (%) is amber (LagScope: loss beyond the router > 1%).
const LOSS_PCT: f64 = 1.0;
/// Round trip: amber when the worst 1% outside freezes is this much above typical (ms).
const RTT_SPREAD_MS: f64 = 50.0;
/// Device: amber with this many pauses of its own, or this many slow frames a minute.
const DEVICE_PAUSES: usize = 3;
const SLOW_FRAMES_PER_MIN: f64 = 1.0;
/// Under load: amber when the round trip (worst 10%) grows this much over idle (ms). Gaming
/// starts to suffer around here; the Waveform test grades +30-60 ms as a B, +60-200 ms as a C.
const LOAD_MS: f64 = 50.0;
/// After a load phase ends, queues take a moment to drain: this long still counts as loaded.
const LOAD_TAIL_MS: f64 = 1_000.0;
/// Less judged time than this, or fewer packets, is not enough to say anything.
const MIN_JUDGED_MS: f64 = 20_000.0;
const MIN_PACKETS: usize = 200;

/// A recorded test, in the format the page records (the prototype's raw-data file).
#[derive(Debug, Deserialize)]
pub struct Recording {
    pub meta: Meta,
    /// Browser packets: [seq, planned ms, actual ms] (browser clock).
    pub sent: Vec<(u32, f64, f64)>,
    /// Server packets: [seq, planned us, actual us (server clock), arrival ms (browser clock)].
    pub down: Vec<(u32, f64, f64, f64)>,
    /// Echoes: [seq, round trip ms, server arrival us].
    pub echo: Vec<(u32, f64, f64)>,
    /// Browser packets as the server saw them: [seq, arrival us (server clock)].
    pub up: Vec<(u32, f64)>,
    /// Page visibility changes: [time ms, "hidden" | "visible"].
    #[serde(default)]
    pub vis: Vec<(f64, String)>,
    /// Animation frames 50 ms+ apart: [time ms, gap ms].
    #[serde(default)]
    pub frames: Vec<(f64, f64)>,
    /// When the line was filled on purpose ("down" or "up", from ms, to ms; browser clock).
    #[serde(default)]
    pub load: Vec<(String, f64, f64)>,
}

#[derive(Debug, Deserialize)]
pub struct Meta {
    /// When the test started (browser clock, ms). All output times are relative to this.
    pub t0_ms: f64,
    /// Packets per second, each way.
    pub rate: f64,
}

#[derive(Debug, Serialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Dir {
    Down,
    Up,
    Both,
}

/// Why a stream paused.
#[derive(Debug, Serialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Cause {
    /// The network held or lost the packets.
    Network,
    /// This device or browser was busy (its own timers or frames ran late).
    Device,
    /// Our server sent late.
    Server,
    /// The tab was hidden: marked, not judged.
    Hidden,
    /// During an under-load phase: judged there, not in the idle verdict.
    Load,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Stall {
    pub dir: Dir,
    /// Start, ms since the test started (browser clock; uploads mapped onto it).
    pub at: f64,
    pub ms: f64,
    /// Held then released together (true) or lost (false).
    pub held: bool,
    pub cause: Cause,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DirStats {
    /// Network stalls of 80 ms+.
    pub stalls: usize,
    /// Of those, 100 ms+.
    pub felt: usize,
    pub held: usize,
    pub typical_ms: f64,
    pub max_ms: f64,
    /// Pauses that were not the network.
    pub device: usize,
    pub server: usize,
    pub hidden: usize,
    /// Packets lost, of those that should have arrived (the last second is left out).
    pub lost: usize,
    pub expected: usize,
    pub loss_pct: f64,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RttStats {
    pub count: usize,
    pub median_ms: f64,
    pub p99_ms: f64,
    pub max_ms: f64,
    /// Worst 1% leaving out the freezes (the freezes are judged separately).
    pub p99_calm_ms: f64,
    /// Spikes above max(3 x median, median + 30 ms), merged within 300 ms.
    pub spikes: usize,
    /// Average change between consecutive round trips (Cloudflare's definition, so the two
    /// sites agree; docs/CLOUDFLARE_IDEAS.md).
    pub jitter_ms: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Timer {
    /// Which pauses keep the beat.
    pub dir: Dir,
    pub period_s: f64,
    /// Share of the gaps between pauses that are on the beat (0..1).
    pub on_beat: f64,
    /// Pauses after merging those within 2 s.
    pub events: usize,
    /// Typical length of a pause in this series (ms).
    pub typical_ms: f64,
    /// A pause on the beat (ms since start), to draw the beat grid from.
    pub anchor_ms: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecondTimer {
    /// Pauses starting at the same point within a second, of `total`.
    pub locked: usize,
    pub total: usize,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DeviceStats {
    /// Pauses this device or browser caused, both directions.
    pub pauses: usize,
    /// Animation frames 50 ms+ apart while the tab was in front.
    pub slow_frames: usize,
    pub hidden_times: usize,
    pub hidden_ms: f64,
}

#[derive(Debug, Serialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Ok,
    Warn,
    Na,
}

#[derive(Debug, Serialize)]
pub struct Line {
    /// timer, download, upload, loss, rtt, device
    pub area: &'static str,
    pub status: Status,
}

/// The line's behaviour in one phase of the test.
#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PhaseStats {
    pub ms: f64,
    pub rtt_median_ms: f64,
    /// Worst 10% (the queue builds over seconds, so the tail matters more than the middle).
    pub rtt_p90_ms: f64,
    /// Pauses of 80 ms+ in either direction, counted once when both stop together.
    pub freezes: usize,
    /// Packets lost in this phase, both directions together (%).
    pub loss_pct: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadStats {
    pub idle: PhaseStats,
    pub down: Option<PhaseStats>,
    pub up: Option<PhaseStats>,
    /// Extra round trip under load: worst 10% loaded minus typical idle (ms).
    pub down_extra_ms: Option<f64>,
    pub up_extra_ms: Option<f64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Analysis {
    /// Length of the test (ms) and the part that was judged (tab in front).
    pub duration_ms: f64,
    pub judged_ms: f64,
    /// One of: steady, timer, freezes, upload, download, loss, load, ping, device, not_enough.
    pub verdict: &'static str,
    pub score: Vec<Line>,
    pub down: DirStats,
    pub up: DirStats,
    /// Felt download stalls with an upload stall at the same moment.
    pub together: usize,
    pub rtt: RttStats,
    pub timer: Option<Timer>,
    pub second_timer: Option<SecondTimer>,
    pub device: DeviceStats,
    /// Every pause of 80 ms+, both directions, in time order.
    pub stalls: Vec<Stall>,
    /// Hidden-tab intervals [from, to] (ms since start).
    pub hidden: Vec<(f64, f64)>,
    /// Round trip over time, one value per bucket: (ms since start, worst round trip ms).
    pub rtt_series: Vec<(f64, f64)>,
    /// Only when the test filled the line on purpose.
    pub load: Option<LoadStats>,
    /// The load phases ("down" / "up", from, to; ms since start), for the chart.
    pub load_phases: Vec<(String, f64, f64)>,
}

pub fn analyze(r: &Recording) -> Analysis {
    let t0 = r.meta.t0_ms;
    let rate = r.meta.rate.max(1.0);

    // Browser send times by sequence number, relative to the start.
    let sent: HashMap<u32, (f64, f64)> = r.sent.iter().map(|&(s, p, a)| (s, (p - t0, a - t0))).collect();
    let end = r.sent.iter().map(|s| s.2 - t0).chain(r.down.iter().map(|d| d.3 - t0)).fold(0.0, f64::max);

    // Map the server's clock onto the browser's: the server got a packet about half a round
    // trip after the browser sent it.
    let offsets: Vec<f64> =
        r.echo.iter().filter_map(|&(s, rtt, srv)| sent.get(&s).map(|&(_, a)| srv / 1000.0 - (a + rtt / 2.0))).collect();
    let offset = if offsets.is_empty() {
        r.up.iter().filter_map(|&(s, srv)| sent.get(&s).map(|&(_, a)| srv / 1000.0 - a)).fold(f64::INFINITY, f64::min)
    } else {
        median_of(offsets)
    };
    let offset = if offset.is_finite() { offset } else { 0.0 };

    let hidden = hidden_intervals(&r.vis, t0, end);
    let phases: Vec<(String, f64, f64)> = r.load.iter().map(|(k, a, b)| (k.clone(), a - t0, b - t0)).collect();
    let phase_at = |t: f64| phases.iter().find(|p| t >= p.1 && t < p.2 + LOAD_TAIL_MS).map(|p| p.0.as_str());
    let in_load = |a: f64, b: f64| phases.iter().any(|p| a < p.2 + LOAD_TAIL_MS && b > p.1);
    let in_hidden = |a: f64, b: f64| hidden.iter().any(|&(h0, h1)| a < h1 + 1_000.0 && b > h0);

    // Download: server packets by arrival in the browser.
    let mut down: Vec<Arrival> = r.down.iter().map(|d| Arrival { seq: d.0, at: d.3 - t0 }).collect();
    down.sort_by(|a, b| a.at.total_cmp(&b.at));
    let srv_sent: HashMap<u32, (f64, f64)> = r.down.iter().map(|d| (d.0, (d.1 / 1000.0, d.2 / 1000.0))).collect();
    // Upload: browser packets by arrival at the server, on the browser's timeline.
    let mut up: Vec<Arrival> =
        r.up.iter()
            .filter(|u| sent.contains_key(&u.0))
            .map(|u| Arrival { seq: u.0, at: u.1 / 1000.0 - offset })
            .collect();
    up.sort_by(|a, b| a.at.total_cmp(&b.at));

    // The page's own lateness: its send loop and its animation frames. A download gap while
    // the page was busy is this device, not the network.
    let late_sends: Vec<(f64, f64)> = r.sent.iter().map(|&(_, p, a)| (p - t0, a - p)).collect();
    let frames: Vec<(f64, f64)> = r.frames.iter().map(|&(t, g)| (t - t0 - g, g)).collect();
    let device_busy = |a: f64, b: f64| {
        let sends = late_sends.iter().filter(|&&(t, _)| t >= a - 5.0 && t <= b).map(|s| s.1).fold(0.0, f64::max);
        let fr = frames.iter().filter(|&&(t, g)| t < b && t + g > a).map(|f| f.1).fold(0.0, f64::max);
        sends.max(fr)
    };
    let classify = |g: &Gap, dir: Dir| -> Cause {
        if in_hidden(g.start, g.start + g.gap) {
            Cause::Hidden
        } else if in_load(g.start, g.start + g.gap) {
            Cause::Load
        } else if dir == Dir::Down && device_busy(g.start, g.start + g.gap) >= 0.5 * g.gap {
            Cause::Device
        } else if g.sender_late {
            if dir == Dir::Down { Cause::Server } else { Cause::Device }
        } else {
            Cause::Network
        }
    };
    let to_stalls = |gaps: Vec<Gap>, dir: Dir| -> Vec<Stall> {
        gaps.iter().map(|g| Stall { dir, at: g.start, ms: g.gap, held: g.held, cause: classify(g, dir) }).collect()
    };

    let down_stalls = to_stalls(stalls::find(&down, &srv_sent, STALL_MS, 4.0), Dir::Down);
    let up_stalls = to_stalls(stalls::find(&up, &sent, STALL_MS, 4.0), Dir::Up);
    let net_short = |arr: &[Arrival], snd: &HashMap<u32, (f64, f64)>, dir: Dir| -> Vec<Stall> {
        to_stalls(stalls::find(arr, snd, SHORT_MS, 2.0), dir)
            .into_iter()
            .filter(|s| s.cause == Cause::Network)
            .collect()
    };
    let down_short = net_short(&down, &srv_sent, Dir::Down);
    let up_short = net_short(&up, &sent, Dir::Up);

    // Loss, leaving out the last second (those packets may still have been on the way) and the
    // load phases (judged on their own). When a server packet was meant to leave, on our clock:
    // its planned time, or for a lost one, its place in the steady schedule.
    let base = r.down.first().map(|d| d.1 / 1000.0 - offset - d.0 as f64 * 1000.0 / rate).unwrap_or(0.0);
    let down_time = |seq: u32| base + seq as f64 * 1000.0 / rate;
    let down_cutoff = r.down.iter().map(|d| d.0).max().map(|m| m.saturating_sub(rate as u32));
    let last_sent = r.sent.iter().map(|s| s.2 - t0).fold(0.0, f64::max);
    let up_cutoff = r.sent.iter().filter(|s| s.2 - t0 <= last_sent - 1_000.0).map(|s| s.0).max();
    let up_time = |seq: u32| sent.get(&seq).map_or(f64::NAN, |s| s.1);
    let idle = |t: f64| phase_at(t).is_none();
    let down_loss = loss(r.down.iter().map(|d| d.0), down_cutoff, |q| idle(down_time(q)));
    let up_loss = loss(r.up.iter().map(|u| u.0), up_cutoff, |q| idle(up_time(q)));

    let down_stats = dir_stats(&down_stalls, down_loss);
    let up_stats = dir_stats(&up_stalls, up_loss);

    // Felt download freezes with an upload freeze at the same moment: the whole line stops.
    let felt = |s: &&Stall| s.cause == Cause::Network && s.ms >= FELT_MS;
    let together = down_stalls
        .iter()
        .filter(felt)
        .filter(|d| up_stalls.iter().filter(felt).any(|u| u.at < d.at + d.ms + 150.0 && u.at + u.ms > d.at - 150.0))
        .count();

    // Round trip, by the time the packet was sent.
    let mut all_echoes: Vec<(f64, f64)> =
        r.echo.iter().filter_map(|&(s, rtt, _)| sent.get(&s).map(|&(_, a)| (a, rtt))).collect();
    all_echoes.sort_by(|a, b| a.0.total_cmp(&b.0));
    let echoes: Vec<(f64, f64)> = all_echoes.iter().copied().filter(|e| idle(e.0)).collect();
    let net_windows: Vec<(f64, f64)> = down_stalls
        .iter()
        .chain(up_stalls.iter())
        .filter(|s| s.cause == Cause::Network)
        .map(|s| (s.at - 300.0, s.at + s.ms))
        .collect();
    let rtt = rtt_stats(&echoes, &net_windows);
    let rtt_series = series(&all_echoes, end);

    // Timers: on short network pauses, per direction and both together.
    let times = |v: &[Stall]| v.iter().map(|s| s.at).collect::<Vec<f64>>();
    let both_short: Vec<Stall> = down_short.iter().chain(up_short.iter()).cloned().collect();
    let b_down = beat::beat(&times(&down_short));
    let b_up = beat::beat(&times(&up_short));
    let b_both = beat::beat(&times(&both_short));
    let same = |a: f64, b: f64| {
        let (hi, lo) = if a > b { (a, b) } else { (b, a) };
        let k = (hi / lo).round();
        (1.0..=6.0).contains(&k) && (hi - k * lo).abs() <= 0.03 * hi
    };
    let pick = match (b_down, b_up, b_both) {
        (Some(d), Some(u), both) if same(d.0, u.0) => Some((Dir::Both, both.unwrap_or(d), &both_short)),
        (Some(d), _, _) => Some((Dir::Down, d, &down_short)),
        (None, Some(u), _) => Some((Dir::Up, u, &up_short)),
        (None, None, Some(b)) => Some((Dir::Both, b, &both_short)),
        _ => None,
    };
    let timer = pick.map(|(dir, (period_s, on_beat, events), series)| Timer {
        dir,
        period_s,
        on_beat,
        events,
        typical_ms: median_of(series.iter().filter(|s| s.ms >= STALL_MS).map(|s| s.ms).collect())
            .max(median_of(series.iter().map(|s| s.ms).collect())),
        anchor_ms: anchor(&times(series), period_s),
    });
    let second_timer = if timer.is_none() {
        // A freeze often cuts both streams: count it once, as the beat search does.
        let merged: Vec<f64> = beat::merged_s(&times(&both_short)).iter().map(|s| s * 1000.0).collect();
        beat::second_timer(&merged).map(|(locked, total)| SecondTimer { locked, total })
    } else {
        None
    };

    let hidden_ms = hidden.iter().map(|h| h.1 - h.0).fold(0.0, |a, b| a + b);
    let load_ms = phases.iter().map(|p| (p.2 + LOAD_TAIL_MS).min(end) - p.1).fold(0.0, |a, b| a + b);
    let judged_ms = (end - hidden_ms - load_ms).max(0.0);
    let device = DeviceStats {
        // A busy page usually cuts both streams at once: one pause, not two.
        pauses: distinct(down_stalls.iter().chain(up_stalls.iter()).filter(|s| s.cause == Cause::Device)),
        // Frames while the line is filled on purpose are left out: that work is ours.
        slow_frames: frames.iter().filter(|&&(t, g)| !in_hidden(t, t + g) && !in_load(t, t + g)).count(),
        hidden_times: hidden.len(),
        hidden_ms,
    };

    // Under load: each phase against the idle time.
    let all_stalls: Vec<&Stall> = down_stalls.iter().chain(up_stalls.iter()).collect();
    let phase_stats = |inside: &dyn Fn(f64) -> bool, ms: f64| -> PhaseStats {
        let mut r: Vec<f64> = all_echoes.iter().filter(|e| inside(e.0)).map(|e| e.1).collect();
        r.sort_by(f64::total_cmp);
        let d = loss(down.iter().map(|a| a.seq), down_cutoff, |q| inside(down_time(q)));
        let u = loss(up.iter().map(|a| a.seq), up_cutoff, |q| inside(up_time(q)));
        let expected = d.1 + u.1;
        PhaseStats {
            ms,
            rtt_median_ms: percentile(&r, 0.5),
            rtt_p90_ms: percentile(&r, 0.9),
            freezes: distinct(
                all_stalls.iter().copied().filter(|s| matches!(s.cause, Cause::Network | Cause::Load) && inside(s.at)),
            ),
            loss_pct: if expected > 0 { 100.0 * (d.0 + u.0) as f64 / expected as f64 } else { 0.0 },
        }
    };
    let load = (!phases.is_empty()).then(|| {
        let idle_stats = phase_stats(&|t| idle(t) && !in_hidden(t, t), judged_ms);
        let one = |kind: &str| {
            let ms: f64 = phases.iter().filter(|p| p.0 == kind).map(|p| p.2 - p.1).sum();
            (ms > 0.0).then(|| phase_stats(&|t| phase_at(t) == Some(kind), ms))
        };
        let (d, u) = (one("down"), one("up"));
        let extra = |p: &Option<PhaseStats>| p.as_ref().map(|p| (p.rtt_p90_ms - idle_stats.rtt_median_ms).max(0.0));
        LoadStats { down_extra_ms: extra(&d), up_extra_ms: extra(&u), idle: idle_stats, down: d, up: u }
    });

    // Scorecard and verdict.
    let minutes = (judged_ms / 60_000.0).max(1.0 / 60.0);
    let felt_warn = |d: &DirStats| d.felt >= FELT_MIN && d.felt as f64 / minutes >= FELT_PER_MIN;
    let enough = judged_ms >= MIN_JUDGED_MS && down.len() >= MIN_PACKETS && echoes.len() >= MIN_PACKETS;
    let st = |warn: bool| {
        if !enough {
            Status::Na
        } else if warn {
            Status::Warn
        } else {
            Status::Ok
        }
    };
    let timer_warn = timer.is_some() || second_timer.is_some();
    let down_warn = felt_warn(&down_stats);
    let up_warn = felt_warn(&up_stats);
    let loss_warn = down_loss.2 > LOSS_PCT || up_loss.2 > LOSS_PCT;
    let rtt_warn = rtt.count > 0 && rtt.p99_calm_ms - rtt.median_ms > RTT_SPREAD_MS;
    let device_warn = device.pauses >= DEVICE_PAUSES || device.slow_frames as f64 / minutes >= SLOW_FRAMES_PER_MIN;
    let load_warn =
        load.as_ref().is_some_and(|l| l.down_extra_ms.unwrap_or(0.0).max(l.up_extra_ms.unwrap_or(0.0)) > LOAD_MS);
    let mut score = vec![
        Line { area: "timer", status: st(timer_warn) },
        Line { area: "download", status: st(down_warn) },
        Line { area: "upload", status: st(up_warn) },
        Line { area: "loss", status: st(loss_warn) },
        Line { area: "rtt", status: st(rtt_warn) },
        Line { area: "device", status: st(device_warn) },
    ];
    if load.is_some() {
        score.insert(5, Line { area: "load", status: st(load_warn) });
    }
    let verdict = if !enough {
        "not_enough"
    } else if timer_warn {
        "timer"
    } else if down_warn && up_warn {
        "freezes"
    } else if up_warn {
        "upload"
    } else if down_warn {
        "download"
    } else if loss_warn {
        "loss"
    } else if load_warn {
        "load"
    } else if rtt_warn {
        "ping"
    } else if device_warn {
        "device"
    } else {
        "steady"
    };

    let mut all: Vec<Stall> = down_stalls.into_iter().chain(up_stalls).collect();
    all.sort_by(|a, b| a.at.total_cmp(&b.at));

    Analysis {
        duration_ms: end,
        judged_ms,
        verdict,
        score,
        down: down_stats,
        up: up_stats,
        together,
        rtt,
        timer,
        second_timer,
        device,
        stalls: all,
        hidden,
        rtt_series,
        load,
        load_phases: phases,
    }
}

/// Pauses counted once when they overlap (within 300 ms), whichever directions they hit.
fn distinct<'a>(stalls: impl Iterator<Item = &'a Stall>) -> usize {
    let mut v: Vec<(f64, f64)> = stalls.map(|s| (s.at, s.at + s.ms)).collect();
    v.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut n = 0;
    let mut end = f64::NEG_INFINITY;
    for (a, b) in v {
        if a > end + 300.0 {
            n += 1;
        }
        end = end.max(b);
    }
    n
}

/// (lost, expected, loss %) for sequence numbers 0..=cutoff that `counts`.
fn loss(got: impl Iterator<Item = u32>, cutoff: Option<u32>, counts: impl Fn(u32) -> bool) -> (usize, usize, f64) {
    let Some(cutoff) = cutoff else { return (0, 0, 0.0) };
    let seen: HashSet<u32> = got.filter(|&s| s <= cutoff).collect();
    let (mut expected, mut lost) = (0, 0);
    for q in 0..=cutoff {
        if counts(q) {
            expected += 1;
            if !seen.contains(&q) {
                lost += 1;
            }
        }
    }
    (lost, expected, if expected > 0 { 100.0 * lost as f64 / expected as f64 } else { 0.0 })
}

fn dir_stats(stalls: &[Stall], loss: (usize, usize, f64)) -> DirStats {
    let net: Vec<&Stall> = stalls.iter().filter(|s| s.cause == Cause::Network).collect();
    let count = |c: Cause| stalls.iter().filter(|s| s.cause == c).count();
    DirStats {
        stalls: net.len(),
        felt: net.iter().filter(|s| s.ms >= FELT_MS).count(),
        held: net.iter().filter(|s| s.held).count(),
        typical_ms: median_of(net.iter().map(|s| s.ms).collect()),
        max_ms: net.iter().map(|s| s.ms).fold(0.0, f64::max),
        device: count(Cause::Device),
        server: count(Cause::Server),
        hidden: count(Cause::Hidden),
        lost: loss.0,
        expected: loss.1,
        loss_pct: loss.2,
    }
}

fn rtt_stats(echoes: &[(f64, f64)], freezes: &[(f64, f64)]) -> RttStats {
    if echoes.is_empty() {
        return RttStats::default();
    }
    let mut all: Vec<f64> = echoes.iter().map(|e| e.1).collect();
    all.sort_by(f64::total_cmp);
    let mut calm: Vec<f64> =
        echoes.iter().filter(|e| !freezes.iter().any(|&(a, b)| e.0 >= a && e.0 <= b)).map(|e| e.1).collect();
    calm.sort_by(f64::total_cmp);
    let median = percentile(&all, 0.5);
    let limit = (3.0 * median).max(median + 30.0);
    let mut spikes = 0;
    let mut last = f64::NEG_INFINITY;
    for &(t, v) in echoes {
        if v > limit {
            if t - last > 300.0 {
                spikes += 1;
            }
            last = t;
        }
    }
    let jitter_ms = if echoes.len() > 1 {
        echoes.windows(2).map(|w| (w[1].1 - w[0].1).abs()).sum::<f64>() / (echoes.len() - 1) as f64
    } else {
        0.0
    };
    RttStats {
        count: all.len(),
        median_ms: median,
        p99_ms: percentile(&all, 0.99),
        max_ms: *all.last().unwrap_or(&0.0),
        p99_calm_ms: percentile(if calm.is_empty() { &all } else { &calm }, 0.99),
        spikes,
        jitter_ms,
    }
}

/// Worst round trip per bucket, about 720 buckets over the test (enough for a wide chart).
fn series(echoes: &[(f64, f64)], end: f64) -> Vec<(f64, f64)> {
    let bucket = (end / 720.0).max(100.0);
    let mut out: Vec<(f64, f64)> = Vec::new();
    for &(t, v) in echoes {
        let b = (t / bucket).floor() * bucket;
        match out.last_mut() {
            Some(last) if last.0 == b => last.1 = last.1.max(v),
            _ => out.push((b, v)),
        }
    }
    out
}

/// Hidden-tab intervals from visibility changes, relative to the start.
fn hidden_intervals(vis: &[(f64, String)], t0: f64, end: f64) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    let mut from: Option<f64> = None;
    for (t, state) in vis {
        let t = t - t0;
        match (state.as_str(), from) {
            ("hidden", None) => from = Some(t),
            ("visible", Some(f)) => {
                out.push((f, t));
                from = None;
            }
            _ => {}
        }
    }
    if let Some(f) = from {
        out.push((f, end.max(f)));
    }
    out
}

/// An event that starts a run on the beat: the first one followed by an on-beat gap.
fn anchor(times: &[f64], period_s: f64) -> f64 {
    let ev = beat::merged_s(times);
    ev.windows(2).find(|w| beat::on_beat(period_s, w[1] - w[0])).map(|w| w[0]).or(ev.first().copied()).unwrap_or(0.0)
        * 1000.0
}
