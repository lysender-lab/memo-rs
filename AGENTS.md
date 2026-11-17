# memo-rs Agent Guide

## Structure

- This is a Rust 2024 workspace: `api` (Axum JSON API), `website` (Axum + Askama server-rendered UI), `db` (Turso repositories), `memo` (shared domain models and validation), `storage` (S3 abstraction), `password` (Argon2), and `yaas` (auth actors, roles, and permissions).
- Keep shared domain types and validators in `memo`; keep persistence behind the trait-based stores in `db`.
- `api/src/main.rs` and `website/src/main.rs` are the service entrypoints. Their runtime settings come from `api/.env.example` and `website/.env.example`; neither application loads `.env` itself.
- Website templates are in `website/templates`; Vite source bundles are in `website/frontend/bundles`. Generated frontend assets and the manifest go in `website/frontend/public/assets/bundles`.

## Commands

- Format Rust: `cargo fmt --all`; verify formatting: `cargo fmt --all -- --check`.
- Lint all Rust targets: `cargo clippy --workspace --all-targets -- -D warnings`.
- Run a focused Rust test: `cargo test -p <crate> <test_name> -- --exact`; list names with `cargo test -p <crate> -- --list`.
- Run a crate's tests: `cargo test -p <crate>`; run the workspace: `cargo test --workspace`.
- Build website frontend assets from `website/frontend`: `npm ci` then `npm run build:assets`. This clears and regenerates `public/assets/bundles`, including its manifest.

## Conventions

- Use SNAFU typed errors and context propagation at I/O, storage, and database boundaries; HTTP error mapping remains centralized in each service's web layer.
- API routes compose in `api/src/web/routes.rs`; retain the auth, directory, and file middleware around protected route changes.
- Preserve LF endings and final newlines. Rust uses four spaces; frontend formatting is governed by `website/frontend/biome.json` (two spaces, single quotes, semicolons, trailing commas).
