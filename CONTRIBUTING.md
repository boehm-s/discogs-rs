# Contributing

## Tooling

- Rust toolchain: pinned in `rust-toolchain.toml` (installed automatically by `rustup`)
- [`just`](https://github.com/casey/just): `just` lists the available recipes
- [`cargo-nextest`](https://nexte.st), [`cargo-deny`](https://github.com/EmbarkStudios/cargo-deny),
  [`cargo-semver-checks`](https://github.com/obi1kenobi/cargo-semver-checks)

```sh
cargo install --locked cargo-nextest cargo-deny cargo-semver-checks
```

`just ci` runs locally what the CI runs on a pull request.

## Tests

- `just test`: unit tests and doctests. They never call the Discogs API: requests go to a
  [`wiremock`](https://docs.rs/wiremock) server answering with real Discogs responses stored in
  `resources/` (personal data anonymized). Tests sit at the bottom of the file they test.
- `just test-live`: tests against the real Discogs API, behind the `live-tests` feature. They
  need `DISCOGS_TOKEN` in `.env` and never run in the CI.

## Branches and release flow


```
feature branch ──PR──▶ integration ──(CI, automatic)──▶ main ──(CI)──▶ crates.io
```

| Event | Jobs |
|---|---|
| Pull request → `integration` | fmt, clippy, rustdoc, cargo-deny, tests, MSRV, `cargo package`, semver checks |
| Push on `integration` (merged PR) | same checks, then **promote**: fast-forwards `main` to the tested commit |
| Push on `main` | **release**: if the `Cargo.toml` version is not on crates.io yet, `cargo publish`, tag `vX.Y.Z`, GitHub release |

- `main` is protected: nobody can push or merge into it, only the CI promote job (deploy key).
- `integration` is protected: changes only through pull requests with green checks.
- Releasing = bumping `version` in `Cargo.toml` (and `CHANGELOG.md`) in a pull request.
  The semver job fails if the public API changes without the matching bump.
  A merge without a version bump updates `main` but publishes nothing.
