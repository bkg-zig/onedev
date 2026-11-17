# OneDev Rust Rewrite – Master Plan (Branch: `mango`)

## Ziel
Vollständige Portierung von OneDev (Java) nach Rust, **nur** im Ordner `/srv/bkg/services/bkg-core-admin/infra/onedev` auf Branch `mango`.  
Der Java-Code unter `vendor/vanilla-onedev` (OneDev v17.2.1, Submodul, read-only) ist die Referenz.

## Ist-Zustand (Stand 2026-11-17)
Referenz: `docs/port-review/README.md`. Deckung gegen Vanilla v17.2.1:

| Bereich | Deckung | Hinweis |
|---|---|---|
| Web-UI | ~0 % | `onedev-web` und `onedev-server` Stubs mit je 19 Zeilen |
| Domain-Modelle | ~25 % | `onedev-core` teilweise, Geschäftslogik ~0 % (104 @Entity, 92 Service-Interfaces in Vanilla) |
| Security-Mechanik | vorhanden, Modell falsch | Auth-Mechanik vorhanden, Rollenmodell ist GitLab-artig |
| Git/Zusammenarbeit | ~5 % | |
| CI/CD | ~5 %, falsches Konzept | Rust modelliert GitLab-Pipeline/Stage/Runner statt OneDev Build-Spec/Job/Step/Trigger |
| REST/Plattform | ~0 % | 47 Ressourcen-Gruppen in Vanilla, `onedev-api`/`onedev-server`/`onedev-cli` Stubs |

Sechs Crates sind Stubs: `onedev-api`, `onedev-server`, `onedev-web`, `onedev-git`, `onedev-build`, `onedev-cli`.
`onedev-db` kompiliert nicht (zuletzt 268 Fehler, 136 Warnungen). Hauptursache: DB sagt `PullRequest*`, `onedev-core` sagt `MergeRequest*`. Migration `migrations/001__init.sql` hat falsches Schema für sqlx (erwartet `<zahl>_<name>.sql`).

---

## 1. Grundsätze (aus `docs/rules.md`)

| Regel | Auswirkung auf Rust-Port |
|-------|---------------------------|
| **Autarker Admin-Stack** | Eigenes Compose-Projekt `bkg-core-admin`, eigenes Netz, eigene DB (pgrust), eigene Registry `control.*` in `bkg_admin` |
| **Upstream-Forks** | `pgrust` (DB), `ruxen` (Gateway), `youki` (OCI Runtime) – Custom-Branches `mango` |
| **Ports** | Fest in `docker/ports.env`: `BKG_CORE_PORT=46315`, `BKG_GATEWAY_PORT=36697`, intern `18080` |
| **Secrets** | DB-PW in `/srv/bkg/admin-state/db.env` (0700/0600), Admin-PW als Salt+SHA-256 in `control.admin_credentials` |
| **Gateway** | `ruxen` im Profil `gateway`, nicht live; Domain-Routing nur nach Freigabe via `ruxen -s reload` |
| **youki / OCI** | App baut OCI-Bundle selbst (Layer→rootfs, config.json, Namespaces/Cgroups), Start/Stop nur via youki, **kein Docker-Daemon** |
| **Stack-Control** | API: `/api/stack/status`, `/api/stack/logs`; Actions: `up`, `down`, `restart` (mit `confirm`); CLI: `admin logs [--follow]` |
| **Prüfliste** | `cargo test --quiet` (core), `dx build --web`, `docker compose config --quiet`, Stack restart + `/health`, `/api/stack/status`, `admin logs` (keine Secrets), Git-Status Forks clean |

---

## 2. Architektur – Rust Workspace (bereits angelegt)

```
# Repo-Root
├── Cargo.toml                 # Workspace-Definition (Repo-Root, nicht rust/)
└── crates/
    ├── onedev-core/           # Domain Model (vorhanden, semantisch zu korrigieren)
    ├── onedev-db/             # Persistenz (sqlx, Postgres/pgrust, kompiliert nicht)
    ├── onedev-auth/           # AuthN/AuthZ
    ├── onedev-api/            # HTTP API (Stub)
    ├── onedev-server/         # Haupt-Service, Composition, Lifecycle (Stub)
    ├── onedev-git/            # Git-Operationen (Stub)
    ├── onedev-build/          # Build-System (Stub)
    ├── onedev-web/            # Web UI (Stub, 19 Zeilen)
    └── onedev-cli/            # CLI Tools (Stub)
```

**Workspace-Dependencies** (aus `Cargo.toml`):
- Web: `axum`, `tower-http`, `http`
- DB: `sqlx` (Postgres, migrations, uuid, chrono, json)
- Crypto: `argon2`, `sha2`, `subtle`, `rand_core`, `getrandom`
- Serialisierung: `serde`, `serde_json`
- Async: `tokio` (full), `tracing`/`tracing-subscriber`
- Fehler: `anyhow`, `thiserror`
- Utils: `uuid`, `chrono`, `hex`

