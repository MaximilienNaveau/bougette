//! Bougette's core: the domain model, with no UI, network or storage.
//!
//! `//!` comments document the enclosing item (here the whole crate), and
//! `///` comments document the item that follows; `cargo doc` turns both
//! into HTML, like Doxygen.
//!
//! The words used here are the ones defined in GLOSSARY.md.

// `pub mod x;` declares a module whose code lives in `src/x.rs` and makes it
// visible to users of the crate. A module is a namespace *and* a file,
// roughly a C++ namespace + header/source pair, but with no #include: the
// compiler finds the file from the module name.
pub mod budget;
pub mod export;
pub mod id;
pub mod line;
pub mod money;
pub mod month;
pub mod pot;
pub mod schedule;
pub mod shared;
pub mod split;
pub mod transaction;
