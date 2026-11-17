# 05 — CI/CD, Paket-Registries, Workspaces

Quelle: `server/buildspec` (158), `server/buildspecmodel` (60), `server/job` (16), `server/agent` (3), `server/pack` (3), `server/workspace` (13), `server/terminal` (3), `server/logging` (26), die Plugin-Module `server-plugin-executor-*`, `server-plugin-provisioner-*`, `server-plugin-buildspec-*`, `server-plugin-report-*`, `server-plugin-pack-*`, gegen `crates/onedev-build` und die Modelle `build.rs`, `pack.rs`, `workspace.rs`.

## 1. Kurzfazit

OneDev macht CI nicht mit Pipelines. Eine **Build-Spec** (`.onedev-buildspec.yml`) beschreibt Jobs, jeder Job hat Steps, Trigger und Abhängigkeiten zu anderen Jobs; ausgeführt wird über **Executors** (Docker, Kubernetes, Shell, Remote-Agent). Rust modelliert stattdessen GitLab-CI: `Pipeline`, Stages, `Runner`. `crates/onedev-build` ist zudem ein Stub mit 19 Zeilen, die Modelle in `onedev-core` beschreiben das falsche Konzept, und die DB-Schicht hat bereits `pipeline`-Tabellen dafür. Deckung: ~5 %, und der vorhandene Teil müsste größtenteils ersetzt werden. Das ist die zweite große Fehlentscheidung neben dem Rollenmodell.

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

## 3. Rust-Stand

`crates/onedev-build` enthält 19 Zeilen. `crates/onedev-core/src/models/build.rs` beschreibt Builds in GitLab-Begriffen (Pipeline, Stages, Job-Status wie `canceled`/`skipped` im GitLab-Sinn). `pack.rs` modelliert Registries, aber ohne die Protokoll-Schicht. `workspace.rs` trifft die Idee einer interaktiven Umgebung, verwendet aber Kubernetes-Begriffe (`PersistentVolumeClaim`, `ConfigMap`, `EmptyDir`), während OneDev seine Workspaces über die Provisioner-Plugins auf Docker, Shell oder Sandbox bereitstellt.

`crates/onedev-db` hat `pipeline.rs` als Modell und Repository — eine Tabelle, der in OneDev nichts entspricht.

## 4. Was ein Build in OneDev tatsächlich ist

Ein Job besteht aus Steps (Checkout, Befehle, Docker-Build, Reports veröffentlichen, Artefakte, Cache). Trigger sind Push, Tag, Pull Request, Schedule, Abhängigkeit von einem anderen Job oder manueller Start. Executor bestimmen den Ausführungsort:

- `serverdocker` / `remotedocker` — Docker auf dem Server oder auf einem entfernten Host.
- `serverhost` / `remotehost` — direkt auf dem Host, ohne Container.
- `serversandbox` — isolierte Ausführung auf dem Server.
- `kubernetes` — als Kubernetes-Job.

Remote-Agents (`Agent`, `AgentToken`) verbinden sich per WebSocket mit dem Server, holen sich Jobs und streamen Logs zurück. Dazu kommen Service-Container, Cache-Keys, Retries, Timeouts, Job-Secrets und Param-Bindung aus dem Pull Request oder Commit.

Rust braucht dafür: eine Container-Laufzeit (die Projektregeln schreiben **youki/OCI** vor und verbieten einen Docker-Daemon; `bollard` nur für den Legacy-Docker-Executor), einen Kubernetes-Client (`kube-rs`) für den Kubernetes-Executor, und den Agent als eigenständiges Binary, das das WebSocket-Protokoll spricht. Das ist deutlich mehr als der „Job-Graph" aus `docs/plan.md` Phase 4.

## 5. Paket-Registries

Acht Protokolle, jedes mit eigener Spezifikation: OCI/Docker Registry v2, Maven-Layout, npm, NuGet v3, PyPI (Simple API), RubyGems, Helm und Cargo. Dazu Storage, Garbage Collection verwaister Blobs (`PackBlobReference`) und Zugriffsrechte über `packPrivilege` im Rollenmodell (Bericht 03). In Rust existiert davon nur das Datenmodell. Für ein MVP genügt **eine** Registry; die übrigen sind austauschbare Module.

## 6. Workspaces

