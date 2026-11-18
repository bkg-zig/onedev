# Changelog

All notable changes to the OneDev Rust Rewrite project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-11-18 - Branch `mango`

### Added
- **Phase 0**: Workspace foundation with 9 crates (`onedev-core`, `onedev-db`, `onedev-auth`, `onedev-api`, `onedev-server`, `onedev-git`, `onedev-build`, `onedev-web`, `onedev-cli`)
- **Phase 1**: Domain models (Project, User, Issue, MergeRequest, Build, Git entities), SQLx PostgreSQL migrations, Shiro-based auth port with `implies()` privilege composition
- **Phase 2**: Axum REST API with handlers for Projects, Issues, MRs, Builds, Git, Auth, Admin; Stack Control APIs (`GET /api/stack/status`, `GET /api/stack/logs`); OpenAPI documentation
- **Phase 3**: Git operations via `gix` crate (Smart-HTTP, LFS, Hooks), ruxen gateway (nginx-style config), Docker Compose stack (core:46315, db:5432, gateway:36697)
- **Phase 4**: Server-side SSR templates (`onedev-web` with Tera), Client-side SPA (`onedev-frontend` with Dioxus 0.7), `dx build --web` working via separate frontend workspace
- **Phase 5**: youki OCI runtime integration, Build-Spec parser with Job Graph, Executor abstraction (Local, Docker, Kubernetes)

### Fixed
- **WASM Build Blocker**: `dx build --web` failed due to `mio` crate incompatibility with `wasm32-unknown-unknown` target. Resolved by:
  - Separating server-side SSR crate (`onedev-web`) from client-side SPA crate (`onedev-frontend`)
  - Creating dedicated `frontend-workspace` for WASM builds
  - Using `dioxus-fullstack` with `web` feature (no `server` feature) to avoid `tokio-tungstenite` → `mio` dependency chain
  - Removed `ws` feature from workspace `axum` dependency

### Changed
- Disabled `ws` feature in workspace `axum` dependency to prevent `mio` transitive dependency
- Updated `tokio-wasm` workspace dependency for WASM-compatible async runtime
- Quality gates enforced: `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`, `cargo test --workspace --quiet` all pass

### Infrastructure
- Docker images built: `bkg-core-admin-core:mango`, `bkg-core-admin-db:mango`, `bkg-core-ruxen:mango`
- Stack verified running and healthy: Core API (46315), PostgreSQL (5432), Gateway/ruxen (36697)
- Stack Control APIs operational: `/health`, `/api/stack/status`, `/api/stack/logs`
- Git forks clean: `pgrust` (clean), `ruxen` (untracked build artifacts only), `youki` (clean)

### Documentation
- Updated `docs/progress.md` with current status (Phase 6 hardening & Cutover)
- Added this `CHANGELOG.md`

---

## [Unreleased]

### Planned for 1-to-1 Port Completion
- Web-UI: Port 552 Vanilla HTML templates to Tera (SSR) + Dioxus (SPA)
- Build-Spec: Implement 158 parser rules, 16 job types, 66 executor plugins
- Search: Migrate ANTLR grammar to pest + Tantivy index
- API: Complete remaining 30+ endpoint groups
- Shiro Parity: All 40 Permission classes with full `implies()` composition

---

## Migration Notes

### Java → Rust Mapping Reference
- Vanilla OneDev: `vendor/vanilla-onedev/server-core/src/main/java/io/onedev/server/...`
- Rust Crates: `crates/onedev-*/src/`
- Shiro SecurityUtils → `onedev-auth/src/shiro/`
- Wicket Components → `onedev-web/src/templates/` (SSR) + `onedev-frontend/src/` (SPA)
- Build Pipeline/Stage/Runner → Build-Spec/Job/Trigger (OneDev model)

### Quality Gates (Must Pass)
```bash
cargo fmt --check
cargo clippy --workspace -- -D warnings
cargo test --workspace --quiet
dx build --web --package onedev-frontend  # in frontend-workspace/
docker compose -f docker/compose.yml config --quiet
```

---

*Generated 2026-11-18 – Branch `mango`*