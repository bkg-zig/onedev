# 04 — Git-Hosting und Zusammenarbeit

Quelle: `server/git` (84), `server/ssh` (7), `server/search` (333), `server/markdown` (17), `server/mail` (13), `server/notification` (12), `server/codequality` (15), `server/xodus` (9), `server/timetracking` (2), `model/support/{issue,pullrequest,code}`, gegen `crates/onedev-git` und die Modelle in `onedev-core`.

## 1. Kurzfazit

Alles, was OneDev zum Git-Server macht, fehlt in Rust: `onedev-git` ist ein Stub mit 19 Zeilen. Es gibt kein Smart-HTTP, kein SSH, kein LFS, keine Pack-Verhandlung. Die Zusammenarbeit (Issues, Pull Requests, Code-Kommentare) ist als Datenstruktur angelegt, aber in GitLab-Begriffen und ohne die Logik dahinter. Der umfangreichste Einzelblock in diesem Bereich ist die Suche mit 333 Klassen — vor allem die Abfrage-Sprachen, für die OneDev 11 ANTLR-Grammatiken (`.g4`) mitbringt, darunter Issues, Pull Requests, Builds, Commits und Code. Deckung: ~5 %.

## 2. Vanilla-Inventar

| Bereich | Pfad | Klassen | Zweck |
|---|---|---|---|
| Git | `server/git` | 84 | Smart-HTTP, JGit-Anbindung, LFS, Diff/Merge, Hooks, Signaturen |
| SSH | `server/ssh` | 7 | SSH-Server auf MINA SSHD |
| Suche | `server/search` | 333 | Code-Suche (Lucene) und Entitäts-Abfragesprachen (ANTLR) |
| Markdown | `server/markdown` | 17 | Rendering inkl. OneDev-Erweiterungen (Issue-/Commit-Links) |
| Mail | `server/mail` | 13 | Versand, Empfang, Service Desk |
| Benachrichtigung | `server/notification` | 12 | Web- und Mail-Benachrichtigungen |
| Code-Qualität | `server/codequality` | 15 | Anzeige von Problems/Coverage am Code |
| Xodus | `server/xodus` | 9 | lokale Stores für Commit-Infos und Besuchshistorie |
| Zeiterfassung | `server/timetracking` | 2 | Schätzungen und Aufwände an Issues |
| Issue-Unterstützung | `model/support/issue` | 83 | Workflow, Felder, Boards, Transitionen |
| PR-Unterstützung | `model/support/pullrequest` | 23 | Auto-Merge, Merge-Preview, Merge-Strategien |

## 3. Rust-Stand

`crates/onedev-git` ist leer. `docs/rust-rules.md` Abschnitt 7 legt **gix** (reines Rust, kein libgit2) fest, mit Bare-Repos unter `/var/opt/onedev/git/` und Hooks als Rust-Funktionen statt Shell-Skripten. Vanilla verwendet **JGit** (Java-Git-Implementierung, sichtbar an `org.eclipse.jgit` in `data/migration`) und ruft für einzelne Operationen das `git`-Binary auf. gix ist die richtige Wahl für einen Rust-Port, aber Smart-HTTP und die Pack-Protokoll-Details sind dort der aufwendigste Teil; das ist im Plan (Phase 4, 4.1.4) korrekt als eigener Punkt erkannt.

Die Modelle `issue.rs`, `pull_request.rs`, `code_comment.rs` in `onedev-core` bilden die Grundstrukturen ab, benennen den Pull Request aber `MergeRequest` und enthalten nicht die Unter-Entitäten (Review, Assignment, Update, Watch — siehe Bericht 02).

## 4. Feature-Matrix

