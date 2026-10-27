# Design

The look both products share. The values are in `tokens.json`; change them there and adopt
the new version in both (README.md).

**Decided (2026-10-26): direction A, "Pulse line".** Chosen from three mock-ups (A Pulse line,
B Dial, C Timeline) on the owner's private design canvas.

The idea: a live heartbeat line of the connection. Steady data is a calm mint line; a freeze is
an orange bar. On the result, a repeating freeze lines up with a dashed beat grid, so a timer
is visible at a glance.

## Look

| Token | Value | Use |
|---|---|---|
| Ground | `#0A0C0F` | page background |
| Surface | `#11151A`, border `#222A33` | cards |
| Raised | `#161B21`, border `#2A333D` | buttons, progress track |
| Text | `#EAF0F5` | main text |
| Muted | `#93A0AD` | labels, secondary text (passes 4.5:1 on the ground) |
| Steady | `#5BE7C4` (mint), dark text on it `#06261D` | Start button, the live line, "fine" |
| Freeze | `#FF8A3D` (orange), soft `#FFB27A` on `#2A1A0E` | freeze bars, "worth a look" |
| Verdict card | `#15120F`, border `#3A2A1E` | the headline result |

- Mint and orange also differ in shape, not only colour: "fine" is a round dot, "worth a
  look" a square; freezes are bars, steady data a line.
- Type: **Space Grotesk** (headings, wordmark, Start), **Inter** (text), **JetBrains Mono**
  (numbers, times). All three are under the SIL Open Font License, fine for a closed product.
  **Self-host the fonts** rather than loading them from Google Fonts, so visitors' browsers do
  not contact Google (privacy first).
- Radii: cards 20 px (18 on phones), buttons fully round. Touch targets at least 44 px.
- Motion: the Start button's soft pulse ring; the live line drawing in. Both off under
  "reduce motion".

## Screens

1. **Before:** wordmark, "Not how fast. How steady.", one line of explanation, the round
   Start button (3 min test), choices as chips (game preset, length, server), "Keep this tab
   open. Nothing is stored.", then cards for what Pulse checks (Ready / Coming soon).
2. **During:** label (server, preset), "Listening for freezes…", time and progress bar, the
   live chart (round trip line, freeze bars), four tiles (round trip, freezes so far,
   download, upload), Stop, and the hint to keep the tab in front.
3. **Result:** verdict card (status chip, one-sentence headline with the key numbers in
   orange, short explanation), the whole-test chart with the beat grid, scorecard (one line
   per area), speed (Cloudflare), "What to try" (with "not guaranteed to help"), actions (Run
   again, Compare, Copy summary, Share link, Download), and "Go deeper" cards for features
   that are coming soon.

## Phones

Same three screens, one column: chips wrap, tiles go two by two, actions become full-width
buttons, the scorecard drops the explanations (status words only) to stay short. The desktop
pages are built to reflow down to phone width as well.

## More screens (canvas page "Pulse (chosen: A)")

- **All fine:** green verdict card ("Steady. No freezes in three minutes."), flat chart, all
  lines green, and "Still lagging in games?" pointing to the LagScope app, plus a hint to test
  again at the time it usually lags.
- **How it works:** "Lag is a pause, not a slow line", a browser ⇄ server diagram (each side
  times what arrives), four steps, what each scorecard line means, Pulse vs. the LagScope app,
  privacy in three cards.
- **Home network test (coming soon):** QR code to pair a phone; the result splits the path
  (devices → router → internet) and colours the part that froze.
- **Feature switches:** card states Ready / Coming soon / Busy / Not available (plus "off" =
  hidden), and the Start button when the server is offline or busy.
- **Warnings:** tab was hidden (time marked, not judged), slower fallback channel, device
  paused, lost the server mid-test.
- **Verdict chips:** Steady, Freezes on a timer, Upload freezes, Short freezes, Packet loss,
  Ping jumps under load, Your device stuttered, Not enough data.

## Not decided yet

- Exact wording of verdicts and scorecard lines; final copy once real results exist.
- Light mode (none so far).
