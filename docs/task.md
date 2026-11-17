# OneDev Rust Rewrite – Task Board (Branch `mango`)

> **Legende**: `- [ ]` Todo, `- [~]` In Progress, `- [x]` Done  
> **Owner**: `core` = Rust-Core-Team, `infra` = Infra-Team, `sec` = Security  
> **Kein Commit/Push ohne Freigabe**.

---

## Phase 0 – Vorbereitung

| # | Task | Owner | Status | Notizen |
|---|------|-------|--------|---------|
| 0.1 | Java-Code nach `vendor/bkg-core-dev/` verschoben (Referenz, read-only) | infra | - [x] | Erledigt: `server-core`, `server-plugin`, `server-product`, `server-ee`, `pom.xml`, `.mvn`, `dev.sh`, `doc`, `e2e-test` → `vendor/bkg-core-dev/` |
| 0.2 | `vendor/bkg-core-dev` formal read-only setzen (`git update-index --skip-worktree` für getrackte Dateien) | infra | - [ ] | Vor Portierung sicherstellen |
| 0.3 | `docker/ports.env` anlegen (`BKG_CORE_PORT=46315`, `BKG_GATEWAY_PORT=36697`, `CORE_INTERNAL_PORT=18080`) | infra | - [ ] | Einmalig, danach nie ändern |
| 0.4 | `/srv/bkg/admin-state/db.env` anlegen (0700/0600), DB-PW generieren | infra | - [ ] | Nur DB-PW, keine anderen Secrets |
| 0.5 | `docker/compose.yml` für `bkg-core-admin` erstellen (Services: `db` → `bkg-core-admin-db`, `core`, `gateway` Profil → `bkg-core-ruxen:mango`) | infra | - [ ] | Eigenes Netz `bkg-core-admin-net` |
| 0.6 | `onedev-core` Domain-Modelle auf Vollständigkeit prüfen vs. Java-Entities (≈500) | core | - [~] | `user.rs`, `project.rs`, `issue.rs`, `pull_request.rs`, `build.rs` vorhanden; `auth.rs`, `query.rs`, `pack.rs` partiell |
| 0.7 | `cargo test --quiet` im Workspace laufen lassen (Baseline) | core | - [ ] | Erwartet: Fehlschlagen bis Crates komplett |

---

## Phase 1 – Domain & Persistence (`onedev-core`, `onedev-db`)

### 1.1 `onedev-core` – Missing Domain Models
| # | Entity / Modul | Java-Referenz | Status |
|---|----------------|---------------|--------|
| 1.1.1 | `Group`, `GroupAuthorization` | `Group.java`, `GroupAuthorization.java` | - [ ] |
| 1.1.2 | `Role`, `BaseAuthorization` | `Role.java`, `BaseAuthorization.java` | - [ ] |
| 1.1.3 | `Setting`, `SecuritySetting`, `SystemSetting` | `Setting.java`, `SecuritySetting.java` | - [ ] |
| 1.1.4 | `AccessToken`, `AccessTokenAuthorization` | `AccessToken.java`, `AccessTokenAuthorization.java` | - [ ] |
| 1.1.5 | `Workspace`, `WorkspaceSpec`, `UserData` | `Workspace.java`, `WorkspaceSpec.java` | - [ ] |
| 1.1.6 | `Pack`, `PackBlob`, `PackLabel` (NuGet, PyPI, Maven, Docker, Generic) | `Pack.java`, `PackBlob.java` | - [ ] |
| 1.1.7 | `BuildSpec`, `Job`, `Step`, `ExecutorSpec` (Local, RemoteDocker, ServerDocker, Sandbox, K8s) | `BuildSpec.java`, `JobExecutor.java` | - [ ] |
| 1.1.8 | `IssueField`, `FieldSpec`, `ChoiceField`, `UserChoiceField`, `TransitionSpec` | `IssueField.java`, `FieldSpec.java` | - [ ] |
| 1.1.9 | `PullRequestChange`, `PullRequestReview`, `PullRequestAssignment` | `PullRequestChange.java`, `PullRequestReview.java` | - [ ] |
| 1.1.10 | `CodeComment`, `CodeCommentReply`, `CompareContext` | `CodeComment.java`, `CodeCommentReply.java` | - [ ] |
| 1.1.11 | `Dashboard`, `Widget`, `QueryPersonalization` | `Dashboard.java`, `Widget.java` | - [ ] |
| 1.1.12 | `EmailAddress`, `EmailAddressVerification` | `EmailAddress.java` | - [ ] |
| 1.1.13 | `SshKey`, `GpgKey` | `SshKey.java`, `GpgKey.java` | - [ ] |
| 1.1.14 | `OAuthApplication`, `OAuthGrant`, `SsoProvider`, `SsoAccount` | `OAuthApplication.java`, `SsoProvider.java` | - [ ] |
| 1.1.15 | `Audit`, `Alert`, `Stopwatch`, `IssueWork`, `IssueVote`, `IssueWatch`, `PullRequestWatch` | diverse | - [ ] |

