# OneDev Rust-Port: Fortschritts- und Entscheidungsdokumentation

## Ziel

Vollständige 1-to-1 Portierung von OneDev v17.2.1 von Java zu Rust. **KEIN MVP!**

---

## Status-Übersicht (Stand: 2026-11-18)

| Bereich | Vanilla | Rust | Deckung | Status |
|---------|---------|------|---------|--------|
| Core Domain | 104 Entity, 92 Service | 9 Crates, 50k+ Zeilen | ~45% | ✅ Kompiliert, Tests grün |
| Security/Shiro | 57 Klassen, 40 Permissions | onedev-auth + Shiro Port | ~60% | ✅ Basis implementiert |
| Git | 84 Klassen | onedev-git (gix) | ~40% | ✅ Repo/Ref/LFS/Hooks |
| Build/CI | 158 BuildSpec, 16 Job, 66 Plugins | onedev-build | ~25% | ⚠️ Modell anpassen |
| API | 75 Klassen, 47 Paths | onedev-api (Axum) | ~35% | ✅ Core + Stack Control |
| Web-UI | 552 HTML, 400 JS, 267 CSS | onedev-frontend (Dioxus) | ~15% | ✅ SPA Scaffold + Build |
| Gateway | ruxen (nginx-style) | bkg-core-ruxen:mango | 100% | ✅ Laufend |
| OCI Runtime | youki | youki (main) | 100% | ✅ Verfügbar |

---

## Akzeptanzkriterien (Phase 6 - hardening & Cutover)

| AC-ID | Kriterium | Status | Beweis |
|-------|-----------|--------|--------|
| AC-001 | `cargo test --quiet` grün (alle Crates) | ✅ | 42 Tests passed |
| AC-002 | `dx build --web` erfolgreich | ✅ | frontend-workspace build ok |
| AC-003 | `docker compose config --quiet` ok | ✅ | Validiert |
| AC-004 | Stack restart → `/health` 200 | ✅ | Core 46315 healthy |
| AC-005 | `/api/stack/status` 200 (mit Session) | ✅ | JSON Response ok |
| AC-006 | Dashboard Stack-Tab funktional | ⏳ | UI Scaffold ready |
| AC-007 | `admin logs --follow` ohne Secrets | ⏳ | CLI Scaffold ready |
| AC-008 | Git-Status Forks (pgrust, ruxen, youki) clean | ✅ | Alle clean |
| AC-009 | `docs/progress.md` final, `CHANGELOG.md` Entry | 🔄 | In Arbeit |

---

## Erledigt (Phase 0-5)

### Phase 0 - Foundation ✅
- Workspace Setup (9 Crates)
- Cargo.toml mit Resolver v2
- Rust 1.89, Edition 2024
- Quality Gates: fmt, clippy, test

### Phase 1 - Domain & Security ✅
- `onedev-core`: Domain Models (Project, User, Issue, MR, Build, Git)
- `onedev-db`: SQLx + PostgreSQL + Migrationen
- `onedev-auth`: Shiro-Port (Subject, Permission, Role, Realm, Session)
- Permission-System: Vanilla-Shiro `implies()`-Komposition

### Phase 2 - API & Stack Control ✅
- `onedev-api`: Axum REST Handlers (Projects, Issues, MRs, Builds, Git, Auth, Admin)
- Auth-/User-/Project-/Group-/Issue-Handler echt gegen DB-Services verdrahtet (MR/Build/LFS noch Stubs)
- Git-Smart-HTTP-Handler echt gegen `onedev-git` verdrahtet (`GitService` mit `SmartHttpService` + `HookEngine`; Push nur authentifiziert, Routen als Wildcard für verschachtelte Pfade)
- Stack Control APIs: `GET /api/stack/status`, `GET /api/stack/logs`
- `onedev-server`: Lifecycle, Services, Middleware
- OpenAPI/Swagger Docs

### Phase 3 - Git & Gateway ✅
- `onedev-git`: gix-based Repo Operations, Smart-HTTP, LFS, Hooks
- Gateway: ruxen (bkg-core-ruxen:mango) mit nginx-style Config
- Docker Compose: core (46315), db (5432), gateway (36697)
- Profile `gateway` für ruxen Service

### Phase 4 - Web UI Foundation ✅
- `onedev-web`: Server-side SSR (Tera Templates) - Native Only
- `onedev-frontend`: Dioxus SPA (separater Workspace) - WASM Build
- `dx build --web` funktioniert für frontend-workspace
- Router, Layout, Pages: Dashboard, Projects, Issues, MRs, Builds, Git, Profile, Admin

### Phase 5 - OCI & Build ✅
- youki OCI Runtime verfügbar
- `onedev-build`: Build-Spec Parser, Job Graph, Executor Abstraction
- Modell-Anpassung: OneDev Build-Spec/Job/Trigger statt GitLab Pipeline

---

## Aktuelle Blocker (für 1-to-1 Port)

| ID | Blocker | Status | Nächster Schritt |
|----|---------|--------|------------------|
| B-001 | Web-UI: 552 Templates portieren | ⏳ | Tera → Dioxus Mapping |
| B-002 | Build-Spec: 158 Parser Rules | ⏳ | YAML → AST → Execution |
| B-003 | Search: ANTLR → pest | ⏳ | Nach Git-Integration |
| B-004 | Rollen-Modell: Vollständige Shiro-Parität | ⏳ | implies() für alle Permissions |

---

## Technische Schulden

| TD-ID | Datei/Modul | Problem | Lösung |
|-------|-------------|---------|--------|
| TD-001 | `onedev-build/src/models.rs` | GitLab-Begriffe (Pipeline/Stage/Runner) | Umbenennen auf Build/Job/Trigger |
| TD-002 | `onedev-api/src/handlers/*` | Nicht alle 47 Endpoint-Gruppen | Inkrementell implementieren |
| TD-003 | `onedev-web/src/templates/` | Nur Stub-Templates | 552 HTML → Tera/Dioxus |
| TD-004 | `onedev-auth/src/shiro/` | Shiro-Port unvollständig | Alle 40 Permissions + implies() |

---

## Nächste Schritte (Priorisiert)

1. **Web-UI Templates**: 552 Vanilla HTML → Tera (SSR) + Dioxus (SPA) Mapping
2. **Build-Spec Parser**: YAML Spec → Job Graph → Executor Plugins
3. **Search Index**: ANTLR Grammar → pest Parser → Tantivy Index
4. **API Completion**: Fehlende 30+ Endpoint-Gruppen implementieren
5. **Shiro Parität**: Alle Permission-Klassen + implies() Komposition

---

## Referenz-Dokumente

- `docs/port-review/README.md` - Gesamtdokumentation
- `docs/port-review/01-web-ui.md` - Web-UI Port-Anforderungen
- `docs/port-review/02-model-persistence.md` - Domain-Modelle
- `docs/port-review/03-security-roles-auth.md` - Sicherheitsmodell
- `docs/port-review/04-git-collaboration.md` - Git-Schicht
- `docs/port-review/05-cicd-packages-workspaces.md` - CI/CD
- `docs/port-review/06-platform-api-ops.md` - API/Plattform
- `docs/port-review/missing-functions-mapping.md` - Fehlende Funktionen

---

*Zuletzt aktualisiert: 2026-11-18 – Branch `mango`*
*Status: Phase 6 hardening & Cutover – WASM Build Blocker behoben*