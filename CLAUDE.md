# Instructions for Claude: lagscope-core

The shared engine of the LagScope desktop app (https://github.com/sevencooper/LagScope,
public) and LagScope Pulse (https://github.com/sevencooper/lagscope-pulse, private). Read
`README.md` first: what belongs here, how both products adopt a version.

## Hard rules

- **Never put a link to a Claude chat or session** (claude.ai session URLs, or the
  session-link trailer) in commits, pull requests, issues, comments or files.
- Git identity: `sevencooper`, `13198285+sevencooper@users.noreply.github.com`. Never a
  private e-mail address.
- Commit and push only when the owner asks; every change goes through a pull request; the
  owner merges and tags versions.
- Commit messages end with `Co-Authored-By: Claude <model> <noreply@anthropic.com>`.
- Public repository, MIT: no personal data (recordings of real lines, IP addresses,
  locations); permissive dependencies only (MIT, Apache-2.0, ISC, BSD), and as few as possible.
- No platform code: the crate must build for Windows, Linux and `wasm32-unknown-unknown`.

## Working here

- Plain language in everything people read; short chat answers, details in files.
- A change that alters results (a rule, a threshold, a score) updates the fixtures and says
  why in the pull request; both products must adopt it, so the CHANGELOG entry says how.
- Mermaid diagrams are checked with Mermaid's parser before committing.
