# Changelog

What changed in lagscope-core, for the two products that use it (the LagScope app and
LagScope Pulse). Each version says what the products need to do to adopt it. New changes go
under "Unreleased".

## Unreleased

### 0.1.0 (planned): the first version

- The two-way stream test's analysis (`stream::analyze`): stalls per direction (held or lost,
  sender late, device, hidden tab, under load), loss, round trip and jitter, timers, the
  under-load phases, scorecard and verdict. Moved from LagScope Pulse, where it runs today.
- `beat` and `second_timer` from LagScope 0.6.0, unchanged.
- "Good for" grades (`quality::quality`): Cloudflare's network quality scoring plus the freeze
  cap for gaming. Ported from Pulse's website code.
- `design/tokens.json`, `design/DESIGN.md`, `design/wording.md`, `docs/API.md`, `docs/RULES.md`.

To adopt: **Pulse** replaces its own analysis crate and its TypeScript "Good for" code with
this crate (same results). **LagScope app**: nothing yet; start with `design/` when restyling.

