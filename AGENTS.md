# memo-rs Agent Guide

## Service boundaries

- `api/src/main.rs` and `website/src/main.rs` start separate Axum binaries. The website renders Askama templates in `website/templates` and calls the JSON API through `website/src/services/`. Keep shared DTOs and validators in `memo`, persistence behind `db` store traits, and auth actors and roles in `yaas`.
- Route wiring lives in each service's `src/web/routes.rs`. Keep protected routes behind the existing auth and directory/file/note middleware. Use SNAFU context at I/O, storage, and database boundaries; map errors to HTTP in the web layer.

## Local setup

- From the root, the service commands are `cargo run -p api` and `cargo run -p website`. Export settings from each service's `.env.example`; neither loads `.env` itself. For current settings, trust the examples and `src/config.rs` over the root README.
- The API uses embedded Turso at `$DATABASE_DIR/default/memo.db`. Apply numbered SQL migrations from `db/db/migrations/memo/` in order; the API does not migrate on startup. `JOB_WORKERS` defaults to `0`; set it above zero to exercise image processing and queued file deletion.
- Website startup requires `$FRONTEND_DIR/public/assets/bundles/.vite/manifest.json`. Point `FRONTEND_DIR` at `website/frontend` locally; from there run `npm ci` then `npm run build:assets` (CI uses Node 24). Vite clears the ignored `public/assets/bundles/` output. Edit the sources in `public/assets/js/` and `public/assets/css/` imported by `bundles/main.js` and `bundles/main.css`; see `biome.json` in the frontend directory.

## Verification

- Rust: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`. Compiling `api` targets requires `pkg-config` and the dav1d development library for native AVIF decoding (`libdav1d-dev` in CI).
- Scope tests with `cargo test -p <crate>`. Get full names with `cargo test -p <crate> -- --list`, then run one with `cargo test -p <crate> <full_test_name> -- --exact`.
- Repository tests using `db/src/test.rs::TestCtx` create isolated local databases (under `$DATABASE_DIR/test` if set, otherwise the system temp directory). The fixture applies an explicit `MIGRATIONS` list; update it when tests need a new migration.