### 1.2 `onedev-core` – Permissions & Search
| # | Task | Status |
|---|------|--------|
| 1.2.1 | Alle `*Permission` Klassen portieren (`ProjectPermission`, `JobPermission`, `BasePermission`, `SystemAdministration`, `UserAdministration`, `CreateRootProjects`, `MaintainProject`, `ManageProject`, `ManageIssues`, `ManageBuilds`, `ManagePullRequests`, `ManageWorkspaces`, `ManageCodeComments`, `ManageJob`, `ReadCode`, `WriteCode`, `ReadPack`, `WritePack`, `AccessBuildLog`, `AccessBuildPipeline`, `AccessBuildReports`, `AccessConfidentialIssues`, `AccessTimeTracking`, `AccessSites`, `RunJob`, `UploadCache`, `ScheduleIssues`, `CreateChildren`, `CreateWorkspaces`, `EditIssueField`, `EditFieldsOfOtherIssues`, `EditIssueLink`, `ConfidentialIssuePermission`, `ConfidentialIssuesPermission`) | - [ ] |
| 1.2.2 | `AuthorizationInfoCache` Port (ThreadLocal → `tokio::task_local!`) | - [ ] |
| 1.2.3 | Search-Criteria Traits & Implementierungen (`EntityCriteria`, `FieldCriteria`, `FuzzyCriteria`, `ProjectCriteria`, `PersonCriteria`, `CreatedByMeCriteria`, `AssignedToMeCriteria`, `SubmittedByMeCriteria`, `PublishedByMeCriteria`, `ApprovedByMeCriteria`, `OwnedByMeCriteria`, `TargetProjectCriteria`, `SourceProjectCriteria`, `IncludesCommitCriteria`, `IncludesIssueCriteria`, `RanOnCriteria`, `FixedIssueCriteria`, `FixedBetweenCriteria`, `ChoiceFieldCriteria`, `EstimatedTimeCriteria`) | - [ ] |

### 1.3 `onedev-db` – Persistenz
| # | Task | Status |
|---|------|--------|
| 1.3.1 | `sqlx` Models für alle Entities (derive `FromRow`, `Decode`) | - [ ] |
| 1.3.2 | Repository-Traits (`UserRepo`, `ProjectRepo`, `IssueRepo`, `PullRequestRepo`, `BuildRepo`, `WorkspaceRepo`, `PackRepo`, `AccessTokenRepo`, `GroupRepo`, `RoleRepo`, `SettingRepo`) | - [ ] |
| 1.3.3 | Postgres-Implementierungen der Repos | - [ ] |
| 1.3.4 | Migrationen: `init.sql` (Schema), inkrementelle Migrations (`V1__...`, `V2__...`) | - [ ] |
| 1.3.5 | Connection-Pool (`sqlx::PgPool`), `SessionService` Port (`openSession`, `closeSession`, `call`, `run`, `runAsync`, `runAsyncAfterCommit`) | - [ ] |
| 1.3.6 | `TransactionService` Port (`callWithTransaction`, `runAfterCommit`) | - [ ] |
| 1.3.7 | `IdService` Port (HiLo / UUID) | - [ ] |
| 1.3.8 | Repository-Tests gegen Test-DB (Testcontainers oder lokale pgrust) | - [ ] |

