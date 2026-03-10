# AGENTS.md
Guide for autonomous coding agents working in `memo-rs`.

## Repository Shape
- Rust workspace (edition 2024) with crates: `api`, `website`, `db`, `memo`, `password`, `storage`.
- `api`: JSON API service (Axum).
- `website`: server-rendered web app (Axum + Askama).
- `db`: persistence layer and repositories.
- `memo`: shared domain models/validators.
- `password`: Argon2 hashing/verification.
- `storage`: cloud storage abstraction + test client feature.
- Frontend assets live in `website/frontend` (Node scripts + Biome).

## Rule Files (Cursor / Copilot)
- No `.cursorrules` file found.
- No `.cursor/rules/` directory found.
- No `.github/copilot-instructions.md` found.
- If any are added later, treat them as higher-priority instructions.

## Build Commands
### Workspace / Rust
- `cargo build --workspace`
- `cargo build --workspace --release`
- `cargo build -p api`
- `cargo build -p website`

### Run Services
- API: `cargo run -p api -- --config api/config-example.toml server`
- Website: `cargo run -p website -- --config website/config-example.toml`
- Root README also shows: `cargo run -- server`

### Frontend (`website/frontend`)
- Install dependencies: `npm install`
- Build bundles: `npm run build`
- Format frontend assets: `npm run format`

## Lint / Format Commands
### Rust
- Format: `cargo fmt --all`
- Check format only: `cargo fmt --all -- --check`
- Clippy baseline: `cargo clippy --workspace --all-targets -- -D warnings`

### Frontend
- Biome write mode: `npm run format`

## Test Commands
### Run All Tests
- Workspace: `cargo test --workspace`
- Per crate:
  - `cargo test -p api`
  - `cargo test -p website`
  - `cargo test -p db`
  - `cargo test -p memo`
  - `cargo test -p password`
  - `cargo test -p storage`

### Run a Single Test (Important)
- By exact name in crate:
  - `cargo test -p api test_home_page -- --exact --nocapture`
  - `cargo test -p password test_verify_password -- --exact`
- If unsure of name, list first:
  - `cargo test -p api -- --list`
  - `cargo test -p website -- --list`
- Async integration tests (API) live in `api/src/web/server.rs` and use `#[tokio::test]`.

### Test Features / Notes
- `db` has test doubles via feature flag:
  - `cargo test -p db --features test`
- `storage` has test doubles via feature flag:
  - `cargo test -p storage --features test`
- Most crate-level tests are inline `mod tests` blocks near implementation modules.

## Coding Style Guidelines
These conventions are already present in the codebase; follow them for new code.

### Formatting and Whitespace
- Respect `.editorconfig`: LF endings + final newline.
- Rust: 4-space indentation.
- Frontend (Biome): 2-space indentation, single quotes, semicolons, trailing commas.

### Imports
- Keep imports at top of file.
- Prefer grouped ordering: std, third-party, internal crates/modules.
- Prefer explicit imports over glob imports.
- Import SNAFU selectors explicitly when using `.context(...)` / `.fail()` (e.g., `ConfigParseSnafu`).
- Let formatter/tools organize imports where configured.

### Naming
- Types/enums/traits: `PascalCase`.
- Functions/modules/files: `snake_case`.
- Constants/statics: `SCREAMING_SNAKE_CASE`.
- Error variants are short domain nouns/phrases (`ConfigParse`, `InvalidAuthToken`, `DbQuery`).
- DTO/payload suffixes are common (`ClientDto`, `ActorPayload`).

### Types and Data Modeling
- Keep shared domain objects in `memo` crate.
- Use `serde` derive on API/web payload structs/enums.
- Use `PathBuf` for filesystem/config paths.
- Prefer explicit return types on public functions.
- Keep crate-local result alias pattern:
  - `pub type Result<T> = std::result::Result<T, Error>;`

### Error Handling
- Use `snafu` for typed error enums and context propagation.
- Prefer typed variants instead of ad-hoc string errors.
- Include `source` and `backtrace` fields on I/O, network, storage, and DB boundaries.
- Use `ensure!` for validation guards.
- Convert app errors to HTTP statuses centrally (`From<&Error> for StatusCode`, `IntoResponse`, or response-mapper middleware).
- `unwrap`/`expect` are common in tests and strict framework invariants; avoid introducing new uses in normal business logic.

### Async and Services
- Use `tokio` runtime idioms (`#[tokio::main]`, `#[tokio::test]`).
- Keep async boundaries at web/service/repository edges.
- Follow existing graceful shutdown + middleware layering patterns in server modules.

### API / Web Patterns
- Router composition commonly uses `.merge(...)`.
- Middleware often handles cross-cutting concerns (trace/auth/response mapping).
- Prefer returning crate `Result<T>` from handlers/services, then map at boundary.
- Keep auth and authorization checks explicit near handler entry points.

### Database Patterns
- `db` exposes trait-based stores (`*Store`) and implementations (`*Repo`).
- `DbMapper` aggregates store traits behind `Arc<dyn Trait>`.
- Keep DB-side max constraints and validation consistent with existing rules.

### Frontend Patterns
- Frontend is script/CSS bundling, not a SPA framework.
- Bundles are generated by scripts in `website/frontend/scripts`.
- Preserve naming convention tied to `bundles.json` suffix.
- Run `npm run format` after editing frontend JS/CSS/JSON.

## Practical Agent Workflow
- Read crate-local `README.md` and config examples before changing runtime behavior.
- Make minimal, crate-scoped edits.
- Run targeted checks first, then broader checks.
- Before finalizing Rust changes:
  - `cargo fmt --all -- --check`
  - `cargo test -p <changed-crate>`
- If frontend changed (in `website/frontend`):
  - `npm run format`
  - `npm run build`

## Known Config Inputs
- API example config: `api/config-example.toml`
- Website example config: `website/config-example.toml`
- Common required values: JWT secret, captcha keys, DB URL, cloud credentials.

## Scope and Safety
- Never commit secrets or credential files.
- If behavior changes, update docs and config examples.
- Preserve architecture boundaries (`memo` domain, `db` persistence, service/web layers).
