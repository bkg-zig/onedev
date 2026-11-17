# OneDev Rust-Port: Gap-Analyse gegen Vanilla v17.2.1

Stand: 2026-11-17, Branch `mango`. Referenz: `vendor/vanilla-onedev` (Submodul, OneDev v17.2.1, read-only).

## Gesamtbild

| Bereich | Vanilla | Rust heute | Deckung | Bericht |
|---|---|---|---|---|
| Web-UI (Wicket) | 1502 Java-Klassen, 552 HTML, 400 JS, 267 CSS | `onedev-web`: Stub (19 Zeilen) | ~0 % | `01-web-ui.md` |
| Domain + Persistenz + Services | 104 `@Entity`, 92 Service-Interfaces, 33 Persistenz-Klassen | `onedev-core` 5,9k Z., `onedev-db` 10k Z. (kompiliert nicht) | Modelle ~25 %, Logik ~0 % | `02-model-persistence.md` |
| Security, Rollen, Auth | 57 Security-Klassen, ~40 Permission-Klassen | `onedev-auth` 4,3k Z., `permissions/` | Mechanik teilweise, **Modell falsch** | `03-security-roles-auth.md` |
| Git + Zusammenarbeit | git 84, search 333, ssh 7, markdown 17, ... | `onedev-git`: Stub; Issue/PR-Modelle GitLab-artig | ~5 % | `04-git-collaboration.md` |
| CI/CD, Pakete, Workspaces | buildspec 158, job 16, ~66 Plugin-Module | `onedev-build`: Stub; Modelle GitLab-CI-artig | ~5 % | `05-cicd-packages-workspaces.md` |
| Plattform, REST, Ops | rest 75, 47 `@Path`-Gruppen, cluster, backup, AI | `onedev-api`, `onedev-server`, `onedev-cli`: Stubs | ~0 % | `06-platform-api-ops.md` |

**Fazit:** Von ~3900 Java-Klassen sind in Rust tragfähige Ansätze nur für Domain-Modelle, die DB-Schicht und Authentifizierung vorhanden — und die weichen semantisch vom Original ab (GitLab- statt OneDev-Modell). Sechs Crates sind leere Stubs. Eine 1:1-Portierung mit identischer Web-UI ist ein Mehrpersonen-Jahresprojekt; ein brauchbares MVP ist in Monaten machbar, wenn der Funktionsumfang beschnitten wird.

## Die drei Entscheidungen, die alles andere blockieren

1. **Rollenmodell** — Vanilla hat frei definierbare Rollen mit feingranularen Privilegien (Shiro-`implies`-Ketten), Rust hat feste GitLab-Rollen als Bitflags. Ohne diese Entscheidung sind `permissions/`, `onedev-auth` und die DB-Tabellen auf Sand gebaut. Details und Referenztext: `03-security-roles-auth.md`.
2. **Web-UI-Strategie** — 1:1-Look bedeutet, die vorhandenen 552 HTML + 400 JS + 267 CSS weiterzuverwenden (serverseitiges Templating + HTMX/WebSocket), nicht neu zu zeichnen. `docs/rust-rules.md` sagt Tera + Tailwind + dx; das reproduziert das OneDev-UI nicht. Details: `01-web-ui.md`.
3. **CI-Modell** — Vanilla ist Build-Spec/Job/Step/Trigger, Rust modelliert GitLab-Pipelines/Stages/Runner. Eines muss weg. Details: `05-cicd-packages-workspaces.md`.

## Weitere strukturelle Abweichungen

- **Naming-Drift:** Rust sagt `MergeRequest*`, `Pipeline`, `Runner`, `Snippet`, `Environment`, `FeatureFlag`, `DeployKey` — nichts davon existiert so in OneDev (`PullRequest`, `Build`, `Agent`, kein Snippet/FeatureFlag/Environment-Konzept).
- **Datenbank:** Vanilla ist Hibernate mit HSQLDB/MySQL/MariaDB/PostgreSQL und Tabellenpräfix `o_`, Rust ist SQLx nur mit PostgreSQL. Ein Import einer bestehenden OneDev-Installation ist damit nicht vorgesehen — das sollte bewusst entschieden werden.
- **`docs/plan.md` und `docs/task.md` sind veraltet:** Sie verweisen auf `vendor/bkg-core-dev` und `rust/` (beides existiert nicht; die Referenz liegt in `vendor/vanilla-onedev`, der Workspace im Root), listen `dx build --web` als UI-Weg und behandeln das Rollenmodell als GitLab-Port. `docs/task.md` 1.1.x steht auf „Todo", obwohl die Modelle in `onedev-core` bereits angelegt sind.
- **`onedev-db` kompiliert nicht** (zuletzt 268 Fehler, nach einem abgebrochenen Fix-Lauf). Solange das so ist, ist die DB-Schicht nicht belastbar.

## Empfohlene Reihenfolge

1. Die drei Entscheidungen oben treffen (sonst wird weiter in die falsche Richtung gebaut).
2. `onedev-db` kompilierbar machen und am echten OneDev-Schema ausrichten.
3. Rollen-/Permission-Modell in `onedev-core` + `onedev-auth` auf das Vanilla-Modell umstellen.
4. Danach vertical: ein Projekt mit Git-Smart-HTTP, Issue-Tracking und einem Build-Job — bevor die Breite (8 Paket-Registries, 25 Report-Plugins, Importer, AI) angegangen wird.
