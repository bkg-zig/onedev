# 01 — Web-UI (1-to-1 Port erforderlich!)

Quelle: `server-core/.../web/**` (1502 Java-Klassen plus die daneben liegenden HTML/JS/CSS-Dateien), gegen `crates/onedev-web` und `crates/onedev-server`.

## 1. Kurzfazit ⚠️ KEIN MVP - VOLLE 1-to-1 PORTIERUNG

**WICHTIG:** Dies ist ein **vollständiger 1-to-1 Port**, kein MVP! Die Web-UI muss **wiederholt** werden:

- **1502 Java-Klassen** → Rust-Äquivalente erforderlich
- **552 HTML-Templates** → Muss mit Wiederholung/portiert werden (nicht neu mit Tera/Tailwind gebaut!)
- **400 JS-Dateien** → Integrität der bestehenden Interaktivität erhalten
- **267 CSS-Dateien** → Konsistente Styles behalten

`onedev-web` und `onedev-server` sind aktuell **leere Stubs** (19 Zeilen).

Der Stack in `docs/rust-rules.md` Abschnitt 9 (Tera + Tailwind + dx) ist **KEIN** die richtige Weg für 1-to-1 Port! Für 1-to-1 muss das vorhandene Wicket-basierte UI wiederholt werden mit HTMX/WebSocket für Interaktivität.

## 2. Vanilla-Inventar

Java-Klassen nach Paket (`web/`, LOC in Klammern):

| Paket | Klassen | LOC | Zweck |
|---|---|---|---|
| `page/` | 457 | 68 012 | alle Seiten |
| `component/` | 574 | 64 367 | wiederverwendbare Widgets |
| `editable/` | 222 | 20 118 | Annotation-getriebene Formulare |
| `behavior/` | 50 | 5 479 | AJAX-/Client-Verhalten |
| `asset/` | 50 | 1 377 | gebündelte JS/CSS-Bibliotheken |
| `translation/` | 14 | 45 653 | Übersetzungen |
| `util/`, `resource/`, `websocket/`, `mapper/`, `resourcebundle/`, `avatar/`, übrige | 135 | ~9 000 | Routing, i18n, WebSocket, Avatare |

Nicht-Java-Dateien neben den Klassen: 552 HTML, 400 JS, 267 CSS, 215 SVG, 168 bcmap, 24 TTF, 20 WOFF2, 20 WOFF, 14 PNG, 2 WASM. Ein großer Teil entfällt auf eingebettetes **PDF.js** (inkl. WASM und Fonts) für die Dateivorschau.

Seiten nach Bereich (`web/page`, 457): `project` 243, `admin` 111, `user` 20, `my` 17, `help` 14, `oauth` 12, `security` 11, `layout` 8, `base` 6, `error` 3, `simple` 2, dazu je eine Seite für `builds`, `issues`, `pullrequests`, `packs`, `workspaces`, `store`, `serverinit` und `HomePage`.

Innerhalb `project` (243): `blob` 71, `setting` 47, `issues` 31, `pullrequests` 26, `builds` 20, `workspaces` 11, `stats` 6, `commits` 5, `codecomments` 4, `tags` 3, `branches` 3, `wiki` 2, `packs` 2, `compare` 2, `overview` 2, `imports` 1. Allein der Datei-Browser ist mit 71 Klassen größer als die meisten Rust-Crates.

Admin (111): `issuesetting` 18, `emailtemplates` 18, `buildsetting` 17, `groupmanagement` 7, `usermanagement` 5, `aisetting` 5, `workspaceprovisioner` 4, `ssosetting` 4, `rolemanagement` 3, `authenticator` 3, dazu Einzelbereiche für Backup, Branding, Service-Desk, Mail, GPG, SSH, OAuth, Security, Performance, Serverlog.

Komponenten nach Bereich (574, Auszug): `user` 74, `issue` 74, `diff` 27, `pullrequest` 24, `project` 24, `markdown` 23, `commandpalette` 22, `select2` 18, `build` 14, `savedquery` 13, `iteration` 12, `workspace` 11, `job` 11, `commit` 11, `branch` 10, `modal` 9, `datatable` 7, `chart` 7, `tabbable` 7, `pipeline` 5, `terminal` 3, `symboltooltip` 3, `suggestionapply` 3, `gitsignature` 3.

Die schwersten Einzelstücke, an denen der Aufwand hängt: Diff-Viewer (27 Klassen), Markdown-Editor mit Vorschau (23), Kommando-Palette (22), Datei-/Blob-Browser inkl. Blame und Syntax (71 Seiten plus PDF.js), Issue-Board, Build-/Pipeline-Graph, Commit-Graph, Terminal über WebSocket (3), Autocomplete für die Query-Sprachen (`codequeryoption`, `select2`).

## 3. Rust-Stand

`crates/onedev-web/src` und `crates/onedev-server/src` enthalten je eine Datei mit 19 Zeilen. Es gibt keine Templates, keine Routen, kein Asset-Build. `docs/plan.md` 3.5 beschreibt das UI als „dx build --web" mit Dashboard, Project-List und einem Stack-Tab — das deckt von den 457 Seiten nur einen Bruchteil ab und erwähnt die tragenden Widgets (Diff, Blob, Board, Terminal) nicht.

## 4. Die `editable`-Frage

