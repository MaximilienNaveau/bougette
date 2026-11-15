# Bougette: ground rules

A local-first, cross-platform app to manage several individual and shared
budgets (see [README.md](README.md), the idea in
[doc/README.md](doc/README.md), and the research in
[doc/research/](doc/research/)). Planned stack: a Rust core (domain
logic, SQLite storage, sync, encryption) with a thin Flutter UI linked by
flutter_rust_bridge. Priority platforms: Android, Linux, Windows.

The code is written mostly by Claude, and the user reads it **to learn
Rust**. The user knows C++, C, Python, CMake and Nix well, and has never
written Rust or Dart. These rules apply to everything in this repository
and override the general defaults for comments and commit granularity.

## 1. Minimalistic git commits
- One logical change per commit: don't bundle unrelated fixes/features.
- Short, imperative commit messages describing *what* changed.
- Sign off every commit (`git commit -s`, DCO, see CONTRIBUTING.md).
- Never commit unless explicitly asked to.

## 2. Explain the Rust (and later the Dart)
- Every non-trivial line gets a comment saying what it does.
- Lean on C++ analogies wherever they hold: moves as `std::move` by
  default, `Box` ≈ `unique_ptr`, `Arc` ≈ `shared_ptr`, `&T`/`&mut T` ≈
  `const T&`/`T&` checked by the compiler, traits ≈ concepts/abstract
  base classes, `cargo` ≈ CMake + a package manager.
- Where Rust has no C++/Python equivalent (the borrow checker,
  lifetimes, `Result`/`?` instead of exceptions, `Option` instead of null,
  exhaustive `match`, `Send`/`Sync`, `async` executors), say so explicitly
  rather than forcing a comparison.
- The first time a concept appears, explain it briefly, even if it recurs
  later without a fresh comment. Trivial repeated boilerplate needs no
  comment once the pattern has been explained.

## 3. Minimalistic code
- Simplest code that does the job: no speculative abstractions, no unused
  scaffolding, no "just in case" flexibility.
- Don't add a crate or package unless it's clearly needed for the task at
  hand.

## 4. Separation of concerns
- All domain logic (budgets, transactions, allocation, extrapolation),
  storage, sync and crypto live in the Rust core, with unit tests.
- The UI only displays and forwards user actions. No business rules in
  Dart.
- Sync and transport are separate layers: the merge logic must not depend
  on how messages travel.

## 5. Money
- Amounts are integers in minor units (cents) with an explicit currency.
  Never use floating point for money.

## 6. Open source, reproducible, distributable
The app must ship on F-Droid (reproducible build, our signed APK) and in
nixpkgs, under `MIT OR Apache-2.0`.
- Dependencies: permissive licences only (MIT, Apache-2.0, BSD, ISC, Zlib,
  Unicode…). No GPL/LGPL/AGPL in the app; ask before adding MPL.
- No prebuilt binaries downloaded at build time, no Google Play Services,
  Firebase, analytics or tracking. No default sync server URL.
- `flake.nix` is the source of truth for every tool version. Commit lock
  files (`flake.lock`, `Cargo.lock`, `pubspec.lock`).
- The user's game at `~/devel/workspace_trampoline/src/trampoline` is the
  reference for the Nix + F-Droid reproducible-build and release pipeline.

## 7. Domain language and decisions
- [GLOSSARY.md](GLOSSARY.md) holds the canonical domain terms. Use them in
  code identifiers and docs, and avoid the words it lists under _Avoid_.
- Architecture decisions live in [doc/adr/](doc/adr/). What is in and
  out of scope is in [doc/scope.md](doc/scope.md): don't build anything
  it lists as out of scope.

## 8. Language
- Code, comments, commit messages and contributor docs: English.
- Design notes and research in `doc/`: French.
