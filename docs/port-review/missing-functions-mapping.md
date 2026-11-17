# Missing Functions Mapping: Vanilla Java → Rust 1-to-1 Port

## Übersicht

Dieses Dokument listet alle fehlenden Funktionen auf, die für einen vollständigen 1-to-1 Port von Vanilla OneDev v17.2.1 zu Rust erforderlich sind.

---

## 1. Git-Schicht (`onedev-git`)

### Vanilla Java Files → Rust Status

| Java File | LOC | Rust Status | Fehlende Funktionen |
|-----------|-----|-------------|---------------------|
| `server/git/GitUtils.java` | ~500 | ❌ Fehlt | Git-Operationen, Branch-Operationen, Tag-Operationen |
| `server/git/Blob.java` | ~150 | ✅ Teilweise | `BlobContent`, `BlobEdits`, `BlameCommit` |
| `server/git/BlameCommit.java` | ~100 | ❌ Fehlt | Blame-Implementierung |
| `server/git/GitLfsFilter.java` | ~200 | ✅ Teilweise | LFS-Authentication, Upload/Download |
| `server/git/Submodule.java` | ~80 | ❌ Fehlt | Submodule-Unterstützung |
| `server/git/GitFilter.java` | ~150 | ❌ Fehlt | Datei-Filter-System |
| `server/git/GitTask.java` | ~100 | ❌ Fehlt | Background Git-Operationen |
| `server/git/SshCommand.java` | ~150 | ❌ Fehlt | SSH-Kommando-Erweiterung |

### Kritische fehlende Funktionen in `models.rs`

```
❌ BlobContent::text() - nur Partial-Implementierung
❌ BlameCommit - vollständig fehlt
❌ BlameBlock - fehlt
❌ BlobChange - fehlt
❌ BlobIdentFilter - fehlt
❌ Submodule - fehlt
❌ LastCommitsOfChildren - fehlt
```

### Kritische fehlende Services in `services/`

```
❌ services/hooks.rs - muss HookEngine mit Pre-Receive-Validierung enthalten
❌ services/lfs.rs - muss vollständige LFS-Protokoll-Implementierung enthalten
❌ services/repository.rs - muss RepositoryService mit gix-Integration enthalten
❌ services/smart_http.rs - Teilweise (UploadPack/ReceivePack), aber:
   - KEINE Branch-Protection-Validierung
   - KEINE Hook-Integration für Push-Events
   - KEINE Ref-Update-Validierung
```

---

## 2. Build-Spec (`onedev-build`)

### Vanilla BuildSpec Files (158 Klassen) → Rust Status

| Vanilla Bereich | Anzahl | Rust Status | Fehlende Teile |
|-----------------|--------|-------------|----------------|
| `buildspec/BuildSpec.java` | 158 | ❌ Fehlt | Komplette Build-Spec-Implementierung |
| `buildspec/Build.java` | 16 | ❌ Fehlt | Build-Entität und Status |
| `buildspec/Job.java` | ~50 | ❌ Fehlt | Job-Definition, Steps, Triggers |
| `buildspec/Agent.java` | 3 | ✅ Teilweise | Agent-Model, aber keine Executor-Implementierung |
| `buildspec/Pack.java` | 3 | ✅ Teilweise | Pack-Model, aber keine Protokoll-Schicht |
| `buildspec/Workspace.java` | 13 | ✅ Teilweise | Workspace-Model, aber falsche Kubernetes-Modellierung |

### Kritische fehlende Modelle in `models.rs`

```
❌ BuildSpec - die zentrale Build-Spezifikation
❌ Job - mit Steps, Triggern, Abhängigkeiten
❌ Step - Checkout, Commands, Docker, Reports
❌ Trigger - Push, Tag, Schedule, PR, Dependency
❌ ParamSpec - Parameter für Jobs
❌ BuildParams - Parameter-Bindung
❌ BuildDependence - Job-Abhängigkeiten
```

### Kritische fehlende Executor-Plugins

```
❌ server-plugin-executor-docker
❌ server-plugin-executor-kubernetes
❌ server-plugin-executor-serverhost
❌ server-plugin-executor-serverdocker
❌ server-plugin-executor-remotehost
❌ server-plugin-executor-serversandbox
```

### Kritische fehlende Report-Plugins (~25)

```
❌ server-plugin-report-junit
❌ server-plugin-report-pmd
❌ server-plugin-report-clippy
❌ server-plugin-report-ruff
❌ server-plugin-report-coverage
❌ server-plugin-report-html
❌ server-plugin-report-markdown
... (20 weitere)
```

---

## 3. Sicherheitsmodell (`onedev-auth` + `onedev-core/src/permissions`)

### Vanilla Security Files → Rust Status

| Vanilla Datei | Klassen | Rust Status | Problem |
|---------------|---------|-------------|---------|
| `model/Role.java` | 1 | ❌ Falsch | GitLab-Bitflags statt OneDev free-form Rollen |
| `model/{Base,User,Group,Issue,Link,AccessToken}Authorization.java` | 6 | ❌ Fehlt | Autorisierungs-Tabellen |
| `server/security/permission/*.java` | ~40 | ❌ Fehlt | Privilege-Klassen mit implies() |