`web/editable` (222 Klassen) ist der Teil, der die UI-Architektur entscheidet. OneDev beschreibt Formulare nicht von Hand, sondern über Annotationen an den Modellklassen: `@Editable`, `@ChoiceProvider`, `@ShowCondition`, `@DependsOn`, `@NotNull`. Zur Laufzeit wird daraus eine Eingabemaske mit bedingten Feldern, Auswahl-Listen und Validierung — verwendet für Rollen, Build-Spec, Issue-Felder, Servereinstellungen, fast jeden Admin-Dialog. Das ist der Grund, warum 111 Admin-Seiten mit so wenig seiten-spezifischem Code auskommen.

Eine 1:1-Übernahme heißt: Rust braucht ein Äquivalent, das aus Typen plus Metadaten Formulare erzeugt (Schema aus `schemars`/`utoipa` oder ein eigenes Derive), inklusive bedingter Felder (`@DependsOn`) und dynamischer Auswahlquellen (`@ChoiceProvider`). Ohne das wird jede der Admin-Seiten Handarbeit, und der Aufwand der Admin-Oberfläche vervielfacht sich.

## 5. Interaktives Verhalten

- **Zustandsbehaftete Komponenten + AJAX:** Wicket hält Komponentenzustand in der Session und tauscht Markup-Fragmente aus (`behavior/` 50 Klassen, `ajaxlistener/` 9). Ein zustandsloses Template-System bildet das nur mit explizitem Ersatz ab.
- **WebSocket** (`websocket/` 16 Klassen): Build-Logs, Terminal, Fortschrittsanzeigen laufen als Push.
- **i18n:** `translation/` mit 45 653 Zeilen und `resourcebundle/` — mehrere Sprachen, nicht nur Englisch.
- **Uploads** (`upload/`, `dropzonefield`), **Avatare** (`avatar/` plus `avatarupload`), **OpenGraph**-Metadaten.
- **PDF-Vorschau** über eingebettetes PDF.js mit WASM.

## 6. Architektur-Optionen

| Option | Aufwand | Bewertung |
|---|---|---|
| A — Vorhandenes HTML/JS/CSS weiterverwenden, serverseitig mit Tera rendern, Interaktion über HTMX und WebSocket | XL | Einziger Weg zu „gleiche Web-UI". Die 552 Templates sind die Spezifikation; Wicket-Bindungen müssen in HTMX-Attribute übersetzt werden. |
| B — SPA (Leptos/React) über die REST-API, UI neu gestaltet | XL | Sauberer technisch, aber es ist ein neues Produkt, kein Port. Verfehlt das Ziel. |
| C — Tera + Tailwind + dx laut `docs/rust-rules.md` | L für ein MVP, XL für Parität | Baut nur die in `docs/plan.md` 3.5 genannten Ansichten. Für ein internes MVP vertretbar, als 1:1-Port ungeeignet. |

Empfehlung: Option A für alles, was der User sieht (Datei-Browser, Diff, Issues, PRs, Builds), und das `editable`-Äquivalent als erste UI-Infrastruktur, weil fast jeder Dialog davon abhängt. Option C nur, wenn das Ziel explizit auf ein reduziertes MVP zurückgenommen wird — dann muss `docs/rust-rules.md` Abschnitt 9 das auch sagen.

## 7. Gap-Tabelle

| Bereich | Vanilla | Rust | Status | Aufwand | Task |
|---|---|---|---|---|---|
| Routing / URL-Schema | `web/mapper`, `WebApplication` | — | fehlt | M | nein |
| Projekt-Dateibrowser (Blob, Blame, History) | `page/project/blob` 71 | — | fehlt | XL | nein |
| Diff-Viewer | `component/diff` 27 | — | fehlt | L | nein |
| Issues, Board, Iterationen | `page/project/issues` 31, `component/issue` 74 | — | fehlt | XL | 3.5.4 |
| Pull Requests | `page/project/pullrequests` 26 | — | fehlt | L | 3.5.4 |
| Builds, Pipeline-Graph, Logs | `page/project/builds` 20, `component/pipeline` | — | fehlt | L | 3.5.4 |
| Markdown-Editor | `component/markdown` 23 | — | fehlt | M | nein |
| Workspaces, Terminal | `page/project/workspaces` 11, `component/terminal` | — | fehlt | L | 4.2.7 |
| Admin-Oberfläche | `page/admin` 111 | — | fehlt | XL | 3.5.6 |
| Formular-Framework | `editable/` 222 | — | fehlt | L | nein |
| Login, OAuth, Security-Seiten | `page/security` 11, `page/oauth` 12 | — | fehlt | M | 3.2 |
| i18n | `translation/` 45k LOC | — | fehlt | M | nein |
| Dashboard, „My"-Seiten | `page/my` 17 | — | fehlt | M | 3.5.2 |

## 8. Abhängigkeiten

Jede Seite braucht vorher: Authentifizierung und das korrekte Rollenmodell (Bericht 03), die REST- oder Server-Endpoints (Bericht 06) und die jeweilige Domänenlogik (Berichte 04, 05). Sinnvolle erste Scheibe: Login + Projektliste + Dateibrowser für ein einzelnes Repository, weil der dafür nötige Git-Unterbau (Bericht 04) sowieso die Grundlage aller weiteren Ansichten ist.
