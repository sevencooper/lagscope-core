//! "Good for" grades: gaming, video calls and streaming, scored exactly like Cloudflare's network
//! quality score (AIM; the point steps and thresholds below are from `@cloudflare/speedtest`
//! 1.15.0, MIT), so a grade means the same here as on speed.cloudflare.com. One addition: for
//! gaming, freezes (which Cloudflare's score does not see) cap the grade at Average.
//!
//! Returns grades and note keys; the sentences are in `design/wording.md`.

use crate::stream::{Analysis, Status};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Grade {
    Bad,
    Poor,
    Average,
    Good,
    Great,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Use {
    Gaming,
    VideoCalls,
    Streaming,
}

/// Why a grade is missing or capped (`design/wording.md`, "Good for").
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Note {
    /// No ping-under-load measurement yet.
    MissingLoad,
    /// No download speed yet (streaming).
    MissingSpeed,
    /// Capped at Average because of the freezes found.
    FreezeCap,
    /// Not graded yet, but the freezes found hurt games.
    FreezesHurt,
}

#[derive(Debug, Clone, Serialize)]
pub struct Quality {
    #[serde(rename = "use")]
    pub use_: Use,
    pub grade: Option<Grade>,
    pub note: Option<Note>,
}

/// A speed test's results (bits per second, milliseconds), when one ran.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Speed {
    pub down: Option<f64>,
    pub up: Option<f64>,
    pub ping: Option<f64>,
    pub ping_down: Option<f64>,
    pub ping_up: Option<f64>,
}

/// Cloudflare's scaleThreshold: below `domain[i]` gives `range[i]`, above all the last.
fn steps(domain: &[f64], range: &[i32], v: f64) -> i32 {
    range[domain.iter().position(|&d| v < d).unwrap_or(domain.len())]
}

fn latency_points(ms: f64) -> i32 {
    steps(&[10.0, 20.0, 50.0, 100.0, 500.0], &[20, 10, 5, 0, -10, -20], ms)
}

fn jitter_points(ms: f64) -> i32 {
    steps(&[10.0, 20.0, 100.0, 500.0], &[10, 5, 0, -10, -20], ms)
}

fn loss_points(share: f64) -> i32 {
    steps(&[0.01, 0.05, 0.25, 0.5], &[10, 5, 0, -10, -20], share)
}

fn download_points(bps: f64) -> i32 {
    steps(&[1e6, 1e7, 5e7, 1e8], &[0, 5, 10, 20, 30], bps)
}

/// Points needed for Poor, Average, Good, Great (below the first: Bad).
fn grade(use_: Use, total: i32) -> Grade {
    let t: [i32; 4] = match use_ {
        Use::Gaming => [5, 15, 25, 30],
        Use::VideoCalls => [5, 15, 25, 40],
        Use::Streaming => [15, 20, 40, 60],
    };
    [Grade::Bad, Grade::Poor, Grade::Average, Grade::Good, Grade::Great][t.iter().filter(|&&x| total >= x).count()]
}

/// What the grades are made of, as plain numbers, for a product that measures them its own way
/// (the LagScope app). `quality` fills it from a stream test.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Inputs {
    /// Idle round trip, median (ms).
    pub latency_ms: f64,
    /// Jitter (ms), as `stream::jitter` computes it.
    pub jitter_ms: f64,
    /// Packet loss, the worse direction (%).
    pub loss_pct: f64,
    /// How much the round trip rises while the line is busy (ms); None = not measured.
    pub load_increase_ms: Option<f64>,
    /// Download speed (bits per second); None = not measured.
    pub download_bps: Option<f64>,
    /// Freezes were found (a timer, a 1-second timer, or felt freezes in either direction):
    /// gaming is capped at Average.
    pub froze: bool,
}

