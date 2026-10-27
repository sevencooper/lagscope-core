# Analysis rules

The rules below come from LagScope 0.6.0 (`src/analyze/detect.rs`, `findings.rs`, `health.rs` in
https://github.com/sevencooper/LagScope). Each was tuned on real recordings; the numbers are
the ones in the code. Times are microseconds in the code, shown here in ms / s.

## Stream stalls (gaps in a steady packet stream)

- A gap counts when it is longer than **4 × the local median spacing** (median of the last 64
  intervals) and longer than the game profile's minimum (80 ms by default), and shorter than 5 s
  (longer = a pause in play, not a stall).
- Only in a live stream: at least 30 packets in the 3 s before **and** after the gap (in menus
  games send a trickle).
- After the gap: 3+ packets within 20 ms (and at least 30% of the packets the gap "should" have
  had) means the packets were **held and then released together** (queueing, a freeze);
  otherwise they were **lost / never sent**.
- **Felt stalls:** 100 ms+. **Short pauses for beat detection:** from max(2 × median spacing,
  50 ms).

## Ping spikes and plateaus

- A spike: round trip above max(3 × median, median + 30 ms). Spikes closer than 300 ms merge
  into one; lost pings form their own events.
- A plateau ("latency stuck high"): the 2-second rolling median stays above median + 25 ms and
  above 1.8 × median for 3 s or more.
- A **freeze** shows as a staircase: pings sent 50 ms apart during a 220 ms freeze come back as
  ~217, 170, 108, 62 ms (all released at once).

## Upload freezes

A router-ping spike of 80 ms+, with a ping further out spiking at the same time (within
100 ms), while the game's incoming stream kept arriving through it (5+ packets, no gap over
60 ms) means the device's **uploads** were held back. In a website with both directions
measured, this is simpler: a gap in what the server receives is an upload pause.

## Beats (freezes on a timer)

Two searches; network series use the skip-tolerant one first.

**Skip-tolerant (`beat`)**, for timers that fire every p seconds but only sometimes hit:

1. Merge events closer than 2 s (one freeze can cut a stream twice). Need 8+ events.
2. For each candidate p from 2 to 60 s (step 0.02 s): a gap between consecutive events is
   "on the beat" if it is k × p (k = 1..6) within a tolerance of max(0.4 s, 4% of p).
   Score = share of gaps on the beat; margin = score − chance, with chance = 2 × tolerance / p.
3. Take the p with the best margin; require score ≥ 0.65 and margin ≥ 0.45.
4. A 10 s beat also fits 5 s and 3.33 s: take the longest whole multiple (k ≤ 6, ≤ 60 s) whose
   score is at least 90% as good.
5. Refine: p = (sum of on-beat gaps) / (sum of their k).

**Strict (`periodicity`)**, for steady timers: share of events followed by another one p later
(tolerance max(0.75 s, 3%)), p from 3 s to a quarter of the recording (at least four cycles),
chance-corrected (chance = 1 − e^(−rate × 2 × tolerance)), score − chance ≥ 0.5, preferring the
fundamental over multiples.

**Same timer in two series:** periods equal, or one a whole multiple (≤ 6) of the other within
3% (a series can catch only every second or third beat).

**1-second timers:** split each second into 20 bins of 50 ms; if 60%+ of 8+ freezes start in one
3-bin window (chance ~15%), they are locked to a 1-second timer.

**Where the timer is:** on the router ping → Wi-Fi / link to the router; on the game stream and
internet pings with the router clean → past the router (modem / provider's gateway or the
provider).

## Verdicts for a marked moment

Look at the 4 s before the mark (users press late), then 10 s with low confidence. Order:

1. Marked while switched away from the game → not judged (only if the network was clean).
2. Long frame (80 ms+) with the network clean → the device (driver, disk, system stall, or the
   game itself).
3. Game stream paused or pings spiked → the first hop that spiked: router (Wi-Fi / link;
   "uploads froze" if the game data kept arriving), ISP hop (modem / line), internet
   (provider), only the server path (route), no ping at all (game server).
4. Nothing measurable → server-side or perceptual.

The headline is the verdict most marks agree on; without marks, patterns in the whole
recording (timers first).

## Avoiding false alarms