---

## Phase 2 – Auth & Security

| # | Task | Owner | Status |
|---|------|-------|--------|
| 2.1 | `Subject` / `PrincipalCollection` Port (`u:<id>`, `a:<tokenId>`, `anonymous`) | sec | - [ ] |
| 2.2 | `SecurityUtils` Port: `getSubject()`, `asSubject()`, `asAnonymous()`, `asSystem()`, `bindAsSystem()`, `inheritSubject()` | sec | - [ ] |
| 2.3 | Permission-Check: `checkPermission()`, `isPermitted()`, `requiresUser()` | sec | - [ ] |
| 2.4 | `AuthorizationInfoCache` Integration in Permission-Checks | sec | - [ ] |
| 2.5 | `BaseAuthorizationService` Port (User/Group/Project-Authorizations, Rollen-Vererbung, Default-Roles) | sec | - [ ] |
| 2.6 | Password-Hashing: Argon2 (Parameter aus Java `PasswordService`), Verify | sec | - [ ] |
| 2.7 | AccessToken: Create (permanent + temporal), Validate, Revoke, Scopes | sec | - [ ] |
| 2.8 | 2FA (TOTP): Generate Secret, QR-Code, Verify, Backup-Codes | sec | - [ ] |
| 2.9 | Axum-Extractor `AuthSubject` + Middleware `require_permission(Permission)` | sec | - [ ] |
| 2.10 | Session-Management: `SessionService` in Axum-State, Request-Lifecycle | sec | - [ ] |

---

## Phase 3 – HTTP API & Web (`onedev-api`, `onedev-server`)

### 3.1 Core API (`onedev-api`)
| # | Endpoint-Gruppe | Status |
|---|-----------------|--------|
| 3.1.1 | `GET/POST /api/users`, `GET/PUT/DELETE /api/users/{id}` | - [ ] |
| 3.1.2 | `GET/POST /api/projects`, `GET/PUT/DELETE /api/projects/{id}` | - [ ] |
| 3.1.3 | `GET/POST /api/issues`, `GET/PUT/DELETE /api/issues/{id}`, Search | - [ ] |
| 3.1.4 | `GET/POST /api/pull-requests`, `GET/PUT/DELETE /api/pull-requests/{id}`, Merge | - [ ] |
| 3.1.5 | `GET/POST /api/builds`, `GET/PUT/DELETE /api/builds/{id}`, Pipeline, Logs | - [ ] |
| 3.1.6 | `GET/POST /api/workspaces`, `GET/PUT/DELETE /api/workspaces/{id}` | - [ ] |
| 3.1.7 | `GET/POST /api/packs`, `GET/PUT/DELETE /api/packs/{id}`, Blobs | - [ ] |
| 3.1.8 | `GET/POST /api/groups`, `GET/PUT/DELETE /api/groups/{id}` | - [ ] |
| 3.1.9 | `GET/POST /api/roles`, `GET/PUT/DELETE /api/roles/{id}` | - [ ] |
| 3.1.10 | `GET/POST /api/settings`, `GET/PUT /api/settings/{key}` | - [ ] |
| 3.1.11 | `GET /api/search/*` (Code, Entity, Commit, BuildMetric) | - [ ] |
| 3.1.12 | Pagination, Sorting, Filtering (Cursor-based) | - [ ] |

### 3.2 Auth API
| # | Endpoint | Status |
|---|----------|--------|
| 3.2.1 | `POST /api/auth/login` (Username/Password, 2FA) | - [ ] |
| 3.2.2 | `POST /api/auth/logout` | - [ ] |
| 3.2.3 | `POST /api/auth/password-reset` (Request + Confirm) | - [ ] |
| 3.2.4 | `POST /api/auth/2fa/enable`, `POST /api/auth/2fa/disable`, `POST /api/auth/2fa/verify` | - [ ] |
| 3.2.5 | `GET/POST/DELETE /api/auth/tokens` (AccessToken CRUD) | - [ ] |
| 3.2.6 | `GET /api/auth/me` (Current User + Permissions) | - [ ] |

