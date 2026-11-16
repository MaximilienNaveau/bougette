# Bougette

> Early stage: a command-line client on one computer, no sync and no
> graphical app yet.

*Bougette* is the Old French word for a small leather purse, and the
origin of the English word *budget*.

A local-first, open-source app to manage **several budgets at once**: your
own, shared ones (a household, flatmates, a group of friends) and the
savings and projects that sit between them. Each person keeps all their
data on their own devices, works offline, and syncs with the people they
share a budget with.

Targets: Android, Linux and Windows first; macOS, iOS and the web if they
come cheaply. Distributed through F-Droid and nixpkgs, built reproducibly
from source.

## Status

The domain and its storage work, behind a command-line client (milestone
M0 of [doc/scope.md](doc/scope.md)):

```sh
nix run . -- help
nix run . -- budget new Household
nix run . -- line add Household Rent Housing cost 900 monthly 2026-01-05
nix run . -- tx add Household 2026-11-05 cost 950 Housing --line Rent
nix run . -- show Household
```

- [doc/scope.md](doc/scope.md): what v1 does and the milestones (French).
- [GLOSSARY.md](GLOSSARY.md): the domain's words.
- [doc/README.md](doc/README.md): the initial idea (French).
- [doc/research/](doc/research/): technology research (French): UI
  framework, local database, sync, encryption, distribution.

## Development

Everything is pinned and built by the repository's Nix flake:

```sh
nix develop        # a shell with the Rust toolchain
nix flake check -L # everything CI runs
```

See [CONTRIBUTING.md](CONTRIBUTING.md) to report a bug, suggest an
improvement or send a change.

## License

Copyright (c) 2026 Maximilien Naveau and contributors.

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.