- Judge only relevant time (match play; not loading, the first 5 s of a match, or while the
  user had switched away).
- Merge events that belong together (stalls within 2 s, slow disk I/Os within 2 s).
- Compare with chance before blaming: a program is named for stutters only if it was busy
  during them at least 30 percentage points more often than in the same window at random
  moments.
- Thresholds relative to the device's own normal where possible (driver load: a full CPU core
  above the session median).
- Amber only for things that cause lag; everything else is a note.

## Scorecard (what turns a line amber)

| Line | Amber when |
|---|---|
| Wi-Fi / link | router ping worst 1% > 20 ms, loss > 0.5%, freezes on a timer, 3+ upload freezes, link drops |
| Internet & game server | 100 ms+ stalls at 0.3+ per minute (3+), internet ping worst 1% > 100 ms, loss beyond the router > 1%, or a timer past the router |
| Frames | 1+ hitch (50 ms+) per minute of play |

The web version would need its own lines (e.g. "steadiness", "upload", "download", "under
load", "device"), but the same idea: one line per area, green / amber / not measured, with a
short reason.

## The two-way stream test (`src/stream.rs`)

The rules above, applied to the website's two streams. `beat` and `second_timer` are copied
unchanged; stalls follow `stream_gaps`, plus a check LagScope cannot do: every packet carries
its planned and actual send time, so a gap the sender caused (a busy page, a late server) is
not blamed on the network. Uploads are put on the browser's clock using the echoes (the server
got a packet about half a round trip after it left), so both directions line up on one chart.

| Scorecard line | Amber when |
|---|---|
| Freezes on a timer | a beat in the short network pauses (download, upload or both), or a 1-second timer |
| Download / Upload | 3+ network stalls of 100 ms+, at least 0.3 a minute |
| Packet loss | above 1% either way (the last second left out) |
| Round trip | worst 1% outside freezes more than 50 ms above typical |
| Under load (only when switched on) | the round trip's worst 10% while downloading or uploading is more than 50 ms above idle |
| This device | 3+ pauses the device or browser caused, or 1+ slow frame (50 ms+) a minute (not counting the load phases, whose work is ours) |

A line is "not measured" when the test judged less than 20 s (time with the tab hidden does
not count) or got fewer than 200 packets. Verdict, first that applies: not enough data, timer,
freezes both ways, upload freezes, download freezes, packet loss, ping jumps under load, ping
jumps, device, steady.

**Under load** (2026-10-26): with the option on, the last part of the test (a quarter each, 20 s
at most) fills the line with 6 parallel 25 MB downloads, then 4 parallel 8 MB uploads, from
Cloudflare's speed test endpoints, capped at 400 MB down and 200 MB up. The load runs in a Web
Worker: run on the page itself it kept the page too busy to time packets (a test from the owner's
line read +941 ms; a plain ping under the same uploads rose only ~9 ms on average). Pauses, loss
and round trip during load phases (plus 1 s for queues to drain) are judged only in the under-load
line, never in the idle verdict.
These thresholds are a first guess for the website, to be tuned on real results.

## Good for (`src/quality.rs`)

Gaming, video calls and streaming, scored with Cloudflare's network quality steps (from
`@cloudflare/speedtest` 1.15.0, MIT) so a grade means the same as on speed.cloudflare.com:

| Metric | Points |
|---|---|
| Latency, idle (ms) | <10: 20 · <20: 10 · <50: 5 · <100: 0 · <500: −10 · else −20 |
| Increase under load (ms) | same steps |
| Jitter (ms) | <10: 10 · <20: 5 · <100: 0 · <500: −10 · else −20 |
| Packet loss | <1%: 10 · <5%: 5 · <25%: 0 · <50%: −10 · else −20 |
| Download | <1 Mbit/s: 0 · <10: 5 · <50: 10 · <100: 20 · else 30 |

Gaming = latency + loss + increase (Poor / Average / Good / Great from 5 / 15 / 25 / 30);
video calls add jitter (5 / 15 / 25 / 40); streaming adds download (15 / 20 / 40 / 60). Below
the first threshold: Bad. **Freeze cap:** a timer, a 1-second timer, or felt freezes in either
direction keep gaming at Average at best (Cloudflare's score does not see freezes).