### 3.3 Stack-Control API (Regel 7)
| # | Endpoint | Status |
|---|----------|--------|
| 3.3.1 | `GET /api/stack/status` → Container-Status (db, core, gateway, …) | - [ ] |
| 3.3.2 | `GET /api/stack/logs?service=…&follow=true` → Logs (Secret-Redaktion) | - [ ] |
| 3.3.3 | `POST /api/stack/up` (Body: `{confirm: true}`) | - [ ] |
| 3.3.4 | `POST /api/stack/down` (Body: `{confirm: true}`) | - [ ] |
| 3.3.5 | `POST /api/stack/restart` (Body: `{confirm: true}`) | - [ ] |

### 3.4 Git HTTP API
| # | Endpoint | Status |
|---|----------|--------|
| 3.4.1 | `GET /api/git/{project}/info/refs` (upload-pack / receive-pack) | - [ ] |
| 3.4.2 | `POST /api/git/{project}/git-upload-pack` | - [ ] |
| 3.4.3 | `POST /api/git/{project}/git-receive-pack` | - [ ] |
| 3.4.4 | LFS: `GET/POST /api/git/{project}/info/lfs/objects/*` | - [ ] |
| 3.4.5 | Hooks: pre-receive, post-receive (Permission-Check, Protected Branches/Tags) | - [ ] |

### 3.5 Web UI (dx/axum SSR oder SPA)
| # | Feature | Status |
|---|---------|--------|
| 3.5.1 | `dx build --web` konfigurieren (Tailwind, Components) | - [ ] |
| 3.5.2 | Dashboard: Project-List, My-Issues, My-PRs, My-Builds | - [ ] |
| 3.5.3 | Project-View: Code, Issues, PRs, Builds, Packs, Settings | - [ ] |
| 3.5.4 | Issue/PR/Build Detail-Views mit Actions | - [ ] |
| 3.5.5 | **Stack-Tab**: Container-Liste, Buttons up/down/restart (confirm), Logs follow | - [ ] |
| 3.5.6 | Admin-Settings: Users, Groups, Roles, Security, Email, OAuth, SSH | - [ ] |

---

## Phase 4 – Git & Build (`onedev-git`, `onedev-build`)

### 4.1 `onedev-git`
| # | Task | Status |
|---|------|--------|
| 4.1.1 | Repo open/init (`gix::Repository`), Bare vs Worktree | - [ ] |
| 4.1.2 | Ref-Handling: `refs/heads/*`, `refs/tags/*`, `refs/pull/*`, `refs/notes/*` | - [ ] |
| 4.1.3 | Rev-Parse: Commit, Tag, Branch, Short-Hash, `HEAD`, `FETCH_HEAD` | - [ ] |
| 4.1.4 | Pack-Protocol v2: `git-upload-pack`, `git-receive-pack` (Capability-Advertisement, Want/Have, Packfile-Streaming) | - [ ] |
| 4.1.5 | LFS: Pointer-File-Detection, Object-Storage, Batch-API | - [ ] |
| 4.1.6 | Hooks: `pre-receive` (Protected Branches/Tags, Permission), `post-receive` (Build-Trigger, Notifications) | - [ ] |
| 4.1.7 | GC: `git gc --auto`, Packfile-Optimierung | - [ ] |
| 4.1.8 | Repo-Locking (concurrent receive-pack) | - [ ] |

### 4.2 `onedev-build`
| # | Task | Status |
|---|------|--------|
| 4.2.1 | Build-Spec Parser (YAML → `BuildSpec` AST), Validation | - [ ] |
| 4.2.2 | Job-Graph: Dependencies, Parallelisierung, Matrix, Conditions | - [ ] |
| 4.2.3 | Executor-Trait: `LocalExecutor`, `RemoteDockerExecutor`, `ServerDockerExecutor`, `ServerSandboxExecutor`, `KubernetesExecutor`, `RemoteHostExecutor` | - [ ] |
| 4.2.4 | Cache: Key-Generierung, Upload/Download, Pruning | - [ ] |
| 4.2.5 | Artifacts: Upload, Download, Retention, Expiration | - [ ] |
| 4.2.6 | Pipeline-Visualisierung (Graph → Mermaid/JSON für UI) | - [ ] |
| 4.2.7 | Terminal-Zugriff: PTY-Stream über WebSocket, Permission-Check (`canOpenTerminal`) | - [ ] |
| 4.2.8 | Build-Logs: Streaming, Secret-Redaktion (`util::redact_env_secrets`), Storage | - [ ] |

