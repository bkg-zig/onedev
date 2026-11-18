# Handover: Was für 1:1 (100 %) noch fehlt — Stand 18.11.2026, Branch `mango`

Ziel: vollständiger 1:1-Port von Vanilla OneDev v17.2.1 nach Rust. Kein MVP.

Verifiziert: `cargo check --workspace` meldet 0 Fehler. Alle unten als
"erledigt" markierten Punkte sind gebaut, getestet und auf `mango` committet.

---

## 1. Erledigt und verifiziert

| Bereich | Stand |
|---|---|
| `onedev-core` / `onedev-db` | Domain-Modelle + DB-Schicht, Migrationen 001–005 |
| `onedev-auth` | Rollenmodell 1:1 (`privilege.rs`, `implies()`-Ketten), Shiro-Parität Basis |
| `onedev-api` | Alle Handler echt verdrahtet: Auth, User, Project, Group, Issue, MR, Build, Git-Smart-HTTP, LFS |
| `onedev-git` | Repos lesen (gix), Smart-HTTP, Hooks, LFS, Blame, Submodules, LastCommits |
| `onedev-build` | Vanilla-BuildSpec inkl. eigener Vanilla-Spec als Test, Host-Executor, OCI-Registry-Client, 130 Tests |
| `onedev-web` | Binary serviert echt; Layout + Login + Listen (Projekte, Issues, MRs, Builds, Git) 1:1 nach Vanilla mit echten DB/Git-Daten; 267 Vanilla-CSS vendored; alle Seiten Smoke-Test 200 |
| `onedev-cli` | Kompiliert (Stack/User/Project/Build/Git/Db/Config/Backup/Admin/Dev) |
| `onedev-server` | Kompiliert (Lead-Komposition gemergt) |
| Compose | Nur OneDev (`db` + `core` + `web`), `docker-compose.yml` am Root, `scripts/docker-build.sh` steuert alles, `scripts/temp-db.sh` für Wegwerf-DB |

---

## 2. Was für 100 % noch fehlt (nach Priorität)

### P0 — Blocker für Vanilla-Parität

1. **Web-UI: ~520 von 552 Templates fehlen.** Portiert sind Layout,
   Login, Dashboard, 5 Listen (Projekte, Issues, MRs, Builds, Git) und
   Basis-Detailseiten. Es fehlen u. a.: Issue-Detail voll (Kommentare,
   State-Transitions), MR-Detail (Diff, Checks, Merge), Build-Detail
   (Log-Streaming, Steps), Datei-Browser voll (71 Klassen: Diff-Viewer,
   Markdown-Editor mit 23 Klassen), Issue-Board (74 Klassen),
   Build-Graph (24 Klassen), Admin-Seiten, Profile-Einstellungen,
   429 Vanilla-JS-Dateien (CodeMirror, Diff, Markdown).
2. **Suchfunktion komplett fehlt.** 333 Lucene-Klassen + 11 ANTLR-Grammatiken
   (Code-, Issue-, Build-, Commit-Suche mit Autovervollständigung).
   Geplant: tantivy + pest. Ohne Suche keine Parität bei Datei-/Issue-/Build-Seiten.
3. **Abfragesprachen fehlen.** Alle Listen nutzen derzeit Substring-Filter
   statt Vanilla-Query-Sprache (`State is Open`, `"Project" is current` …).
   Betrifft Projekte, Issues, MRs, Builds, Commits.

### P1 — Hoch

4. **Executor-Plugins fehlen** (nur Host-Executor da): Docker,
   Kubernetes, Server-Docker, Remote-Host, Sandbox (youki-Dateien liegen
   als Stub in `onedev-build/src/services/youki/`, unverdrahtet).
5. **Report-Plugins fehlen**: ~25 Stück (JUnit, PMD, Clippy, Ruff,
   Coverage, HTML, Markdown …).
6. **Pack-Registries fehlen**: 8 Paketformate (Maven, npm, PyPI, NuGet,
   RubyGems, Conan, Alpine, RPM) — nur Modelle vorhanden.
7. **API-Lücken**: Rollen-/Autorisierungs-Endpunkte
   (`/roles`, `/user-authorizations`, `/group-authorizations`,
   `/base-authorizations`, `/issue-authorizations`,
   `/access-token-authorizations`), SSH-Keys, Memberships, Admin-Endpunkte.
8. **Shiro-Rest**: Passwort-Migration BCrypt→Argon2id unvollständig,
   Session-Refresh-Rotation prüfen, alle 40 Permission-Klassen
   gegen Vanilla abgleichen.

### P2 — Mittel

9. **Git-Lücken**: SSH-Zugriff, Archiv-Download (zip/tar),
   `BlobEdits`, Datei-Filter-System, Background-Git-Tasks.
10. **Build-Lücken**: Build-Detail Log-Streaming im Web, Artefakt-Browser,
    Parameter-Matrix-UI, Job-Abhängigkeitsgraph.
11. **CLI-Lücken**: Handler sind Stubs (`println!`), keine echte
    Stack-/User-/Build-Steuerung; `admin logs --follow` ohne Secrets fehlt.
12. **Tests**: Hook-/Protection-Tests, LFS-Batch-Tests, E2E-Tests fehlen;
    nur Unit- + Smoke-Tests vorhanden.

### P3 — Operativ

13. **Docker-Build auf deiner Maschine fertig bringen** (läuft bei dir,
    hier kein Daemon). Offene lokale Änderung: `docker/Dockerfile.web`
    (Toolchain-Bump, uncommittet) und deine SupaDB-Umstellung in
    `docker-compose.yml` (uncommittet, bewusst nicht angerührt).
14. **CI-Pipeline** für den Rust-Workspace fehlt (nur Doku-Checklisten).

---

## 3. Empfohlene Reihenfolge (nächste Slices)

1. Suche (tantivy + erste pest-Grammatik für Issues) — entsperrt alle Listen.
2. Issue-Detail 1:1 (Kommentare, Transitions) mit echten Daten.
3. MR-Detail 1:1 (Diff via gix, Checks, Merge-Button).
4. Datei-Browser voll (Tree + Blob sind verdrahtet, Diff-Viewer fehlt).
5. Executor Docker + erster Report (JUnit).
6. Rollen-/Autorisierungs-API-Endpunkte.
7. CLI-Handler echt verdrahten.
8. Build-Detail mit Log-Streaming.

---

## 4. Offene lokale Änderungen (nicht committet, bewusst)

- `docker/Dockerfile.web` — Toolchain-Bump (Build-Fix, kann committet werden).
- `docker-compose.yml` — deine SupaDB-Umstellung (deine Baustelle).
- `.gitignore` — deine Änderung.
- `docs/user-report.mp3` — deine Datei.
- Secrets unter `docker/secrets/` — gitignored, pro Maschine neu erzeugen
  (Anleitung in `docs/compose-web-test.md`).

## 5. Referenzen

- `docs/port-review/missing-functions-mapping.md` — Vollständige Lückenliste
- `docs/port-review/review-fix.md` — Review-Fixes und Gaps
- `docs/port-review/01-web-ui.md` … `06-platform-api-ops.md` — Bereichsberichte
- `docs/progress.md` — Fortschrittstabelle
- `docs/compose-web-test.md` — Compose + Web-Test-Anleitung
- Skripte: `scripts/docker-build.sh` (Stack-Lifecycle), `scripts/temp-db.sh` (Wegwerf-DB)
