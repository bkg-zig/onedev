# OneDev Rust Rewrite – Project Rules

Dieses Dokument definiert die verbindlichen Regeln für den **kompletten Rust-Rewrite von OneDev** im Verzeichnis `/srv/bkg/services/bkg-core-admin/infra/onedev` auf Branch `mango`.

---

## 1. Repository-Struktur & Branch-Regeln

### 1.1 Arbeitsverzeichnis
- **Arbeitsverzeichnis:** `onedev` (Root des Rust-Projekts)
- **Branch:** `mango` (Custom-Änderungen, **niemals** Upstream-PRs)
- **Keine Arbeit außerhalb** dieses Verzeichnisses.

### 1.2 Branch-Strategie (gemäß PR-Regeln)
| Branch | Zweck |
|--------|-------|
| `main` | Spiegel des Upstream-Forks (Read-only für uns) |
| `tango` | Upstream-PRs (Bugfixes, Features für Original) |
| `mango` | **Nur** BKG-Custom-Änderungen (dieser Rewrite) |

- **Nie** direkt auf `main` arbeiten.
- **Nie** Custom-Code in `tango` committen.
- **Nie** Upstream-Fixes in `mango` „verstecken“.
- Push **nur nach expliziter Freigabe**.

### 1.3 Verzeichnis-Layout (Rust Workspace)

```
/srv/bkg/services/bkg-core-admin/infra/onedev/
├── Cargo.toml                    # Workspace-Root
├── crates/                       # Alle Rust-Crates
│   ├── onedev-core/              # Domain Models, Shared Types
│   ├── onedev-db/                # Database Layer (SQLx, Migrations)
│   ├── onedev-auth/              # AuthN/AuthZ, Sessions, Tokens
│   ├── onedev-api/               # HTTP API (Axum, OpenAPI)
│   ├── onedev-server/            # Main Server Composition
│   ├── onedev-git/               # Git Operations (gix)
│   ├── onedev-build/             # CI/CD Pipeline, Executors
│   ├── onedev-web/               # Web UI (SSR, Templates)
│   └── onedev-cli/               # CLI Tools (admin, stack-control)
├── docker/                       # Dockerfiles, Compose
├── infra/config/                 # Config-Templates (ports.env, db.env)
├── scripts/                      # Build/Deploy/Scripts
├── vendor/                       # Read-only Referenzen
│   └──vanilla-onedev/         # Archiv alter Struktur
├── docs/                         # Dokumentation (dieses File, plan.md, progress.md, task.md)
└── .gitignore, .gitmodules, AGENTS.md
```

---

## 2. Rust-Entwicklungsstandards

### 2.1 Toolchain
- **Rust Version:** 1.89+ (in `Cargo.toml` als `rust-version` festgelegt)
- **Edition:** 2024
- **Resolver:** 2

### 2.2 Code-Qualität (verbindlich vor jedem Push)
```bash
cargo fmt --check --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --quiet --workspace
cargo build --release --workspace
```

### 2.3 Dependencies
- **Workspace-Dependencies** in `Cargo.toml` (Root) definieren, in Crates nur `workspace = true` referenzieren.
- Keine direkten Versionsangaben in Sub-Crates.
- Minimale, gut gewartete Crates bevorzugen.
- `unsafe` nur mit expliziter Begründung und Review.

### 2.4 Architektur-Prinzipien
| Prinzip | Umsetzung |
|---------|-----------|
| **Domain-Driven Design** | `onedev-core` = reine Domain-Logik, keine Framework-Abhängigkeiten |
| **Clean Architecture** | Inner Crates (`core`, `auth`) kennen Outer Crates nicht |
| **Dependency Inversion** | Traits in `core`, Impls in `db`, `git`, `build` |
| **Single Responsibility** | Jede Crate hat klar abgegrenztes Verantwortungsgebiet |
| **Type Safety** | Newtypes für IDs, `NonZero`, Validierung bei Construction |

### 2.5 Error Handling
- `thiserror` für Library-Errors (public API)
- `anyhow` / `eyre` für Application-Errors (binaries)
- **Keine** `unwrap()`/`expect()` in Production-Code (außer Tests)
- RFC 7807 Problem Details für HTTP-Errors

---

## 3. Domain-Modell-Regeln (`onedev-core`)

### 3.1 Entity-Design
- Jede Entity hat: `id: i64`, `uuid: Uuid`, `created_at`, `updated_at`, `deleted_at` (Soft-Delete)
- Projekt-scoped Nummerierung (`number: i64`) für Issues, MRs, Builds
- Enums mit `strum` für Serialisierung/Display
- Validierung via `validator` crate bei Input-Structs

