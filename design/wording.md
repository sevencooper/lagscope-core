# Wording

The sentences both products show for the same finding, by key (the keys are what
`stream::analyze` and `quality::quality` return). `{…}` are numbers filled in by the product;
**bold** marks the parts shown in colour. Taken from LagScope Pulse 2026-10-27; the LagScope
app adopts them as it adopts this crate.

Rules for new text: plain language, short sentences, no jargon; advice comes only from what was
measured and always ends with "not guaranteed to help".

## Verdicts (`Analysis.verdict`)

| Key | Chip | Headline | Explanation |
|---|---|---|---|
| `timer` | Freezes on a timer | Your connection freezes for about **{ms} ms** every **{s} seconds**. (download only: "Your download…", upload only: "Your upload…") | Both directions stop at the same moment, then the held packets arrive at once. In a game this feels like delayed hits or rubber-banding. A fixed beat means something on the way does a job on a schedule: Wi-Fi on this device, the router, or the modem / provider's gateway. |
| `timer` (1-second) | Freezes on a timer | Your freezes start at the **same point in each second**. | {n} of {total} short pauses began within the same 150 ms of a second. Something runs on a 1-second timer: often the Wi-Fi adapter, its driver, or the router. |
| `freezes` | Short freezes | Your connection froze **{n} times** in {length}, for about **{ms} ms** each. | Data stopped in both directions. In a game this feels like freezes, delayed hits or rubber-banding. |
| `upload` | Upload freezes | Your upload froze **{n} times** in {length}, for about **{ms} ms** each. | What you send stopped while what you receive kept flowing. In a game your own actions reach the server late: shots that do not register, rubber-banding. |
| `download` | Short freezes | Your download froze **{n} times** in {length}, for about **{ms} ms** each. | What you receive stopped while what you send kept flowing. In a game the world stops for a moment, then jumps ahead. |
| `loss` | Packet loss | **{pct}%** of packets got lost on the way {to you / from you}. | Lost game packets are gone for good: the game has to guess, so players jump or hits go missing. Weak or busy Wi-Fi is a common cause. |
| `load` | Ping jumps under load | Your ping jumps by about **{ms} ms** while **someone downloads / uploads**. | When the line is full, packets wait in a queue, usually in the router or modem. Games feel that as lag whenever someone at home streams, downloads or backs up files. It is called bufferbloat, and it can often be fixed in the router. |
| `ping` | Ping jumps | Your round trip jumps: usually **{ms} ms**, but the worst 1% reach **{ms} ms**. | Uneven delay makes a game feel inconsistent even without freezes. A full line (downloads, streams, uploads on the same network) is the most common cause. |
| `device` | Your device stuttered | The connection held up, but your device **paused {n} times**. | The browser or the computer was too busy to keep up, so those moments are not blamed on the network. A busy device makes games stutter too. |
| `not_enough` | Not enough data | Not enough data to judge this connection. | The test was too short or too few packets arrived. Run it again for at least a minute. |
| `steady` | Steady | Steady. **No freezes** in {length}. (with 1-2 short ones: "Mostly steady. **{n} short freezes** in {length}.") | Data flowed without a pause in both directions, and the round trip stayed around {ms} ms (worst 1%: {ms} ms). |

## What to try (per verdict)

- `timer`: Run again with a cable straight to the router; if the beat stays, Wi-Fi is not the
  cause. If it stays on a cable, restart the modem or the provider's gateway; if it comes back,
  ask your provider about it.
- `freezes` / `download` / `upload`: If you are on Wi-Fi, run again over a cable; if the freezes
  stop, the Wi-Fi is the cause. (upload) Pause uploads on this network (cloud backup,
  streaming, game updates) and run again. Run again at another time of day; if it only happens
  at busy times, your provider's network may be full.
- `loss`: On Wi-Fi, move closer to the router or use a cable, then run again. If it stays on a
  cable, check the cable and the router, then ask your provider.
- `load`: Look for "Smart Queue", "SQM", "QoS" or "bufferbloat" in your router's settings, and
  set it a little below your measured speed. If your router has none of these, a router with
  Smart Queue support can usually fix it. Until then, pause big downloads and uploads while you
  play.
- `ping`: Pause downloads and streams on this network and run again; if it helps, look for
  "Smart Queue", "QoS" or "SQM" in your router's settings.
- `device`: Close other tabs and heavy programs, then run again.
- Always: "Based on this test; not guaranteed to help."

## Scorecard (`Analysis.score`)

Status words: `ok` "fine" (round dot), `warn` "worth a look" (square), `na` "not measured".

| Area | Name | Typical lines |
|---|---|---|
| `timer` | Freezes on a timer | "Every {s} s, about {ms} ms each. {k} of {n} freezes on the beat." / "No freezes on a schedule." |
| `download` | Download | "{n} freezes of 100 ms or more (longest {ms} ms); packets were held, then released together." / "No freezes." |
| `upload` | Upload | "Froze together with the download: the whole line stops, not one direction." / "No freezes." |
| `loss` | Packet loss | "{pct}% down, {pct}% up." |
| `load` | Under load | "+{ms} ms while downloading, +{ms} ms while uploading ({n} freezes)." |
| `rtt` | Round trip | "{ms} ms typical, {ms} ms for the worst 1% outside the freezes, jitter {ms} ms." |
| `device` | This device | "No browser or device hiccups during the test." / "Paused {n} times, {n} slow frames. Not blamed on the network." |

## Good for (`quality::Quality`)

Uses: `gaming` "Gaming", `video_calls` "Video calls", `streaming` "Streaming". Grades: Bad,
Poor, Average, Good, Great; missing: "Not measured yet". Footnote: "Graded like Cloudflare's
network quality score."

| Note | Text |
|---|---|
| `missing_load` | Switch on Under load or measure speed to include ping under load. |
| `missing_speed` | Measure speed to include download speed. |
| `freeze_cap` | Would be better without the freezes this test found. |
| `freezes_hurt` | The freezes this test found hurt games. (followed by `missing_load`) |
