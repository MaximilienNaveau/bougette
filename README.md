# Bougette

> Early stage: a command-line client, a server, and a first Linux app;
> shared budgets are encrypted end to end.

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

The domain and its storage work behind a command-line client (milestone
M0 of [doc/scope.md](doc/scope.md)):

```sh
nix run . -- help
nix run . -- budget new Household
nix run . -- line add Household Rent Housing cost 900 monthly 2026-01-05
nix run . -- tx add Household 2026-11-05 cost 950 Housing --line Rent
nix run . -- show Household
```

Members share budgets through a server that keeps their changes,
encrypted end to end: it can't read the budgets (milestones M1 and M2,
[ADR 0005](doc/adr/0005-chiffrement-signatures-et-acces.md)). Devices talk
over [iroh](https://www.iroh.computer/):

```sh
# The server's operator lets Alice in.
nix run .#server -- --db server.sqlite invite alice      # prints TOKEN
nix run .#server -- --db server.sqlite serve --port 47210  # prints ADDRESS
# Alice joins, creates a budget, invites Bob, sets up her recovery phrase.
nix run . -- server join ADDRESS TOKEN alice
nix run . -- budget new Household
nix run . -- invite Household                            # prints INVITATION
nix run . -- recovery setup
# Bob asks to join; Alice's device lets him in at its next sync.
nix run . -- accept INVITATION --username bob
nix run . -- sync
```

The Linux app (milestone M3, [app/](app/README.md)) shows the budgets, a
calendar coloured by overspending and the detail of each month, from the
same database as the command-line client:

```sh
nix run .#app
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