### 3.2 Referenzen vs. Voll-Entitäten
- **Ref-Structs** (`UserRef`, `ProjectRef`, `IssueRef`) für Listen/API-Responses
- **Voll-Entitäten** nur für Detail-Views / Mutationen
- `From<&Entity> for EntityRef` implementieren

### 3.3 Permissions als Typen
- Jede Permission = eigenes Struct (Newtype-Pattern)
- `impl Permission for MyPermission { ... }`
- Bitflags für kombinierte Permissions

---

## 4. Database-Layer-Regeln (`onedev-db`)

### 4.1 SQLx
- **Compile-time checked queries** (`sqlx::query!`, `query_as!`)
- `offline` Feature für CI (Prepared Statements)
- Migrations in `crates/onedev-db/migrations/` (nummeriert `V1__...`, `V2__...`)

### 4.2 Repository Pattern
- Traits in `onedev-core` (z.B. `UserRepo`)
- Impls in `onedev-db` (z.B. `PostgresUserRepo`)
- Connection Pool via `deadpool-postgres`
- Transaktionen: `repo.transaction(|tx| ...)`

### 4.3 Soft Delete & Audit
- `deleted_at` Timestamp, **kein** Hard Delete
- `EntityId` Newtype für Type-Safety
- Optimistic Locking via `version` Column wo nötig

---

## 5. Auth & Security (`onedev-auth`)

### 5.1 Subject Model
- `Subject` = `UserPrincipal` | `AccessTokenPrincipal` | `Anonymous`
- Principal-Format: `u:<user_id>`, `a:<token_id>`, `anonymous`
- Thread-local via `tokio::task_local!` (nicht `ThreadLocal`)

### 5.2 Passwords & Tokens
- **Argon2id** (Parameter: m=19456, t=2, p=1)
- AccessTokens: JWT (RS256) mit kurzer TTL + Refresh-Token Rotation
- 2FA: TOTP (RFC 6238), Backup-Codes (bcrypt-gehasht)
- Secrets **niemals** in Logs, ENV, Config-Files, API-Responses

### 5.3 Permission Checks
- Middleware: `require_permission!(permission)` Extractor
- `AuthorizationInfoCache` für Project-Permissions (TTL 5min)
- Default-Roles via `BaseAuthorizationService`

---

## 6. HTTP API (`onedev-api`)

### 6.1 Framework
- **Axum** mit Tower Middleware Stack
- OpenAPI 3.1 via `utoipa` (Code-First)
- Swagger UI unter `/api/docs`

### 6.2 REST-Konventionen
| Resource | Collection | Item |
|----------|------------|------|
| List | `GET /api/{resource}` | `GET /api/{resource}/{id}` |
| Create | `POST /api/{resource}` | — |
| Update | — | `PATCH /api/{resource}/{id}` |
| Delete | — | `DELETE /api/{resource}/{id}` |
| Actions | `POST /api/{resource}/{id}/{action}` | |

- Cursor-based Pagination (`after`, `before`, `limit`)
- ETags für Conditional Requests
- Problem Details (RFC 7807) für Errors

### 6.3 Git Smart HTTP
- `/api/git/{project}/info/refs?service=git-upload-pack`
- `/api/git/{project}/git-upload-pack` (POST)
- `/api/git/{project}/git-receive-pack` (POST)
- LFS: `/api/git/{project}/info/lfs/objects/*`

---

## 7. Git Operations (`onedev-git`)

### 7.1 Library
- **gix** (pure Rust, kein libgit2)
- Async-First API
- SSH via `gix-transport` (rustls-tls)

### 7.2 Repository Storage
- Bare Repos unter `/var/opt/onedev/git/{project_path}.git`
- Object-Pool für Forks (Alternates)
- GC via `gix::Repository::gc()` (scheduled)

### 7.3 Hooks
- Pre-Receive: Branch/Tag Protection, Permission Check
- Post-Receive: Build Trigger, Notifications, Mirror Push
- Hooks als Rust-Funktionen, **keine** Shell-Scripts

---

## 8. Build System (`onedev-build`)

### 8.1 Pipeline Spec
- YAML → `BuildSpec` AST (serde_yaml + Validierung)
- Job Graph mit Dependencies, Matrix, Conditions
- Executor Trait: `Local`, `Docker`, `Kubernetes`, `Sandbox`, `RemoteHost`

### 8.2 Executors
- **Kein Docker-Daemon** für neue Services (youki/OCI)
- `bollard` nur für Legacy Docker-Executor
- OCI Bundle Builder: Layer → rootfs, `config.json`, Namespaces/Cgroups

