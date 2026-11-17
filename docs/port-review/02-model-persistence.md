# 02 — Domain-Modell, Persistenz, Service-Schicht (1-to-1 PORT)

Quelle: `server-core/.../model` (104 `@Entity`-Klassen), `persistence` (33), `service` (92 Interfaces), `data` (Versionierung und Migration), gegen `crates/onedev-core/src/models` und `crates/onedev-db`.

## 1. Kurzfazit ⚠️ KEIN MVP - VOLLE 1-to-1 PORTIERUNG

**WICHTIG:** Dies ist ein **vollständiger 1-to-1 Port**, kein MVP!

**Blockierender Fehler:** Rust hat für die großen Entitäten Datenträger, aber sie beschreiben an mehreren Stellen ein **anderes Produkt** (GitLab statt OneDev Konzepte).

### Kritische Abweichungen

**Namensdrift:**
- Rust sagt `MergeRequest*` (GitLab) vs OneDev: `PullRequest`
- `Pipeline` (GitLab) vs OneDev: `Build` mit Job-Abhängigkeiten
- `Runner` vs OneDev: `Agent`
- `Snippet`, `Environment`, `FeatureFlag`, `DeployKey` → gibt es in OneDev nicht!

**Service-Schicht:** 92 Interfaces (Geschäftslogik) existieren in Rust **praktisch nicht**.

**Deckung:** Datenmodell ~25 %, Verhalten ~0 %.

### 1-to-1 Portierung erfordert:

1. **Alle 104 Entity-Klassen** exakt nachbilden
2. **Service-Interfaces** implementieren (Build, Job, Agent, etc.)
3. **Namensangleichung**: `MergeRequest` → `PullRequest` überall
4. **GitLab-only Konzepte entfernen** oder als bewusste Abweichung dokumentieren

> **KEIN MVP:** Ein MVP-Teil funktioniert nicht mit Vanilla-OneDev-Datenbank! Ein vollständiger Port ist erforderlich.

## 2. Vanilla-Inventar

| Bereich | Pfad | Umfang | Zweck |
|---|---|---|---|
| Entitäten | `model/*.java` | 104 mit `@Entity` | gesamtes persistentes Modell |
| Eingebettete Typen | `model/support/**` | 49 allein unter `administration`, plus `issue` (83), `pullrequest` (23), `workspace` (28), `role` (8), `code` (8), `build` (7) | Einstellungen, Workflow, Protections |
| Persistenz | `persistence/**` | 33 Klassen | Hibernate-Setup, Session, Transaktion, ID-Vergabe, Dialekte |
| Datendienst | `data/DefaultDataService.java` | 1, groß | Erstbefüllung, Import/Export, Versionsprüfung |
| Schema-Migration | `data/migration/` | 252 Methoden `migrate5` … `migrate257` | migrationsfreie Upgrades zwischen Versionen |
| Services | `service/**` | 92 Interfaces | Geschäftslogik |
| Querverweise | `entityreference/` | 11 | Referenzen zwischen Commit, Issue, PR, Build, Workspace |

Die 104 Entitäten, gruppiert:

