# 06 — Plattform, REST-API, Betrieb, Integrationen

Quelle: `server/rest` (75), `server/cluster` (6), `server/replica` (2), `server/data` (10), `server/commandhandler` (10), `server/ai` (38), `server/jetty` (9), `server/oem` (5), `server/updatecheck` (3), `server/taskschedule` (3), `server/imports` (4), `server-product`, die Import-/Mail-/Notification-Plugins, gegen `crates/onedev-api`, `crates/onedev-server`, `crates/onedev-cli`.

## 1. Kurzfazit

Die Betriebs- und Integrationsschicht fehlt in Rust vollständig: `onedev-api`, `onedev-server` und `onedev-cli` sind Stubs mit je 19 Zeilen. Vanilla liefert eine REST-API mit 47 Ressourcen-Gruppen, Kommandozeilen-Werkzeuge für Backup und Passwort-Reset, Clustering, einen AI-Assistenten auf Basis von LangChain4j und Importer für sieben fremde Systeme. Positiv: fast nichts davon blockiert ein MVP. Die API ist größtenteils eine Abbildung der Entitäten, und Clustering, AI und Importer lassen sich nachordnen. Deckung: ~0 %.

## 2. Vanilla-Inventar

| Bereich | Pfad | Umfang | Zweck |
|---|---|---|---|
| REST | `server/rest` | 75 Klassen, 47 `@Path`-Gruppen | öffentliche API |
| Cluster | `server/cluster`, `server/replica` | 6 + 2 | Hazelcast-Cluster, Replikation |
| Daten | `server/data` | 10 | Backup, Restore, Import/Export, Versionsprüfung |
| CLI | `server/commandhandler` | 10 | `backup`, `restore`, `reset-admin-password`, `check-data-version`, ... |
| AI | `server/ai` | 38 | Chat mit Tool-Aufrufen über LLM-Provider |
| Bootstrap | `server/jetty`, `server/OneDev.java` | 9 + 1 | HTTP(S), Konfiguration, Start |
| OEM | `server/oem` | 5 | White-Labeling, Produktname |
| Planung | `server/taskschedule` | 3 | wiederkehrende Aufgaben |
| Update-Prüfung | `server/updatecheck` | 3 | Hinweis auf neue Versionen |
| Importer | `server/imports` + 7 Plugins | 4 + 7 Module | GitHub, GitLab, Gitea, Bitbucket Cloud, Jira Cloud, YouTrack, URL |
| Mail | `server-plugin-mail-{smtpimap,gmail,office365}` | 3 | Versand und Empfang |
| Benachrichtigung | `server-plugin-notification-{discord,slack,ntfy}` | 3 | Chat-Kanäle |
| Distribution | `server-product` | — | `server.properties`, Docker, Helm, Start-Skripte |

## 3. REST-API

Die 47 Ressourcen-Gruppen unter `server/rest/resource`:

`projects`, `repositories`, `users`, `groups`, `memberships`, `roles`, `user-authorizations`, `group-authorizations`, `base-authorizations`, `issue-authorizations`, `access-tokens`, `access-token-authorizations`, `ssh-keys`, `email-addresses`, `sso-providers`, `settings`, `server`, `issues`, `issue-comments`, `issue-votes`, `issue-links`, `issue-works`, `issue-watches`, `iterations`, `label-specs`, `project-labels`, `pulls`, `pull-request-comments`, `pull-request-labels`, `pull-request-watches`, `code-comments`, `code-comment-replies`, `builds`, `build-labels`, `artifacts`, `streaming/build-logs`, `job-runs`, `trigger-job`, `packages`, `package-blobs`, `package-labels`, `agents`, `agent-tokens`, `workspaces`, `worker`, `help`.

Das ist im Wesentlichen CRUD je Entität plus wenige spezielle Endpunkte (Build-Logs streamen, Job auslösen, Artefakte). `docs/task.md` 3.1 bildet davon nur acht Gruppen ab und lässt unter anderem Iterationen, Issue-Works, Autorisierungen, Agents und Label-Specs aus. Die API ist damit im Plan unterschätzt, aber mechanisch dankbar: mit dem richtigen Datenmodell (Bericht 02) lässt sich der größte Teil generieren. OpenAPI liefert Vanilla nicht als Spec-Datei; Rust sieht dafür `utoipa` vor (`docs/rust-rules.md` 6.1), was der bessere Weg ist.

Authentifizierung an der API läuft über Basic und Bearer (`BasicAuthenticationFilter`, `BearerAuthenticationFilter`), wobei Bearer-Tokens an `AccessTokenAuthorization` gebunden sind — also an projektbezogene Rollen, nicht an globale Scopes (Bericht 03).

## 4. Rust-Stand

Alle drei Crates sind leer. `onedev-core/src/events` hat ein Event-System mit `define_event!`, das es in dieser Form in Vanilla nicht gibt; Vanilla koppelt über seine Service-Schicht und `entityreference`. Das Event-System ist als Zusatz vertretbar, ersetzt aber keine der hier gelisteten Funktionen.

## 5. Betrieb

