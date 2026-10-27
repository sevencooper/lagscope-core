# lagscope-core

The shared engine of two products that find out where game lag comes from:

- **LagScope**, the Windows app that records while you play
  ([sevencooper/LagScope](https://github.com/sevencooper/LagScope), open source);
- **LagScope Pulse**, the website that tests a connection's steadiness
  (https://lagscope.sevencooper.com).

Whatever both must agree on lives here, once, so the two say the same thing about the same
line, look like one family, and talk to the same server the same way.

## What belongs here

| Part | Where | Used by |
|---|---|---|
| Analysis rules: stalls (held or lost, sender late), beats, 1-second timers, thresholds | `src/` (Rust) | the app natively, Pulse compiled to WebAssembly |
| Scoring: scorecard ambers, "Good for" grades (Cloudflare's network quality steps plus a freeze cap), jitter, under load | `src/` | both |
| Test fixtures: synthetic recordings with known answers | `tests/` | both run them in CI and must agree |
| Design tokens: colours, fonts, status shapes | `design/tokens.json`, `design/DESIGN.md` | Pulse's Tailwind theme, the app's egui look |
| Wording: verdict and scorecard sentences by key | `design/wording.md` | both |
| The test protocol and Pulse's server API | `docs/API.md` | Pulse's server and website, the app |

What does **not** belong here: anything tied to one platform or one product (Windows
collectors, the website's screens, the server's code). The crate has no platform code, so it
builds for Windows, Linux and `wasm32-unknown-unknown`.

## How the products use it

Both depend on a tagged version:

```toml
lagscope-core = { git = "https://github.com/sevencooper/lagscope-core", tag = "v0.1.0" }
```

A change that affects both goes like this:

1. A pull request here: the change, its tests, updated fixtures if results change, and a
   `CHANGELOG.md` entry that says what each product must do to adopt it.
2. After merging, the maintainer tags a version (`vX.Y.Z`; only admins can create tags).
3. One pull request in each product: "update lagscope-core to vX.Y.Z", plus any screen or
   wording that needs it. Each product's CI warns when a newer tag exists.

Versions: while below 1.0, a new minor version (`0.2.0`) may change results or the API;
a patch (`0.1.1`) never does.

## Repository settings

Applied with `scripts/apply-repo-settings.sh` (safe to run again): squash merges only,
branches deleted after merge, `main` changed only through pull requests with the checks
`linux`, `windows` and `no-chat-links`, version tags only by admins, Dependabot and secret
scanning on, private vulnerability reports on.

## License

MIT. See [LICENSE](LICENSE).