- **Identität und Rechte:** `User`, `EmailAddress`, `Group`, `Membership`, `SshKey`, `GpgKey`, `SsoAccount`, `SsoProvider`, `AccessToken`, `AccessTokenAuthorization`, `OAuthApplication`, `OAuthGrant`, `UserInvitation`, `Role`, `UserAuthorization`, `GroupAuthorization`, `BaseAuthorization`, `IssueAuthorization`, `LinkAuthorization`, `GroupEntitlement`, `UserEntitlement`, `ProjectEntitlement`.
- **Projekt:** `Project`, `ProjectLabel`, `ProjectLastActivityDate`, `ProjectNumberCounter`, `LabelSpec`, `Setting`.
- **Issues:** `Issue`, `IssueField`, `IssueChange`, `IssueComment`, `IssueCommentReaction`, `IssueCommentRevision`, `IssueDescriptionRevision`, `IssueLink`, `IssueMention`, `IssueReaction`, `IssueSchedule`, `IssueStateHistory`, `IssueTouch`, `IssueVote`, `IssueWatch`, `IssueWork`, `IssueWorkReaction`, `Iteration`, `LinkSpec`, `Stopwatch`.
- **Pull Requests:** `PullRequest`, `PullRequestAssignment`, `PullRequestChange`, `PullRequestComment`, `PullRequestCommentReaction`, `PullRequestCommentRevision`, `PullRequestDescriptionRevision`, `PullRequestLabel`, `PullRequestMention`, `PullRequestReaction`, `PullRequestReview`, `PullRequestTouch`, `PullRequestUpdate`, `PullRequestWatch`, `ReviewedDiff`, `PendingSuggestionApply`.
- **Code-Kommentare:** `CodeComment`, `CodeCommentReply`, `CodeCommentMention`, `CodeCommentStatusChange`, `CodeCommentTouch`.
- **Builds:** `Build`, `BuildDependence`, `BuildLabel`, `BuildParam`, `CoverageMetric`, `UnitTestMetric`, `ProblemMetric`.
- **Pakete:** `Pack`, `PackBlob`, `PackBlobReference`, `PackLabel`.
- **Agents:** `Agent`, `AgentAttribute`, `AgentLastUsedDate`, `AgentToken`.
- **Workspaces:** `Workspace`.
- **Dashboards:** `Dashboard`, `DashboardGroupShare`, `DashboardUserShare`, `DashboardVisit`.
- **Suche/Personalisierung:** `BuildQueryPersonalization`, `CodeCommentQueryPersonalization`, `CommitQueryPersonalization`, `IssueQueryPersonalization`, `PackQueryPersonalization`, `PullRequestQueryPersonalization`, `WorkspaceQueryPersonalization`.
- **Betrieb:** `Alert`, `Audit`, `GitLfsLock`, `ModelVersion`, `Chat`, `ChatMessage`.

## 3. Rust-Stand

`crates/onedev-core/src/models` (18 Dateien): `user`, `project`, `issue`, `pull_request`, `build`, `workspace`, `pack`, `group`, `role`, `setting`, `access_token`, `email`, `ssh_key`, `gpg_key`, `oauth`, `audit`, `webhook`, `dashboard`, `code_comment`.

`crates/onedev-db` spiegelt das mit `models/` und `repositories/` (je ~26 Dateien) und einer Migration `migrations/001__init.sql`. Die Migration hieß ursprünglich `V1__init.sql`; das ist Flyway-Syntax und wird von `sqlx::migrate!` abgelehnt. Zusätzlich referenziert die DB-Schicht Typen, die in `onedev-core` anders heißen (`PullRequest*` gegen `MergeRequest*`), wodurch der Großteil der Compile-Fehler entsteht. Zuletzt gemessen: 268 Fehler, 136 Warnungen.

Was in Rust **fehlt**, obwohl es in OneDev zentral ist: `Iteration`, `LinkSpec`, `IssueField` als eigene Entität, `IssueSchedule`, `IssueWork`, `Stopwatch`, `IssueVote`, die gesamten `IssueChange`-/`IssueStateHistory`-Strukturen, `PullRequestReview`, `PullRequestAssignment`, `PullRequestUpdate`, `ReviewedDiff`, `PendingSuggestionApply`, `BuildDependence`, `BuildParam`, alle drei Build-Metriken, `Agent` und `AgentToken` (nur in der DB, nicht in core), `GitLfsLock`, `ProjectNumberCounter`, alle sieben `*QueryPersonalization`, `Alert`, `Chat`/`ChatMessage`.

Was in Rust **existiert, aber nicht in OneDev**: `MergeRequest` als Name (OneDev sagt durchgängig `PullRequest`), `Pipeline` und `PipelineSchedule` (OneDev hat Builds mit Job-Abhängigkeiten, keine Pipelines), dazu in den Enums Konzepte wie `Runner`, `Snippet`, `Environment`, `FeatureFlag`, `DeployKey`, `ProtectedBranch` als eigene Entitäten. In OneDev sind Branch- und Tag-Protection eingebettete Einstellungen am Projekt, kein eigener Datensatz, und es gibt keine Snippets oder Feature Flags.

## 4. Persistenz-Unterschiede