### Kritische fehlende Implementierungen

```
❌ PermissionChecker::check_* gibt immer true zurück
❌ Keine implizierende Berechtigungsprüfung
❌ Keine UserAuthorization-Entität
❌ Keine GroupAuthorization-Entität
❌ Keine BaseAuthorization-Entität
❌ Keine IssueAuthorization-Entität
❌ Keine AccessTokenAuthorization-Entität
```

---

## 4. API-Schicht (`onedev-api`)

### Vanilla REST-Ressourcen (47 `@Path`-Gruppen) → Rust Status

| API Gruppe | Rust Status | Fehlende Endpunkte |
|------------|-------------|-------------------|
| `/projects` | ❌ Fehlt | CRUD + Spezielle Operationen |
| `/repositories` | ❌ Fehlt | Git-Integration |
| `/users` | ❌ Fehlt | User-Management |
| `/groups` | ❌ Fehlt | Gruppen-Management |
| `/memberships` | ❌ Fehlt | Mitgliedschafts-Management |
| `/roles` | ❌ Fehlt | Rollen-Management |
| `/roles/*/members` | ❌ Fehlt | Rollen-Mitglieder |
| `/user-authorizations` | ❌ Fehlt | User-Autorisierungen |
| `/group-authorizations` | ❌ Fehlt | Group-Autorisierungen |
| `/base-authorizations` | ❌ Fehlt | Base-Autorisierungen |
| `/issue-authorizations` | ❌ Fehlt | Issue-Autorisierungen |
| `/access-token-authorizations` | ❌ Fehlt | Token-Autorisierungen |
| `/ssh-keys` | ❌ Fehlt | SSH-Key-Management |
| `/issues` | ❌ Fehlt | Issue-Tracking |
| `/pulls` | ❌ Fehlt | Pull-Request-Management |
| `/builds` | ❌ Fehlt | Build-Tracking |
| ... | ... | ... |

---

## 5. Web-UI (`onedev-web`)

### Vanilla UI Files → Rust Status

| Typ | Anzahl | Rust Status |
|-----|--------|-------------|
| Java-Klassen | 1502 | ❌ Leider
| HTML Templates | 552 | ❌ Keine
| JS-Dateien | 400 | ❌ Keine
| CSS-Dateien | 267 | ❌ Keine

### Kritische fehlende UI-Komponenten

```
❌ Page-Klassen (457 Seiten)
❌ Component-Klassen (574 Widgets)
❌ Editable-Klassen (222 Formular-Klassen)
❌ Behavior-Klassen (50 AJAX-Verhalten)
❌ Diff-Viewer (27 Klassen)
❌ Markdown-Editor (23 Klassen)
❌ Datei-Browser (71 Klassen)
❌ Issue-Board (74 Klassen)
❌ Build-/Pipeline-Graph (24 Klassen)
```

---

## 6. Suchfunktionalität

### Vanilla Search → Rust Status

| Vanilla | Rust | Status |
|---------|------|--------|
| `server/search/` 333 Klassen | - | ❌ Fehlt |
| Lucene Index | tantivy (vorgeschlagen) | ⚠️ Nicht implementiert |
| ANTLR Grammatiken (11) | pest/serde | ❌ Fehlt |

---

## Summare Geforderte Arbeit für 1-to-1 PORT

### Kritische Blocker (müssen vor allem anderen erledigt werden):

1. **Rollenmodell-Überarbeitung** - Von GitLab auf Vanilla-Shiro
2. **Build-Spec-Parser** - Komplette Implementierung
3. **Web-UI-Wiederholung** - Alle 552 HTML-Templates

### Hohe Priorität:

1. **Git-Smart-HTTP erweitern** - Hooks, Branch-Protection, Ref-Updates
2. **API-Endpunkte** - Alle 47 REST-Gruppen
3. **Search-Funktionalität** - Lucene → tantivy, ANTLR → pest

### Mittlere Priorität:

1. **Executor-Plugins** - Docker, Kubernetes, Shell, Remote
2. **Report-Plugins** - ~25 Plugins implementieren
3. **LFS erweitern** - Vollständige Protokoll-Unterstützung

---

## Empfohlene nächste Schritte

1. **Sofort:** Rollenmodell auf Vanilla-Modell umstellen (RV-PORT-003)
2. **Kurzfristig:** Build-Spec-Parser implementieren (RV-PORT-006)
3. **Mittelfristig:** Web-UI Portierungsstrategie definieren (RV-PORT-001)
4. **Langfristig:** Alle API-Endpunkte implementieren
5. **Abgeschlossen:** Search-Funktionalität implementieren

---

*Analyse erstellt: 2026-11-17*
*Referenz: Vanilla OneDev v17.2.1*
*Status: Vollständiger 1-to-1 Port erforderlich*