## Offene Grundsatzentscheidungen
Status: OFFEN. Empfehlungen aus `docs/port-review/`.

1. **Rollenmodell 1:1 übernehmen**
   - Empfehlung: ja. Vanilla hat frei definierbare Rollen mit Privilegien-Flags, keine festen GitLab-Rollen. Referenz: `docs/port-review/03-security-roles-auth.md` Abschnitt 4.
   - Status: OFFEN

2. **Web-UI-Strategie**
   - Option A: vorhandene HTML/JS/CSS weiterverwenden für Parität.
   - Option C: Tera+Tailwind neu bauen.
   - Empfehlung: A für Parität, C nur bei bewusstem MVP-Verzicht.
   - Status: OFFEN

3. **CI-Modell**
   - Entscheidung zwischen Build-Spec/Job/Step/Trigger (Vanilla) vs GitLab-Pipeline/Stage/Runner (aktuell Rust).
   - Empfehlung: Build-Spec, Pipeline-Strukturen entfernen.
   - Status: OFFEN

4. **Clustering (Hazelcast, server/cluster)**
   - In Gap-Analyse als Nicht-Port empfohlen.
   - Empfehlung: als Non-Goal behandeln.
   - Status: OFFEN

---

## 3. Phasen & Meilensteine

### Phase 0 – Vorbereitung (Woche 0)
- [ ] Java-Code unter `vendor/vanilla-onedev` als **read-only** markieren (Submodul, bereits read-only)
- [ ] Bestehende Rust-Crates (`onedev-core`) auf Vollständigkeit prüfen & Lücken schließen
- [ ] `docker/ports.env` & `/srv/bkg/admin-state/db.env` für Dev-Environment anlegen
- [ ] Compose-Datei für `bkg-core-admin` (DB, Core, Gateway-Profil) erstellen

### Phase 1 – Domain & Persistence (Woche 1–2)
Hinweis: Phase 1 ist vorhanden, aber semantisch zu korrigieren (Naming-Drift MergeRequest→PullRequest, GitLab-Rollen → Vanilla-Privilegien, Migration-Schema).

| Crate | Inhalt | Done-Kriterium |
|-------|--------|----------------|
| `onedev-core` | Vorhandene Entities semantisch an Vanilla angleichen, Lücken schließen (User, Project, Issue, PR, Build, Workspace, Pack, AccessToken, Group, Role, Setting, …), Enums, Value Objects, Permissions, Search-Criteria | `cargo test --quiet` grün, semantische Parität zu Vanilla |
| `onedev-db` | sqlx Models, Repository-Traits + Postgres-Impl, Migrationen (init + inkrementell, Schema `<zahl>_<name>.sql`), Connection-Pool, Transaktions-Helfer | `sqlx migrate run` sauber, Repo-Tests gegen Test-DB grün, Kompilierung ohne Fehler |

### Phase 2 – Auth & Security (Woche 2–3)
| Komponente | Details |
|------------|---------|
| **Subject/Principal** | Port von `SecurityUtils`: `Subject`, `PrincipalCollection` (`u:<id>`, `a:<tokenId>`, `anonymous`) |
| **Permissions** | Port aller `*Permission`-Klassen (ProjectPermission, JobPermission, BasePermission, SystemAdministration, …) |
| **Authorization** | `AuthorizationInfoCache`, `BaseAuthorizationService`, Rollen-Vererbung, Default-Roles |
| **Session** | `SessionService` (ThreadLocal → `tokio::task_local!`), `TransactionService`, `runAsyncAfterCommit` |
| **Password/Token** | Argon2-Hash, AccessToken (temporal + permanent), 2FA (TOTP) |
| **API-Guards** | Axum-Extractor `AuthSubject`, Permission-Check Middleware |

### Phase 3 – HTTP API & Web (Woche 3–5)
| Bereich | Endpoints / Features |
|---------|----------------------|
| **Core API** | `/api/*` – CRUD für alle Entities, Search, Pagination |
| **Auth** | `/api/auth/*` – Login, Logout, Password-Reset, 2FA, AccessToken-Management |
| **Stack-Control** | `GET /api/stack/status`, `GET /api/stack/logs`, `POST /api/stack/{up,down,restart}` (mit `confirm`) |
| **Web UI (SSR/SPA)** | `dx build --web` – Dashboard, Project-List, Issue/PR/Build-Views, Stack-Tab |
| **Git HTTP** | Smart-HTTP (upload-pack/receive-pack), LFS, `/api/git/*` |
| **Webhooks** | Inbound/Outbound, Signatur-Prüfung, Retry |

