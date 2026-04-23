# Agent Notes

## Build
- `nix flake check -L` is the authoritative local-first CI entrypoint. It runs build, tests, rustfmt, strict clippy, and tigerstyle.
- For ad-hoc local Rust commands outside the flake, use `nix shell nixpkgs#cargo nixpkgs#rustc nixpkgs#gcc`; omitting `gcc` causes link failures during `cargo test`.

## Lint Policy
- `src/lib.rs` is the crate-level lint policy surface: `missing_docs`, `clippy::all`, `clippy::pedantic`, and `clippy::undocumented_unsafe_blocks` are all denied.
- `dylint.toml` denies every tigerstyle lint. `Cargo.toml` includes `[workspace.metadata.tigerstyle]` so the single-crate repo works with `tigerstyle.lib.mkConsumerCheck`.

## Nix
- `flake.nix` uses `crane`, `rust-overlay`, and the `git+ssh://git@github.com/brittonr/tigerstyle-rs.git` input. New checks should be added under `checks.<system>` so `nix flake check` stays the one command that matters.