- **Konfiguration.** `server.properties` steuert HTTP-Port, HTTPS, Datenbank-Verbindung, Datenverzeichnis. Rust legt das über Umgebungsvariablen und `docker/ports.env` an (`docs/plan.md` Abschnitt 4). Inhaltlich gleichwertig, aber ein Import muss `server.properties` lesen können.
- **Backup/Restore und CLI.** Zehn Kommandozeilen-Befehle, darunter `reset-admin-password` und `check-data-version`. In Rust ist nichts davon vorhanden; `onedev-cli` ist der vorgesehene Ort. Für den Betrieb sind Backup und Passwort-Reset die beiden, die man wirklich braucht.
- **Clustering.** Hazelcast-basiert (`server/cluster`, 6 Klassen) mit Replikation (`replica`). Für einen einzelnen Server irrelevant und in Rust mit erheblichem Aufwand verbunden. Empfehlung: **nicht portieren**, stattdessen einen Server mit PostgreSQL. Das sollte explizit als Nicht-Ziel in `docs/plan.md` stehen.
- **Geplante Aufgaben** (`taskschedule`, 3 Klassen): Aufbewahrung, Aufräumen, geplante Builds. Klein, aber erst sinnvoll, wenn es etwas aufzuräumen gibt.
- **Update-Prüfung und OEM/White-Label.** Kosmetik, kein MVP-Bestandteil. `SubscriptionService` taucht im Code auf, die Enterprise-Module unter `server-ee` sind in diesem Tree jedoch leer — es gibt hier nichts zu portieren.

## 6. AI

`server/ai` (38 Klassen) integriert LangChain4j mit OpenAI-kompatiblen Modellen, Streaming und **Tool-Aufrufen** — das Modell kann über definierte Werkzeuge auf Issues, Code und Builds zugreifen. Einstellungen über `AiSetting` und `AiModelSetting`, fünf Admin-Seiten (`page/admin/aisetting`). Das ist ein eigenständiges Feature und hängt an keiner Kernfunktion. Für einen Port, der selbst LLM-Funktionen anbieten will, ist es interessant; für die 1:1-Abbildung des Grundsystems entbehrlich.

## 7. Integrationen

| Integration | Vanilla | Rust | Aufwand | MVP |
|---|---|---|---|---|
| Import GitHub | `server-plugin-import-github` | — | M | nein |
| Import GitLab | `server-plugin-import-gitlab` | — | M | nein |
| Import Gitea | `server-plugin-import-gitea` | — | S | nein |
| Import Bitbucket Cloud | `server-plugin-import-bitbucketcloud` | — | M | nein |
| Import Jira Cloud | `server-plugin-import-jiracloud` | — | M | nein |
| Import YouTrack | `server-plugin-import-youtrack` | — | M | nein |
| Import per URL | `server-plugin-import-url` | — | S | nein |
| Mail SMTP/IMAP | `server-plugin-mail-smtpimap` | — | M | ja, für Passwort-Reset |
| Mail Gmail / Office365 (OAuth) | 2 Plugins | — | M | nein |
| Benachrichtigung Discord/Slack/ntfy | 3 Plugins | — | S je Kanal | nein |

Mail-Versand ist die einzige Integration, die früher gebraucht wird, weil Passwort-Reset und Benachrichtigungen daran hängen.

## 8. Plugin-Architektur

Vanilla lädt Funktionen als Guice-Module; jedes der 66 Server-Plugins ist so ein Modul und wird über `ServiceLoader`-ähnliche Beiträge eingebunden. In Rust entspricht das am ehesten einem Trait pro Erweiterungspunkt (Executor, Report-Prozessor, Registry, Importer, Benachrichtigungs-Kanal) plus eine Implementierung pro Crate oder Feature-Flag. Ein dynamisches Nachladen zur Laufzeit braucht es nicht — OneDev lädt Plugins faktisch beim Start. Empfehlung: statische Registrierung, kein Plugin-System nachbauen.

## 9. Gap-Tabelle (Auszug)

| Feature | Vanilla | Rust | Status | Aufwand | Task |
|---|---|---|---|---|---|
| REST-CRUD je Entität | 47 `@Path`-Gruppen | — | fehlt | L | 3.1 (teilweise) |
| Build-Logs streamen, Job auslösen | `/streaming/build-logs`, `/trigger-job` | — | fehlt | M | 3.1.5 |
| OpenAPI-Spec | keine | `utoipa` vorgesehen | fehlt | S | X.5 |
| Basic/Bearer-Auth | 2 Filter | Extractor in `onedev-auth` | teilweise | S | 2.9 |
| Backup/Restore | `server/data`, CLI | — | fehlt | M | nein |
| CLI Passwort-Reset, Versionsprüfung | `server/commandhandler` 10 | — | fehlt | S | nein |
| Clustering/HA | Hazelcast, 8 Klassen | — | fehlt | XL | nein |
| AI-Assistent mit Tool-Aufrufen | `server/ai` 38 | — | fehlt | L | nein |
| Importer (7 Systeme) | 7 Plugins | — | fehlt | L | nein |
| Mail-Versand | `mail-smtpimap` | — | fehlt | M | nein |
| Distribution (Docker, Helm) | `server-product` | `docker/` geplant | fehlt | M | 0.5, 6.5 |

## 10. MVP-Schnitt

Ein erster benutzbarer Stand braucht aus diesem Bericht nur drei Dinge: die REST-Endpunkte für die Entitäten, die das MVP anfasst (User, Projekt, Issue, Pull Request, Build), Basic-/Bearer-Authentifizierung, und Mail-Versand für den Passwort-Reset. Backup als CLI-Befehl sollte folgen, bevor jemand echte Daten anvertraut.

Bewusst später: Clustering (durch Einzelserver mit PostgreSQL ersetzt), AI, sämtliche Importer, Chat-Benachrichtigungen, White-Labeling, Update-Prüfung. Diese sind zusammen größer als der Kern und hängen von nichts ab, was ihre Reihenfolge erzwingen würde.
