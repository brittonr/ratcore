# Napkin

## Corrections
| Date | Source | What Went Wrong | What To Do Instead |
|------|--------|----------------|-------------------|

## User Preferences
- Enforce lints as hard errors; do not leave Clippy or lint policy as advisory warnings.

## Patterns That Work
- Use `nix flake check` as the integration point for repository policy rails.
- In git repos, `nix flake lock` needs `flake.nix` tracked first; `git add flake.nix` before locking or checking a new flake.

## Patterns That Don't Work
- Running `cargo test` in a bare `nix shell` without `gcc` fails at link time; include `nixpkgs#gcc` in the shell.

## Domain Notes
- (fill in as we learn the repo)