### 8.3 Cache & Artifacts
- Content-Addressable Storage (SHA256/Blake3)
- S3-compat oder lokaler Dateisystem-Backend
- TTL & Quota Enforcement

---

## 9. Web UI (`onedev-web`)

### 9.1 Tech Stack
- **Tera** Templates (SSR) + **Axum** Handler
- **dx** Build Tool für Assets (Tailwind, esbuild)
- Progressive Enhancement, **kein** SPA-Framework-Zwang

### 9.2 Stack-Tab (Admin)
- Container-Listing via `docker compose ps --format json`
- Actions: `up`, `down`, `restart` (Body: `{confirm: true}`)
- Logs: `docker compose logs --follow` mit Secret-Redaktion

---

## 10. Infrastructure & Deployment

### 10.1 Compose Stack (`bkg-core-admin`)
```yaml
services:
  db:      # pgrust (Postgres 18 + pgvector)
  core:    # onedev-server binary
  gateway: # ruxen (Profil: gateway, nicht live)
```

### 10.2 Ports (fest in `docker/ports.env`)
- `BKG_CORE_PORT=46315` (extern)
- `BKG_GATEWAY_PORT=36697` (extern)
- `CORE_INTERNAL_PORT=18080` (intern)

### 10.3 Secrets
- DB-PW: `/srv/bkg/admin-state/db.env` (0700/0600)
- Admin-PW: Salt+SHA256 in `control.admin_credentials`
- **Keine** Secrets in Image, Repo, Logs, argv

### 10.4 Observability
- OpenTelemetry (OTLP) → Collector
- Prometheus Metrics unter `/metrics`
- Health: `GET /health` (Liveness), `GET /ready` (Readiness)
- Structured JSON Logs (`tracing-json`)

---

## 11. Testing

| Ebene | Tool | Ziel |
|-------|------|------|
| Unit | `cargo test` | Domain-Logik, Pure Functions |
| Integration | `testcontainers` (Postgres) | Repo-Impls, Migrations |
| Contract | `wiremock` | Externe APIs (OAuth, Webhooks) |
| E2E | `cargo test --test e2e` | API-Flows, Git-HTTP, Builds |
| Property | `proptest` | Serialisierung, Parsing, Validation |

- `cargo test --quiet` muss **grün** sein vor jedem Push
- Test-DB via `testcontainers` (efimer, parallel)

---

## 12. Dokumentation

- **Rustdoc** für alle public APIs (`cargo doc --no-deps`)
- **ADR** (Architecture Decision Records) in `docs/adr/`
- **OpenAPI** Spec automatisch aus Code
- **Changelog** in `docs/progress.md` (Changelog-Format)
- **Task Board** in `docs/task.md` (Checkboxen)
- **Plan** in `docs/plan.md`

---

## 13. CI/CD (Buildspec)

- `.onedev-buildspec.yml` für OneDev-eigene CI
- Stages: `fmt` → `clippy` → `test` → `build` → `docker` → `deploy-staging`
- Nur `mango` Branch triggert Pipeline
- Artefakte: Binary, Docker Image, SBOM

---

## 14. Non-Goals (Explizit ausgeschlossen)

- ❌ Migration von `bkg-supafleet` / `bkg-dockgott`
- ❌ Host-weites Docker-Daemon-Setup
- ❌ Änderungen an `/etc/hosts`, Port 80/443, fremde Nginx/ruxen-Configs
- ❌ Java-Code kompilieren oder ausführen
- ❌ Upstream-Änderungen in `mango`

---

## 15. Definition of Done (Gesamtes Projekt)

- [ ] Alle 9 Crates kompilieren (`cargo build --release --workspace`)
- [ ] `cargo fmt --check --all` clean
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` clean
- [ ] `cargo test --quiet --workspace` grün
- [ ] `dx build --web` erfolgreich
- [ ] `docker compose -f docker-compose.yml config --quiet` ok
- [ ] Stack restart → `/health` 200, `/ready` 200
- [ ] `/api/stack/status` mit Session 200, Container gelistet
- [ ] Dashboard Stack-Tab: Buttons funktionieren, Logs follow
- [ ] `admin logs --follow` läuft, **keine Secrets** in Ausgabe
- [ ] Git-Status Forks (`pgrust`, `ruxen`, `youki`): **clean**
- [ ] `docs/progress.md`, `CHANGELOG.md` aktuell

---

*Letzte Aktualisierung: 2026-11-17 – Branch `mango` – Gilt nur für `/srv/bkg/services/bkg-core-admin/infra/onedev`*