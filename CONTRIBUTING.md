# Contributing

Thanks for helping. Bug reports and ideas from people who actually use the
app are the most useful contributions of all, so you don't need to write
code to help.

## Reporting a bug or suggesting an improvement

Open an issue and pick the matching template. For a bug, say what you did,
what you expected and what happened instead, plus your platform and the
app version. **Never paste real financial data**: replace amounts, names
and accounts with made-up ones.

## Building

Everything (toolchains, dependencies, checks) is pinned by the Nix flake,
so your build matches CI and the F-Droid build:

```sh
nix develop        # shell with the pinned toolchain
nix flake check -L # formatting, lint, tests, licences: what CI runs
```

## Sending a change

1. One logical change per commit, with a short, imperative message
   ("Add the monthly view", not "Changes").
2. `nix flake check -L` passes.
3. Sign off every commit (see below).
4. Open a pull request against `main`.

### Dependencies

The app ships on F-Droid and in nixpkgs, and is licensed under
`MIT OR Apache-2.0`. A new dependency must:

- be open source under a permissive licence (MIT, Apache-2.0, BSD, ISC,
  Zlib, Unicode…). No GPL, LGPL or AGPL in the app; ask first for MPL;
- build from source: no prebuilt binaries downloaded at build time;
- not pull in Google Play Services, Firebase, analytics or tracking.

`nix flake check` checks the licences and sources of every crate against
[deny.toml](deny.toml); CI also checks them for known vulnerabilities
(`cargo deny check advisories`). A licence missing from `deny.toml` fails
the check: add it only if it fits the rules above.

## Developer Certificate of Origin (DCO)

There is no contributor licence agreement. Instead, each commit carries a
`Signed-off-by` line certifying that you wrote the change, or otherwise
have the right to submit it under the project's licence, as stated in the
[Developer Certificate of Origin 1.1](https://developercertificate.org/).
`git commit -s` adds it for you:

```
Signed-off-by: Your Name <you@example.org>
```

Your contribution is licensed under `MIT OR Apache-2.0`, like the rest of
the project, and you keep the copyright on it.

## Code of conduct

Be kind and assume good faith. See [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).
