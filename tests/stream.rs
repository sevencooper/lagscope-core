//! Fixtures: whole synthetic stream tests through `analyze`, with the verdict and scorecard
//! both products must get. A change here changes results for both (README.md).

use lagscope_core::stream::{Analysis, Recording, analyze};
use serde_json::json;

const RATE: f64 = 64.0;
const ONE_WAY: f64 = 10.0;
/// The server's clock is far from the browser's; the analysis must not care.
const SERVER_CLOCK: f64 = 9_876_543.0;
/// The analysis's amber line for extra round trip under load (ms).
const LOAD_LIMIT: f64 = 50.0;

#[derive(Default)]
struct Line {
    secs: f64,
    /// (start ms, length ms) when the download holds packets.
    down: Vec<(f64, f64)>,
    /// (start ms, length ms) when the upload holds packets.
    up: Vec<(f64, f64)>,
    /// (start ms, length ms) when the page is busy and sends nothing.
    page: Vec<(f64, f64)>,
    /// Every n-th packet lost, both ways.
    lose_every: Option<u32>,
    /// (from, to) ms with the tab hidden.
    hidden: Option<(f64, f64)>,
    /// (from, to, extra ms) when packets queue behind a full line, both ways.
    delay: Vec<(f64, f64, f64)>,
    /// Load phases as the page records them: ("down" | "up", from, to).
    load: Vec<(&'static str, f64, f64)>,
    /// One-way delay (ms); ONE_WAY when not set.
    one_way: Option<f64>,
}

fn held(t: f64, windows: &[(f64, f64)]) -> f64 {
    windows.iter().find(|&&(a, ms)| t >= a && t < a + ms).map_or(t, |&(a, ms)| a + ms)
}

fn delayed(t: f64, windows: &[(f64, f64, f64)]) -> f64 {
    windows.iter().find(|&&(a, b, _)| t >= a && t < b).map_or(t, |&(_, _, d)| t + d)
}

impl Line {
    fn record(&self) -> Recording {
        let t0 = 1_000.0;
        let step = 1000.0 / RATE;
        let n = (self.secs * RATE) as u32;
        let one_way = self.one_way.unwrap_or(ONE_WAY);
        let (mut sent, mut down, mut echo, mut up, mut frames) = (vec![], vec![], vec![], vec![], vec![]);
        for seq in 0..n {
            let planned = seq as f64 * step;
            // Browser to server.
            let actual = held(planned, &self.page);
            sent.push(json!([seq, t0 + planned, t0 + actual]));
            let lost = self.lose_every.is_some_and(|k| seq % k == k - 1);
            let at_server = delayed(held(actual + one_way, &self.up), &self.delay) + (seq % 3) as f64 * 0.05;
            if !lost {
                up.push(json!([seq, (at_server + SERVER_CLOCK) * 1000.0]));
                let back = delayed(held(at_server + one_way, &self.down), &self.delay);
                echo.push(json!([seq, back - actual, (at_server + SERVER_CLOCK) * 1000.0]));
            }
            // Server to browser.
            let srv = (planned + SERVER_CLOCK) * 1000.0;
            if !lost {
                let at = delayed(held(planned + one_way, &self.down), &self.delay) + (seq % 3) as f64 * 0.05;
                down.push(json!([seq, srv, srv, t0 + at]));
            }
        }
        for &(a, ms) in &self.page {
            frames.push(json!([t0 + a + ms, ms]));
        }
        let vis = match self.hidden {
            Some((a, b)) => json!([[t0 + a, "hidden"], [t0 + b, "visible"]]),
            None => json!([]),
        };
        serde_json::from_value(json!({
            "meta": { "t0_ms": t0, "rate": RATE },
            "sent": sent, "down": down, "echo": echo, "up": up, "vis": vis, "frames": frames,
            "load": self.load.iter().map(|&(k, a, b)| json!([k, t0 + a, t0 + b])).collect::<Vec<_>>(),
        }))
        .unwrap()
    }
}

fn every(start: f64, period: f64, ms: f64, secs: f64) -> Vec<(f64, f64)> {
    let mut v = vec![];
    let mut t = start;
    while t < secs * 1000.0 - 5_000.0 {
        v.push((t, ms));
        t += period;
    }
    v
}

fn statuses(a: &Analysis) -> String {
    a.score.iter().map(|l| format!("{}={:?}", l.area, l.status)).collect::<Vec<_>>().join(" ")
}

#[test]
fn steady_line() {
    let a = analyze(&Line { secs: 180.0, ..Default::default() }.record());
    assert_eq!(a.verdict, "steady", "{}", statuses(&a));
    assert!((a.rtt.median_ms - 2.0 * ONE_WAY).abs() < 1.0);
    assert_eq!(a.down.loss_pct, 0.0);
}

#[test]
fn freezes_on_a_timer_both_ways() {
    let f = every(6_400.0, 10_000.0, 150.0, 180.0);
    let a = analyze(&Line { secs: 180.0, down: f.clone(), up: f, ..Default::default() }.record());
    assert_eq!(a.verdict, "timer", "{}", statuses(&a));
    let t = a.timer.as_ref().unwrap();
    assert_eq!(t.period_s, 10.0);
    assert_eq!(format!("{:?}", t.dir), "Both");
    assert!((t.typical_ms - 150.0).abs() < 25.0, "{}", t.typical_ms);
    assert_eq!(a.down.felt, a.up.felt);
    assert_eq!(a.together, a.down.felt);
    assert!(a.down.held == a.down.stalls, "freezes are held, not lost");
}

#[test]
fn upload_only_freezes() {
    // Irregular upload freezes, no timer.
    let up: Vec<(f64, f64)> =
        [12_000.0, 31_000.0, 47_500.0, 80_000.0, 95_250.0, 140_100.0].iter().map(|&t| (t, 200.0)).collect();
    let a = analyze(&Line { secs: 180.0, up, ..Default::default() }.record());
    assert_eq!(a.verdict, "upload", "{}", statuses(&a));
    assert_eq!(a.up.felt, 6);
    assert_eq!(a.down.stalls, 0);
}

#[test]
fn busy_page_is_the_device_not_the_network() {
    let page = every(3_500.0, 7_300.0, 250.0, 120.0);
    let a = analyze(&Line { secs: 120.0, page, ..Default::default() }.record());
    assert_eq!(a.verdict, "device", "{}", statuses(&a));
    assert_eq!(a.up.stalls, 0, "browser sent late: not an upload freeze");
    // 16 busy moments, each cutting both streams: counted once each.
    assert_eq!(a.device.pauses, 16, "{}", a.device.pauses);
}

#[test]
fn packet_loss() {
    let a = analyze(&Line { secs: 120.0, lose_every: Some(40), ..Default::default() }.record());
    assert_eq!(a.verdict, "loss", "{}", statuses(&a));
    assert!((a.down.loss_pct - 2.5).abs() < 0.2, "{}", a.down.loss_pct);
}

#[test]
fn hidden_tab_is_not_judged() {
    // A freeze while the tab is hidden is marked, not counted.
    let down = vec![(40_000.0, 300.0), (45_000.0, 300.0), (50_000.0, 300.0)];
    let a = analyze(&Line { secs: 120.0, down, hidden: Some((38_000.0, 55_000.0)), ..Default::default() }.record());
    assert_eq!(a.down.stalls, 0);
    assert_eq!(a.down.hidden, 3);
    assert_eq!(a.device.hidden_times, 1);
    assert_eq!(a.verdict, "steady", "{}", statuses(&a));
}

#[test]
fn too_short_is_not_enough() {
    let a = analyze(&Line { secs: 10.0, ..Default::default() }.record());
    assert_eq!(a.verdict, "not_enough");
}

#[test]
fn few_freezes_both_ways_are_not_a_one_second_timer() {
    // Five freezes, each cutting both streams: ten pauses, but five events.
    let f = every(20_000.0, 10_000.0, 150.0, 75.0);
    let a = analyze(&Line { secs: 75.0, down: f.clone(), up: f, ..Default::default() }.record());
    assert!(a.second_timer.is_none());
    assert_eq!(a.verdict, "freezes", "{}", statuses(&a));
}

#[test]
fn ping_jumps_under_load() {
    // Steady when idle; 120 ms of queueing while the download is full, 15 ms while uploading.
    let line = Line {
        secs: 180.0,
        delay: vec![(140_000.0, 160_000.0, 120.0), (160_000.0, 180_000.0, 15.0)],
        load: vec![("down", 140_000.0, 160_000.0), ("up", 160_000.0, 180_000.0)],
        ..Default::default()
    };
    let a = analyze(&line.record());
    assert_eq!(a.verdict, "load", "{}", statuses(&a));
    let l = a.load.as_ref().unwrap();
    // The queue adds 120 ms to the way back and, for echoes, to the way there as well.
    assert!(l.down_extra_ms.unwrap() > 100.0, "{:?}", l.down_extra_ms);
    assert!(l.up_extra_ms.unwrap() < LOAD_LIMIT, "{:?}", l.up_extra_ms);
    assert!((a.rtt.median_ms - 2.0 * ONE_WAY).abs() < 1.0, "idle round trip is judged on idle time only");
}

#[test]
fn freezes_while_loaded_stay_out_of_the_idle_verdict() {
    let f = every(141_000.0, 4_000.0, 200.0, 165.0);
    let line = Line {
        secs: 180.0,
        down: f.clone(),
        up: f,
        load: vec![("down", 140_000.0, 160_000.0), ("up", 160_000.0, 180_000.0)],
        ..Default::default()
    };
    let a = analyze(&line.record());
    assert_eq!(a.down.felt, 0, "load-phase freezes are not idle freezes");
    assert!(a.load.as_ref().unwrap().down.as_ref().unwrap().freezes >= 4);
    assert_ne!(a.verdict, "freezes");
}

// "Good for" grades on the same fixtures.
use lagscope_core::quality::{Grade, Note, Speed, quality};

fn steady_with_load() -> Line {
    Line {
        secs: 180.0,
        delay: vec![(140_000.0, 160_000.0, 15.0), (160_000.0, 180_000.0, 15.0)],
        load: vec![("down", 140_000.0, 160_000.0), ("up", 160_000.0, 180_000.0)],
        ..Default::default()
    }
}

#[test]
fn good_for_a_steady_line() {
    let a = analyze(&steady_with_load().record());
    let speed = Speed { down: Some(300e6), ..Default::default() };
    let q = quality(&a, Some(&speed));
    // 20 ms round trip (5 points: Cloudflare's step is "below 20"), no loss (10), +~30 ms under
    // load (5): 20 = Average for gaming.
    assert_eq!(q[0].grade, Some(Grade::Average), "{q:?}");
    assert!(q.iter().all(|x| x.grade.is_some()), "{q:?}");
}

#[test]
fn freezes_cap_gaming() {
    let f = every(6_400.0, 10_000.0, 150.0, 140.0);
    // A fast line (8 ms round trip: 20 points) with little bloat (+2 ms: 20) would be Great.
    let mut line = Line {
        secs: 180.0,
        one_way: Some(4.0),
        delay: vec![(140_000.0, 160_000.0, 1.0), (160_000.0, 180_000.0, 1.0)],
        load: vec![("down", 140_000.0, 160_000.0), ("up", 160_000.0, 180_000.0)],
        ..Default::default()
    };
    line.down = f.clone();
    line.up = f;
    let a = analyze(&line.record());
    assert_eq!(a.verdict, "timer");
    let q = quality(&a, None);
    assert_eq!(q[0].grade, Some(Grade::Average), "{q:?}");
    assert_eq!(q[0].note, Some(Note::FreezeCap), "{q:?}");
    assert_eq!(q[1].grade, Some(Grade::Great), "video calls are not capped: {q:?}");
}

#[test]
fn missing_inputs_are_named() {
    let a = analyze(&Line { secs: 120.0, ..Default::default() }.record());
    let q = quality(&a, None);
    assert!(q.iter().all(|x| x.grade.is_none()));
    assert_eq!(q[0].note, Some(Note::MissingLoad));
    assert_eq!(q[2].note, Some(Note::MissingSpeed));
}

#[test]
fn a_saved_result_reads_back_the_same() {
    // Products keep only the analysis (history, share links) and grade it again later.
    let a = analyze(&steady_with_load().record());
    let back: Analysis = serde_json::from_str(&serde_json::to_string(&a).unwrap()).unwrap();
    let speed = Speed { down: Some(50e6), ..Default::default() };
    assert_eq!(format!("{:?}", quality(&a, Some(&speed))), format!("{:?}", quality(&back, Some(&speed))));
    assert_eq!(back.verdict, a.verdict);
}
