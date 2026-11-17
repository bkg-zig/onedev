# OneDev Rust Rewrite – Progress Log (Branch `mango`)

> **Format**: Changelog-Stil, neueste Einträge oben.  
> **Branches**: Nur `mango` (Custom-Änderungen). Upstream-Fixes → `tango`.  
> **Kein Commit/Push ohne Freigabe**.

---

## [Unreleased] – 2026-11-17

### Analyse
- **Gap-Analyse gegen Vanilla v17.2.1** (`docs/port-review/`): sechs Bereichsberichte plus Übersicht. Kernbefunde: Web-UI, REST, Git-Server und CI sind in Rust Stubs; die vorhandenen Domain-Modelle folgen an mehreren Stellen GitLab statt OneDev (feste Rollen statt Privilegien-Modell, `MergeRequest`/`Pipeline`/`Runner` existieren in OneDev nicht). Drei offene Grundsatzentscheidungen blockieren die weitere Arbeit: Rollenmodell, UI-Strategie, CI-Modell. Details in `docs/port-review/README.md`.
- **`onedev-db` kompiliert nicht** (zuletzt 268 Fehler). Ein Fix-Lauf wurde durch ein API-Rate-Limit unterbrochen; bisher nur Teiländerungen in `migrations`, `pool`, `error`, `user`, `group`.
- **Build-Blocker behoben:** ungenutztes `simd-json` aus Workspace-`Cargo.toml` und `crates/onedev-api` entfernt (kompilierte nur mit SIMD-fähiger CPU).

## [Unreleased] – 2026-11-17 (früher)

### Added
- **Plan-Dokumentation** (`docs/plan.md`): Master-Plan mit 6 Phasen, Meilensteinen, Non-Goals, Risiken, DoD.
- **Progress-Log** (diese Datei): Changelog für Fortschritt.
- **Task-Liste** (`docs/task.md`): Granulare Tasks mit Checkboxen, Owner, Status.
- **Rust Workspace** (bereits vorhanden):
  - `rust/Cargo.toml` mit 6 Crates (`onedev-core`, `onedev-db`, `onedev-api`, `onedev-server`, `onedev-git`, `onedev-build`)
  - `onedev-core` mit Domain-Modellen: `auth.rs`, `build.rs`, `error.rs`, `issue.rs`, `lib.rs`, `pack.rs`, `project.rs`, `pull_request.rs`, `query.rs`, `user.rs`

### Infrastructure
- Branch `mango` aktiv (`git branch --show-current` → `mango`)
- **Java-Code nach `vendor/bkg-core-dev/` verschoben** (Referenz-Implementierung, read-only)
- `.gitmodules` angepasst: Submodule `server-ee` → `vendor/bkg-core-dev/server-ee`
- `docker/ports.env` & `/srv/bkg/admin-state/db.env` **noch nicht angelegt** (Phase 0)
- Compose `docker/compose.yml` für `bkg-core-admin` **noch nicht angelegt** (Phase 0)

### Repository State
- Root: nur Config-Files (`.gitignore`, `.gitmodules`, `AGENTS.md`, `.trivyignore`, `development.md`)
- `docs/` – Plan, Progress, Task Board
- `rust/` – Workspace mit 6 Crates (Skelett)
- `vendor/bkg-core-dev/` – Kompletter Java-Code (server-core, server-plugin, server-product, server-ee, pom.xml, .mvn, dev.sh, doc, e2e-test, license, readme)

### Domain (`onedev-core`) – Status
| Modul | Status | Nächste Schritte |
|-------|--------|------------------|
| `user.rs` | ✅ Basis (User, Type, Auth) | Permissions, 2FA, AccessToken komplettieren |
| `project.rs` | ✅ Basis (Project, Settings) | Branch/Tag Protection, Authorization |
| `issue.rs` | ✅ Basis (Issue, Field, Transition) | Search-Criteria, Work, Watch, Vote |
| `pull_request.rs` | ✅ Basis (PR, Review, Comment) | Merge-Strategien, Conflict-Detection |
| `build.rs` | ✅ Basis (Build, Job, Step) | Executor-Traits, Pipeline, Cache |
| `auth.rs` | 🟡 Grundgerüst | Shiro-Permission-Port, Subject/Principal, Session |
| `query.rs` | 🟡 Search-Traits | Code-Search, Entity-Search, Lucene-Alternative |
| `pack.rs` | 🟡 Grundgerüst | NuGet, PyPI, Maven, Docker, Generic |
| `error.rs` | ✅ Error-Types | Mapping zu HTTP-Status, Problem-Details |

### Tests
- `cargo test --quiet` **noch nicht ausgeführt** (Workspace unvollständig)

---

## Legend
- ✅ = Done / Implementiert & getestet
- 🟡 = In Progress / Partiell
- ⬜ = Todo / Nicht begonnen
- 🔴 = Blockiert / Risiko

---

*Letzte Aktualisierung: 2026-11-17*