Ein Workspace ist eine langlebige, interaktive Entwicklungsumgebung (Shell, Projekt-Checkout, Ports), nicht ein CI-Job. Bereitstellung über drei Provisioner (Docker, Host, Sandbox), Zugriff über das Terminal-WebSocket (`server/terminal`). Die Rust-Modellierung mit Kubernetes-Volumen-Typen passt nur zum Kubernetes-Fall und sollte auf die drei realen Provisioner reduziert werden.

## 7. Gap-Tabelle

| Feature | Vanilla | Rust | Status | Aufwand | Task |
|---|---|---|---|---|---|
| Build-Spec-Format parsen | `buildspec/` 158, `buildspecmodel/` 60 | — | fehlt | L | 1.1.7, 4.2.1 |
| Job-Abhängigkeiten, Concurrency | `server/job` 16 | Pipeline-Graph (falsches Modell) | falsch | L | 4.2.2 |
| Executor Docker/Host/Sandbox | 6 Executor-Plugins | — | fehlt | XL | 4.2.3 |
| Kubernetes-Executor | `executor-kubernetes` | — | fehlt | L | 4.2.3 |
| Remote-Agent + Protokoll | `server/agent`, `Agent`, `AgentToken` | — | fehlt | L | nein |
| Build-Logs streamen | `server/logging` 26 | — | fehlt | M | 4.2.8 |
| Cache und Artefakte | `server/job`, `pack` | — | fehlt | M | 4.2.4, 4.2.5 |
| Trigger (Push, PR, Schedule, Abhängigkeit) | `buildspec/` | — | fehlt | M | nein |
| Job-Vorlagen pro Sprache | 9 `buildspec-*`-Plugins | — | fehlt | M | nein |
| Report-Verarbeitung | ~25 `report-*`-Plugins | — | fehlt | XL | nein |
| Build-Metriken | `CoverageMetric`, `UnitTestMetric`, `ProblemMetric` | — | fehlt | M | nein |
| Registry Cargo | `pack-cargo` | — | fehlt | M | 1.1.6 |
| Registry Container/Maven/npm/NuGet/PyPI/Helm/Gems | 7 weitere | — | fehlt | XL | 1.1.6 |
| Workspace-Provisioner | 3 Provisioner-Plugins | K8s-Modell | falsch | L | 1.1.5 |
| Terminal-Zugriff | `server/terminal` | — | fehlt | M | 4.2.7 |

## 8. Was ins MVP muss und was nicht

Für einen ersten lauffähigen Build genügen: Build-Spec-Parser, ein einziger Executor (Shell auf dem Host oder OCI über youki), Push-Trigger, Log-Streaming und Artefakte. Kubernetes, Remote-Agents, die acht Sprach-Vorlagen und die ~25 Report-Plugins sind eigene Ausbaustufen. Bei den Registries genügt eine — Cargo liegt für ein Rust-Projekt am nächsten.

## 9. Abweichungen und Risiken

- **`Pipeline`/`Runner`/`Stage` gehören nicht ins Modell.** Sie in `build.rs` und `onedev-db/pipeline.rs` zu lassen, während die echte Struktur Job/Step/Trigger ist, erzeugt zwei CI-Systeme. Empfehlung: entfernen und `Build` an `Build`, `BuildDependence`, `BuildParam` ausrichten.
- **youki statt Docker** ist in `docs/rust-rules.md` Abschnitt 8 verbindlich, betrifft aber nur den Server-eigenen Executor. Die Kompatibilität zu Vanilla entscheidet sich daran, ob dieselben `.onedev-buildspec.yml`-Dateien unverändert laufen. Das Parser-Format ist damit die eigentliche Spec, nicht die Ausführungs-Technologie.
- **Agent-Protokoll** ist nicht dokumentiert, es steckt im Code (`server/agent`, `server/websocket`). Wer Remote-Agents will, muss es aus dem Java-Code rekonstruieren; das sollte nicht nebenbei passieren.

## 10. Empfohlene Reihenfolge

1. Entscheidung: GitLab-CI-Modell verwerfen (Empfehlung) und `pipeline`-Strukturen entfernen.
2. Build-Spec-Parser mit Validierung, aus `buildspecmodel` abgeleitet.
3. Ein Executor, Log-Streaming, Artefakte — genug für einen grünen Beispiel-Job.
4. Trigger und Job-Abhängigkeiten.
5. Eine Paket-Registry.
6. Kubernetes, Agents, Reports, weitere Registries nach Bedarf.
