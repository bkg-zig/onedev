# 05 — CI/CD, Paket-Registries, Workspaces (1-to-1 PORT)

Quelle: `server/buildspec` (158), `server/buildspecmodel` (60), `server/job` (16), `server/agent` (3), `server/pack` (3), `server/workspace` (13), `server/terminal` (3), `server/logging` (26), die Plugin-Module `server-plugin-executor-*`, `server-plugin-provisioner-*`, `server-plugin-buildspec-*`, `server-plugin-report-*`, `server-plugin-pack-*`, gegen `crates/onedev-build` und die Modelle `build.rs`, `pack.rs`, `workspace.rs`.

## 1. Kurzfazit ⚠️ KEIN MVP - VOLLE 1-to-1 PORTIERUNG

**WICHTIG:** Dies ist ein **vollständiger 1-to-1 Port**, kein MVP!

**Blockierender Fehler:** `crates/onedev-build` modliert **GitLab-CI-Konzepte** statt OneDev Build-Spec:
- `Pipeline` (OneDev: Build mit Jobs)
- `Stages` (OneDev: Job-Gruppen)
- `Runner` (OneDev: Agent)

**KEIN MVP:** Die gesamte Build-Spec-Implementierung muss wiederholt werden!

## 2. Vanilla-Inventar

| Bereich | Pfad | Umfang | Zweck |
|---|---|---|---|
| Build-Spec | `server/buildspec` | 158 | Job-, Step-, Trigger- und Param-Typen |
| Build-Spec-Modell | `server/buildspecmodel` | 60 | serialisierbare Form der Spec |
| Job-Ausführung | `server/job` | 16 | Scheduling, Ressourcen, Concurrency |
| Agent | `server/agent` | 3 | Remote-Build-Agents |
| Logging | `server/logging` | 26 | Build-Logs, Sammlung, Kürzung |
| Workspace | `server/workspace` | 13 | interaktive Entwicklungsumgebungen |
| Terminal | `server/terminal` | 3 | Shell-Zugriff auf laufende Builds/Workspaces |
| Pack | `server/pack` | 3 | Paket-Registry-Kern |
| Executor-Plugins | `server-plugin-executor-{kubernetes,remotedocker,remotehost,serverdocker,serverhost,serversandbox}` | 6 | wo ein Job läuft |
| Provisioner-Plugins | `server-plugin-provisioner-{serverdocker,serverhost,serversandbox}` | 3 | wie Workspaces bereitgestellt werden |
| Build-Spec-Vorlagen | `server-plugin-buildspec-{bazel,cmake,dotnet,golang,gradle,maven,node,python,rust}` | 9 | fertige Job-Vorlagen pro Sprache |
| Report-Plugins | `server-plugin-report-*` | ~25 | JUnit, JaCoCo, PMD, SpotBugs, Clippy, Ruff, Jest, Playwright, Coverage, HTML, Markdown, ... |
| Pack-Plugins | `server-plugin-pack-{cargo,container,gem,helm,maven,npm,nuget,pypi}` | 8 | je ein Registry-Protokoll |

Entitäten dazu: `Build`, `BuildDependence`, `BuildLabel`, `BuildParam`, `CoverageMetric`, `UnitTestMetric`, `ProblemMetric`, `Agent`, `AgentAttribute`, `AgentToken`, `Pack`, `PackBlob`, `PackBlobReference`, `PackLabel`, `Workspace`.

## 3. Rust-Stand ⚠️ KEIN MVP

**WICHTIG:** Dies ist ein **vollständiger 1-to-1 Port**, kein MVP!

`crates/onedev-build` ist **teilweise implementiert** (nicht als Stub mehr), aber noch nicht vollständig für 1-to-1 Port.

### Implementierte Komponenten

| Komponente | Datei | Status |
|------------|-------|--------|
| Fehler/Result-Typen | `errors.rs` | ✅ implementiert |
| Modelle | `models.rs` | ✅ implementiert |
| Utils | `utils.rs` | ✅ vorhanden |
| Handler | `handlers/` | ✅ definiert |

### Noch zu erledigen für 1-to-1 Port

1. **Build-Spec-Parser** (158 Klassen): komplette Implementierung von Jobs, Steps, Triggern
2. **Executor-Plugins** (6 Module): Docker, Kubernetes, Shell, Remote-Agent
3. **Report-Plugins** (~25 Module): JUnit, JaCoCo, PMD, Clippy, etc.
4. **Pack-Plugins** (8 Protokolle): OCI, Maven, npm, PyPI, Cargo, etc.
5. **Workspace-Provisionierung**: Docker/Shell/Sandbox over Kubernetes-Modell

`crates/onedev-core/src/models/build.rs` beschreibt Builds in GitLab-Begriffen (Pipeline, Stages). **DIES IST KEIN ONEDEV-KONZEPT!**

### Kritisches Problem: GitLab-Modell statt OneDev

`crates/onedev-build` modelliert:
- `Pipeline` (OneDev: Build mit Jobs)
- `Stages` (OneDev: Job-Gruppen)  
- `Runner` (OneDev: Agent)

`crates/onedev-db` hat bereits `pipeline.rs` als Modell — ein **falscher Datensatz**!

> **KEIN MVP:** Ohne Build-Spec-Parser, Executor-Plugins und Reports funktioniert kein 1-to-1 Port!