/// The three grades for a stream test, with the speed test's results if it ran.
pub fn quality(a: &Analysis, speed: Option<&Speed>) -> Vec<Quality> {
    if a.verdict == "not_enough" {
        return Vec::new();
    }
    // Increase under load: from the under-load phases if they ran, else the speed test's.
    let increase = match (&a.load, speed) {
        (Some(l), _) if l.down_extra_ms.is_some() || l.up_extra_ms.is_some() => {
            Some(l.down_extra_ms.unwrap_or(0.0).max(l.up_extra_ms.unwrap_or(0.0)))
        }
        (_, Some(Speed { ping: Some(p), ping_down, ping_up, .. })) if ping_down.is_some() || ping_up.is_some() => {
            Some((ping_down.unwrap_or(*p) - p).max(ping_up.unwrap_or(*p) - p).max(0.0))
        }
        _ => None,
    };
    let froze = a.timer.is_some()
        || a.second_timer.is_some()
        || a.score.iter().any(|l| (l.area == "download" || l.area == "upload") && l.status == Status::Warn);
    grade_inputs(&Inputs {
        latency_ms: a.rtt.median_ms,
        jitter_ms: a.rtt.jitter_ms,
        loss_pct: a.down.loss_pct.max(a.up.loss_pct),
        load_increase_ms: increase,
        download_bps: speed.and_then(|s| s.down),
        froze,
    })
}

/// The three grades from plain numbers (`Inputs`).
pub fn grade_inputs(i: &Inputs) -> Vec<Quality> {
    let latency = latency_points(i.latency_ms);
    let loss = loss_points(i.loss_pct / 100.0);
    let jitter = jitter_points(i.jitter_ms);
    let load = i.load_increase_ms.map(latency_points);
    let missing_load = load.is_none().then_some(Note::MissingLoad);
    let froze = i.froze;
    let gaming = match load.map(|l| grade(Use::Gaming, latency + loss + l)) {
        Some(g) if froze && g > Grade::Average => {
            Quality { use_: Use::Gaming, grade: Some(Grade::Average), note: Some(Note::FreezeCap) }
        }
        Some(g) => Quality { use_: Use::Gaming, grade: Some(g), note: None },
        None => Quality {
            use_: Use::Gaming,
            grade: None,
            note: Some(if froze { Note::FreezesHurt } else { Note::MissingLoad }),
        },
    };
    let calls = Quality {
        use_: Use::VideoCalls,
        grade: load.map(|l| grade(Use::VideoCalls, latency + jitter + loss + l)),
        note: missing_load,
    };
    let down = i.download_bps;
    let streaming = Quality {
        use_: Use::Streaming,
        grade: load.zip(down).map(|(l, d)| grade(Use::Streaming, latency + loss + download_points(d) + l)),
        note: if down.is_none() { Some(Note::MissingSpeed) } else { missing_load },
    };
    vec![gaming, calls, streaming]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_steps_match_cloudflare() {
        assert_eq!(latency_points(9.9), 20);
        assert_eq!(latency_points(10.0), 10);
        assert_eq!(latency_points(600.0), -20);
        assert_eq!(loss_points(0.0), 10);
        assert_eq!(loss_points(0.6), -20);
        assert_eq!(download_points(5e8), 30);
        assert_eq!(jitter_points(15.0), 5);
    }

    #[test]
    fn grades_from_plain_numbers() {
        let i = Inputs {
            latency_ms: 8.0,
            jitter_ms: 2.0,
            loss_pct: 0.0,
            load_increase_ms: Some(5.0),
            download_bps: Some(3e8),
            froze: false,
        };
        let q = grade_inputs(&i);
        // 20 + 10 + 20 = 50: Great for gaming; calls add jitter (60), streaming download (80).
        assert!(q.iter().all(|q| q.grade == Some(Grade::Great) && q.note.is_none()));
        let q = grade_inputs(&Inputs { froze: true, ..i.clone() });
        assert_eq!((q[0].grade, q[0].note), (Some(Grade::Average), Some(Note::FreezeCap)));
        let q = grade_inputs(&Inputs { load_increase_ms: None, download_bps: None, ..i });
        assert_eq!((q[0].grade, q[0].note), (None, Some(Note::MissingLoad)));
        assert_eq!((q[2].grade, q[2].note), (None, Some(Note::MissingSpeed)));
    }

    #[test]
    fn grade_thresholds() {
        assert_eq!(grade(Use::Gaming, 4), Grade::Bad);
        assert_eq!(grade(Use::Gaming, 30), Grade::Great);
        assert_eq!(grade(Use::VideoCalls, 39), Grade::Good);
        assert_eq!(grade(Use::Streaming, 60), Grade::Great);
    }
}
