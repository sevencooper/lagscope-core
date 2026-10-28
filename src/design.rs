//! The design files, for products that build their look from them (the LagScope app's
//! build.rs reads the tokens through this crate as a build dependency).

/// `design/tokens.json`: colours, fonts, radii, status shapes, chart colours.
pub const TOKENS_JSON: &str = include_str!("../design/tokens.json");
