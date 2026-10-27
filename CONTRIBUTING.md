# Contributing

This crate is shared by the LagScope app and LagScope Pulse, so a change here changes both.

- **A wrong result** (a freeze missed, a timer that is not there): open an issue in the
  product where you saw it (LagScope or Pulse); the fix lands here if it is a shared rule.
- **Code:** open an issue first for anything beyond a small fix. A pull request needs tests;
  if it changes results, updated fixtures and the reason; and a `CHANGELOG.md` entry saying
  what each product must do to adopt it.
- **No personal data** in the repository: fixtures are synthetic.