| Feature | Vanilla | Rust | Status | Aufwand | Task |
|---|---|---|---|---|---|
| Git Smart-HTTP (upload-pack, receive-pack) | `server/git` | — | fehlt | L | 3.4.1–3.4.3, 4.1.4 |
| SSH-Zugriff | `server/ssh` (MINA SSHD) | — | fehlt | L | nein |
| Git LFS | `GitLfsLock`-Entität, `server/git` | — | fehlt | M | 3.4.4, 4.1.5 |
| Archiv-Download (tar/zip) | `server/git` | — | fehlt | S | nein |
| Dateibaum, Blob, Blame, History, Compare | `server/git` + UI | — | fehlt | L | 4.1.2, 4.1.3 |
| Datei im Browser bearbeiten/hochladen | UI + `server/git` | — | fehlt | M | nein |
| Branch-/Tag-Verwaltung, Default-Branch | `server/git` | — | fehlt | S | 4.1.2 |
| Branch-/Tag-Protection | `model/support`, am Projekt | — | fehlt | M | 4.1.6 |
| Datei-Protection, Line-Protection | `model/support/code` | — | fehlt | M | nein |
| Required Checks (Builds, Reviews, Signaturen) | Protection-Regeln | — | fehlt | M | nein |
| Merge-Strategien + Auto-Merge | `model/support/pullrequest/AutoMerge` | — | fehlt | M | nein |
| PR: Reviewer, Approvals, Assignees | `PullRequestReview`, `PullRequestAssignment` | nur Rumpf | Stub | M | 1.1.9 |
| PR: Update/Rebase, Revert, Cherry-pick | `PullRequestUpdate` | — | fehlt | M | nein |
| Code-Kommentare mit Vorschlägen | `CodeComment`, `PendingSuggestionApply` | Rumpf | teilweise | M | 1.1.10 |
| Commit-/Issue-Referenzen, „fixes #123" | `entityreference/`, `CommitMessageFixSetting` | — | fehlt | M | nein |
| Issue: eigene Felder | `IssueField`, `model/support/issue/field` | — | fehlt | L | 1.1.8 |
| Issue: Workflow, Zustände, Transitionen | `StateSpec`, `transitionspec/` (12 Klassen) | — | fehlt | L | 1.1.8 |
| Issue: Boards | `BoardSpec` | — | fehlt | M | nein |
| Issue: Iterationen (Meilensteine) | `Iteration` | — | fehlt | M | nein |
| Zeiterfassung | `server/timetracking`, `IssueWork` | — | fehlt | M | 1.1.15 |
| Issue-Vorlagen | `IssueTemplate` | — | fehlt | S | nein |
| Abfrage-Sprachen (Issue, PR, Build, Commit, Code) | `server/search/entity`, ANTLR-Grammatiken | — | fehlt | XL | 1.2.3 |
| Gespeicherte Abfragen | `*QueryPersonalization` (7 Entitäten) | — | fehlt | S | 1.1.11 |
| Watches, Mentions, Benachrichtigungen | `IssueWatch`, `PullRequestWatch`, `server/notification` | `watch`-Repo | Stub | M | 1.1.15 |
| Eingehende Mail, Service Desk | `server/mail` | — | fehlt | L | nein |
| Markdown mit OneDev-Erweiterungen | `server/markdown` 17 | — | fehlt | M | nein |
| Code-Suche und Symbol-Navigation | `server/search/code` (Lucene) | — | fehlt | XL | 1.2.3, 3.1.11 |
| GPG-signierte Commits prüfen | `server/git`, `GpgKey` | Modell ohne Prüfung | Stub | M | nein |
| Forks, Projekt-Hierarchie, Mirror | `model/Project` | Hierarchie-Felder teilweise | teilweise | M | nein |

## 5. Die zwei großen Brocken

**Abfrage-Sprachen.** OneDev parst Ausdrücke wie `submitted by me and state is open` mit ANTLR-Grammatiken und liefert dazu Autovervollständigung (`codeassist`). Rust hat dafür nichts. Von den üblichen Parsern passt **pest** am besten, weil die Grammatik als eigene Datei lesbar bleibt und sich aus den `.g4`-Dateien übertragen lässt; `chumsky` wäre die Alternative, wenn die Fehlerausgabe für die Autovervollständigung wichtiger ist als die Nähe zur Vorlage. Das ist der aufwendigste Punkt dieses Berichts (XL), weil daran Issue-, PR-, Build- und Code-Suche hängen.

**Code-Suche.** 333 Klassen, der Löwenanteil für Lucene-Index plus Symbol-Extraktion pro Sprache. Ein Rust-Port braucht einen anderen Index (tantivy ist die naheliegende Entsprechung). Das ist eigenständig genug, um es aus dem MVP herauszunehmen, ohne den Rest zu blockieren.

## 6. Abweichungen und Risiken

- **JGit-Verhalten ist die Spezifikation.** Dinge wie Merge-Basis-Berechnung, Rename-Erkennung und die Behandlung von Submodulen müssen sich so verhalten wie JGit, sonst fallen Tests und Nutzer auf. gix bildet das nicht überall identisch ab; die Unterschiede sollten beim Port pro Funktion geprüft werden.
- **Xodus-Stores.** Commit-Infos und Besuchshistorie liegen bei Vanilla nicht in der SQL-Datenbank, sondern in eingebetteten Xodus-Datenbanken (`server/xodus`, 9 Klassen). Ein Import muss diese Stores mitlesen oder bewusst verwerfen.
- **Naming.** Dieselbe Drift wie in Bericht 02: `MergeRequest` überall auf `PullRequest` ziehen, bevor hier weitergebaut wird.

## 7. Empfohlene Reihenfolge

1. Repository lesen: Bare-Repo öffnen, Refs, Tree, Blob, Commit (gix). Das entblockt den Dateibrowser aus Bericht 01.
2. Smart-HTTP, damit ein echtes `git clone`/`git push` gegen den Server läuft.
3. Branch-Protection und Receive-Hooks, sonst ist Push ohne Regeln.
4. Pull-Request-Lebenszyklus (Review, Approval, Merge) — die meistgenutzte Zusammenarbeit.
5. Issue-Workflow mit eigenen Feldern und Transitionen.
6. Abfrage-Sprachen und Code-Suche zuletzt; sie sind groß und blockieren nichts anderes.