---

## Phase 5 – Gateway & OCI (`onedev-server` + Infra)

| # | Task | Owner | Status |
|---|------|-------|--------|
| 5.1 | `ruxen` Image bauen: `docker build -t bkg-core-ruxen:mango infra/ruxen` (aus `contrib/docker/Dockerfile`) | infra | - [ ] |
| 5.2 | Compose: `gateway` Profil mit `bkg-core-ruxen:mango`, Config-Volume, `ruxen -s reload` Endpoint | infra | - [ ] |
| 5.3 | Domain-Routing Registry: `host` → `container:port` in `control.*` (bkg_admin), API `domain::reload()` | core | - [ ] |
| 5.4 | `youki` Wrapper: OCI-Bundle Builder (Layer→rootfs, `config.json`, Namespaces: pid, net, mnt, uts, ipc, user, cgroup; cgroup v2 unified: cpu, cpuset, io, memory, pids, hugetlb) | core | - [ ] |
| 5.5 | `youki` Operations: `create`, `start`, `stop`, `delete` (Rust Bindings / CLI-Wrapper) | core | - [ ] |
| 5.6 | Stack-Tab Integration: Container-Listing via `docker compose ps --format json`, Actions via Compose API | core | - [ ] |
| 5.7 | Logs: `docker compose logs --follow --timestamps` → `/api/stack/logs` mit Secret-Redaktion | core | - [ ] |

---

## Phase 6 – Hardening & Cutover

| # | Task | Owner | Status |
|---|------|-------|--------|
| 6.1 | `cargo test --quiet` (alle Crates) grün | core | - [ ] |
| 6.2 | `cargo clippy -D warnings` clean | core | - [ ] |
| 6.3 | `cargo fmt --check` clean | core | - [ ] |
| 6.4 | `dx build --web` erfolgreich | core | - [ ] |
| 6.5 | `docker compose -f docker/compose.yml config --quiet` ok | infra | - [ ] |
| 6.6 | Stack-Neustart: `docker compose up -d` → `/health` 200 | infra | - [ ] |
| 6.7 | `/api/stack/status` mit Session 200, Container gelistet | core | - [ ] |
| 6.8 | Dashboard Stack-Tab: Buttons funktionieren, Logs follow | core | - [ ] |
| 6.9 | `admin logs --follow` läuft, **keine Secrets** in Ausgabe (Redaktion prüfen) | core | - [ ] |
| 6.10 | Git-Status Forks: `cd infra/pgrust && git status`, `cd infra/ruxen && git status`, `cd infra/youki && git status` → **clean** | infra | - [ ] |
| 6.11 | `docs/progress.md` finalisieren, `CHANGELOG.md` Entry für `v0.1.0-mango` | core | - [ ] |

---

## Quer-Schnittsaufgaben (laufend)

| # | Task | Owner | Status |
|---|------|-------|--------|
| X.1 | CI/CD: GitHub Actions / OneDev Buildspec für `cargo test`, `clippy`, `fmt`, `docker build` | core | - [ ] |
| X.2 | Observability: `tracing` Setup (OTLP Export), Metrics (Prometheus), Health-Checks | core | - [ ] |
| X.3 | Secrets-Handling: `util::redact_env_secrets` Port, Log-Filter, Env-Var-Scrubbing | sec | - [ ] |
| X.4 | Error-Handling: `anyhow`/`thiserror` konsistent, Problem-Details (RFC 7807) in API | core | - [ ] |
| X.5 | Dokumentation: Rustdoc (`cargo doc`), API-Docs (OpenAPI/Swagger via `utoipa`), Architektur-Entscheidungen (ADR) | core | - [ ] |

---

*Letzte Aktualisierung: 2026-11-17 – Branch `mango`*