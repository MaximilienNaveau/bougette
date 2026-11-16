//! Bougette's API for the Flutter app (M3). The app calls only what is in
//! `api`, through flutter_rust_bridge, which generates the Dart side.
//!
//! Every figure the app shows is computed here, from `bougette-core`: the
//! app displays, and forwards what the user does (CLAUDE.md, §4). Amounts
//! cross as whole cents (`i64`) with their currency's code; formatting
//! them for people is the app's job.

pub mod api;
// Written by flutter_rust_bridge's generator: the C functions Dart calls.
mod frb_generated;