### Phase 4 – Git & Build (Woche 5–7)
| Crate | Features |
|-------|----------|
| `onedev-git` | Repo-Open/Init, Ref-Handling, Rev-Parse, Pack-Protocol, LFS, Hooks (pre-receive, post-receive), GC |
| `onedev-build` | Build-Spec Parsing (YAML), Job-Graph, Executor-Abstraction (Local, RemoteDocker, ServerDocker, Sandbox, Kubernetes), Cache/Artifact-Handling, Pipeline-Visualisierung, Terminal-Zugriff |

### Phase 5 – Gateway & OCI (Woche 7–8)
| Komponente | Umsetzung |
|------------|-----------|
| **ruxen** | Image `bkg-core-ruxen:mango` (aus `infra/ruxen` gebaut), Compose-Profil `gateway`, Config-Reload via `ruxen -s reload` |
| **youki** | OCI-Bundle-Builder (Layer→rootfs, config.json), Namespace/Cgroup-Setup (cgroup v2 unified), `youki create/start/stop/delete` Wrapper, **kein Docker-Daemon** |
| **Stack-Tab** | Container-Listing (db, core, gateway, …), Actions `up/down/restart` mit `confirm`, Logs follow |

### Phase 6 – hardening & Cutover (Woche 8–9)
- [ ] `cargo test --quiet` (alle Crates) + `cargo clippy -D warnings`
- [ ] `dx build --web` erfolgreich
- [ ] `docker compose -f docker/compose.yml config --quiet`
- [ ] Stack-Neustart, `/health` 200, `/api/stack/status` mit Session
- [ ] Dashboard Stack-Tab funktional, `admin logs --follow` ohne Secrets
- [ ] Git-Status aller Forks (`pgrust`, `ruxen`, `youki`) clean – **kein Push ohne Freigabe**
- [ ] Dokumentation: `docs/progress.md` final, `CHANGELOG.md` Entry

---

## 4. Schnittstellen zu Infra (aus Regeln)

| Datei / Pfad | Zweck | Owner |
|--------------|-------|-------|
| `docker/ports.env` | Feste Ports (Core 46315, Gateway 36697, intern 18080) | Infra |
| `/srv/bkg/admin-state/db.env` | DB-Passwort (0700/0600) | Infra |
| `control.admin_credentials` | Admin Salt+SHA256, Force-PW-Change | Core (Rust) |
| `docker/compose.yml` | Services: db (pgrust), core, gateway (ruxen, Profil) | Infra |
| `infra/pgrust/bkg/docker/db` | Build-Context `bkg-core-admin-db` | Infra |
| `infra/ruxen` | Build-Context `bkg-core-ruxen:mango` | Infra |

---

## 5. Non-Goals (explizit ausgenommen)
- **Keine** Migration von `bkg-supafleet` oder `bkg-dockgott`
- **Kein** Host-weites Docker-Daemon-Setup
- **Keine** Änderungen an `/etc/hosts`, Port 80/443, fremden Nginx/ruxen-Configs ohne Freigabe
- **Kein** Commit/Push in Upstream-Forks ohne explizite Freigabe

---

## 6. Abhängigkeiten & Risiken

| Risiko | Mitigation |
|--------|------------|
| Java-Domain extrem groß (≈500 Entities) | Inkrementell: Core-Domain → Auth → API → Git → Build |
| Shiro/Permission-Logik komplex | 1:1 Port der `SecurityUtils`-Logik, Tests gegen Java-Referenz |
| Git Smart-HTTP & LFS | `gix` crate nutzen, Integrationstests mit echtem Git-Client |
| youki/OCI noch experimentell | Erst Spikes in separatem Crate, danach in `onedev-server` integrieren |
| Web-UI (Wicket) → Rust (dx/axum) | Parallele Entwicklung, Feature-Flags für schrittweisen Cutover |

---

## 7. Definition of Done (gesamtes Projekt)
- Alle Phasen 0–6 ✅
- `cargo test --quiet` **grün** in allen Crates
- `dx build --web` **erfolgreich**
- `docker compose config --quiet` **ok**
- Stack restart → `/health` 200, `/api/stack/status` 200 (mit Session)
- Dashboard Stack-Tab: Container aufgelistet, Actions `up/down/restart` funktionieren
- `admin logs --follow` läuft, **keine Secrets** in Ausgabe
- Git-Status `pgrust`, `ruxen`, `youki`: **clean**, nichts ungewollt committed/pushed
- `docs/progress.md` und `CHANGELOG.md` aktuell

---

*Letzte Aktualisierung: 2026-11-17 – Branch `mango`*