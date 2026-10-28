# Changelog

What changed in lagscope-core, for the two products that use it (the LagScope app and
LagScope Pulse). Each version says what the products need to do to adopt it. New changes go
under "Unreleased".

## Unreleased

## 0.2.0: grades and jitter from plain numbers

No result changes (the fixtures are unchanged). New API, for a product that measures in its
own way rather than with the stream test:

- `quality::Inputs` and `quality::grade_inputs`: "Good for" from plain numbers (idle round
  trip, jitter, loss, increase under load, download speed, whether it froze). `quality` now
  fills `Inputs` from a stream test and calls it.
- `stream::jitter`: jitter as Cloudflare defines it (the average change between consecutive
  round trips), the same calculation the stream test uses.
- `design::TOKENS_JSON`: `design/tokens.json`, readable from a build script that depends on
  this crate.

To adopt: **Pulse**: nothing required (same functions, same results). **LagScope app**: grade
its recordings with `grade_inputs` and `jitter`, and read the tokens from `TOKENS_JSON`.

## 0.1.0 (2026-10-27): the first version

- The two-way stream test's analysis (`stream::analyze`): stalls per direction (held or lost,
  sender late, device, hidden tab, under load), loss, round trip and jitter, timers, the
  under-load phases, scorecard and verdict. Moved from LagScope Pulse, where it runs today.
- `beat` and `second_timer` from LagScope 0.6.0, unchanged.
- "Good for" grades (`quality::quality`): Cloudflare's network quality scoring plus the freeze
  cap for gaming. Ported from Pulse's website code.
- `design/tokens.json`, `design/DESIGN.md`, `design/wording.md`, `docs/API.md`, `docs/RULES.md`.

To adopt: **Pulse** replaces its own analysis crate and its TypeScript "Good for" code with
this crate (same results). **LagScope app**: nothing yet; start with `design/` when restyling.

