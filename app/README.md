# Bougette's app

The Flutter app for Linux (milestone M3 of [doc/scope.md](../doc/scope.md)).
It shows what the Rust core computes, through the API in
[crates/bougette-api](../crates/bougette-api), and computes nothing itself.

```sh
nix run .#app              # build and run it (from the repository's root)
```

It opens the same database as the command-line client
(`$XDG_DATA_HOME/bougette/bougette.sqlite`, or `$BOUGETTE_DB`).

Developing, in `nix develop`:

```sh
cargo build --release -p bougette-api          # the Rust library
cd app
flutter_rust_bridge_codegen generate           # after changing the API
flutter analyze
BOUGETTE_LIB=../target/release/libbougette_api.so flutter test
```

`lib/src/rust/` and `crates/bougette-api/src/frb_generated.rs` are written
by the code generator: don't edit them by hand.