- **Eine Datenbank gegen fünf.** Vanilla erkennt den Dialekt zur Laufzeit (`HibernateConfig.isHSQLDialect()`, `isMySQLDialect()`, eigene `PostgreSQLDialect`) und unterstützt HSQLDB, MySQL/MariaDB und PostgreSQL. Rust legt sich über SQLx auf PostgreSQL fest. Das ist vertretbar, aber es macht den Import einer bestehenden Installation unmöglich, deren Daten in MySQL liegen. Das sollte als bewusste Entscheidung in `docs/plan.md` stehen, nicht als Nebenwirkung.
- **Tabellenpräfix.** `PrefixedNamingStrategy` vergibt in Vanilla ein Präfix (die Tabellen heißen dort nicht wie die Klassen). Die Rust-Migration verwendet eigene Namen. Ein Direktimport der Vanilla-Tabellen scheitert damit doppelt.
- **IDs.** `DefaultIdService` vergibt blockweise IDs (Hi/Lo-artig), dazu gibt es feste System-IDs (`User.ROOT_ID = 1`, `SYSTEM_ID = -1`, `Project.DEFAULT_ID = -1`). Rust nutzt `i64` plus `uuid`. Die negativen System-IDs muss ein Import erhalten, sonst brechen Verweise auf System-User und Default-Projekt.
- **Versionierung statt Migrations.** `data/migration` enthält 252 einzelne `migrateN`-Methoden, die Daten in-place transformieren, plus `VersionedXmlDoc` für den XML-Export. Rust hat eine einzige initiale Migration und kein Konzept für Daten-Upgrades. Für einen Neu-Start genügt das; für „bestehende Installation übernehmen" fehlt die ganze Kette.
- **Backup/Restore** läuft in Vanilla über `DefaultDataService.exportData()` als XML-Dump inkl. Versionsprüfung (`checkDataVersion`). Eine Rust-Entsprechung existiert nicht.

## 5. Service-Schicht

92 Service-Interfaces sind die Geschäftslogik: für fast jede Entität ein `*Service` mit den Regeln, die über CRUD hinausgehen — Merge-Prüfung, Workflow-Übergänge, Benachrichtigungen, Referenz-Auflösung zwischen Commit, Issue und PR (`entityreference/`, 319 Zeilen allein in `DefaultReferenceChangeService`). In Rust gibt es davon nur die Auth-Services in `onedev-auth`. `docs/task.md` Phase 1 listet die Entitäten als Datenstrukturen, aber nicht diese Services; dort liegt der eigentliche Aufwand, und er ist im Plan unterschätzt.

## 6. Abweichungen und Risiken

- **`docs/task.md` 1.1.x ist irreführend.** Alle 15 Punkte stehen auf „Todo", dabei existieren die Dateien in `onedev-core`. Der offene Punkt ist nicht das Anlegen, sondern die Korrektur am Vanilla-Modell.
- **Zwei Namensräume für dasselbe.** Solange `onedev-db` von `PullRequest` und `onedev-core` von `MergeRequest` spricht, kompiliert die Schicht nicht, und jede weitere Arbeit baut auf einer von beiden Versionen auf. Empfehlung: überall `PullRequest`, weil das der Name in OneDev, in der REST-API (`/pulls`) und in den URLs ist.
- **Pipeline-Tabellen löschen oder begründen.** `crates/onedev-db` hat `pipeline.rs` in Modell und Repository. OneDev hat keine Pipeline-Entität. Entweder entfernen oder schriftlich begründen, warum der Port hier vom Original abweicht.

## 7. Empfohlene Reihenfolge

1. Namensangleichung `MergeRequest` → `PullRequest` in `onedev-core`, danach `onedev-db` wieder kompilierbar machen.
2. Entitäten ergänzen, die von anderen Berichten blockiert werden: `Iteration`, `LinkSpec`, `IssueField`, `PullRequestReview`, `PullRequestAssignment`, `BuildDependence`, `Agent`.
3. GitLab-only-Konzepte (`Pipeline`, `Snippet`, `FeatureFlag`, `Environment`, `DeployKey`) entfernen oder in `docs/plan.md` als bewusste Abweichung dokumentieren.
4. Service-Schicht nicht vorab portieren, sondern je Vertical-Slice (Bericht 04, 05) mitziehen.
