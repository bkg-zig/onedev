# 03 — Security, Rollenmodell, Authentifizierung

Quelle: `vendor/vanilla-onedev` v17.2.1 (`server-core/.../security`, `model`, `model/support/role`, `model/support/administration`), gegen `crates/onedev-core/src/permissions`, `crates/onedev-core/src/models/role.rs`, `crates/onedev-auth`.

## 1. Kurzfazit

Das Vanilla-Berechtigungsmodell ist **kein festes Rollenset**, sondern ein Shiro-basiertes Privilegien-System: Rollen sind frei definierbare Datensätze, jedes Privileg ist eine eigene Klasse mit `implies()`, und die effektive Berechtigung entsteht aus einer Kette (Administrator → Gruppen-Flags → User-/Gruppen-/Default-Autorisierungen → Projekt-Vererbung). Rust modelliert stattdessen **feste GitLab-Rollen** (`Guest`…`Owner`) als `PermissionSet`-Bitflags. Die Mechanik in `onedev-auth` (Argon2id, JWT, TOTP) ist solide und an mehreren Stellen sogar strenger als Vanilla, sitzt aber auf dem falschen Modell. Zusätzlich geben die `PermissionChecker::check_*`-Methoden in `onedev-core` pauschal `true` zurück. Deckung: Authentifizierung ~40 %, Autorisierung ~15 %, und der vorhandene Teil ist semantisch falsch. **Das ist die teuerste Fehlentscheidung im Port, weil Rechte-, DB- und API-Schicht davon abhängen.**

## 2. Vanilla-Inventar

| Bereich | Pfad | Umfang | Zweck |
|---|---|---|---|
| Security-Kern | `server/security/` | 57 Klassen | `SecurityUtils`, `DefaultAuthorizingService` (Shiro-Realm), `DefaultAuthenticatingService`, Basic-/Bearer-Filter, Passwort, Remember-Me |
| Permissions | `server/security/permission/` | ~40 Klassen | je Privileg eine Klasse, komponiert über `implies()` |
| Rollen-Modell | `model/Role.java` (527 Z.) | 1 | alle Privilegien-Flags einer Rolle |
| Autorisierungen | `model/{Base,User,Group,Issue,Link,AccessToken}Authorization.java` | 6 | Zuordnung Rolle ↔ Projekt/User/Gruppe/Issue/Link/Token |
| Gruppe | `model/Group.java` | 1 | `administrator`, `createRootProjects`, `enforce2FA` |
| User | `model/User.java` | 1 | Typen `ORDINARY/SERVICE/AI`, `ROOT_ID=1`, `SYSTEM_ID=-1`, `UNKNOWN_ID=-2` |
| Settings | `model/support/administration/SecuritySetting.java` | 1 | anonyme Nutzung, Selbstregistrierung, 2FA-Pflicht, internes Login |
| Rollen-Hilfstypen | `model/support/role/` | 8 | `CodePrivilege`, `JobPrivilege`, `IssueFieldSet`, ... |
| Plugins | `server-plugin-sso-{openid,discord}`, `server-plugin-authenticator-ldap` | 3 Module | SSO + LDAP |

## 3. Rust-Stand

- `crates/onedev-auth` (25 Dateien, ~4,3k Zeilen): `auth_service.rs` (Login, Sessions, Passwort-Reset), `authorization_service.rs`, `models/{password,token,two_factor}.rs`, Axum-Extractor/Middleware. Passwort-Hashing Argon2id mit m=19456, t=2, p=1; TOTP mit SHA1/256/512; JWT mit Access-/Refresh-Token.
- `crates/onedev-core/src/models/role.rs`: `PermissionSet`-Bitflags und ein Enum fester Rollen (`Guest`, `Reporter`, `Developer`, `Maintainer`, `Owner`, `Admin`).
- `crates/onedev-core/src/permissions/mod.rs`: `Permission`-Trait mit `implies()`, `ProjectPermission`, `JobPermission`, `BasePermission`, `SystemPermission`. Die `PermissionChecker::check_*`-Methoden sind **Platzhalter und geben `true` zurück** — jede darauf aufbauende Prüfung ist wirkungslos.
- DB: `crates/onedev-db/src/{models,repositories}/{role,authorization,membership}.rs` bilden Rollen als feste Mengen ab, nicht als Privilegien-Datensätze.

## 4. Rollenmodell (Vanilla) — Referenztext für die Docs

Eine **Rolle** ist in OneDev kein vorgegebener Name, sondern ein frei anlegbarer Datensatz (`model/Role.java`). Sie trägt:

- `manageProject` — Projektverwaltung. Im Shiro-Modell impliziert `ManageProject` **jedes** Privileg (`implies()` gibt `true` zurück), also auch Projekt löschen, transferieren, Einstellungen ändern.
- `managePullRequests` — impliziert `WriteCode`.
- `manageCodeComments` — impliziert `WriteCode`.
- `manageBuilds` — impliziert `ManageJob` für alle Jobs plus `UploadCache`.
- `manageWorkspaces` — impliziert `CreateWorkspaces` und `WriteCode`.
- `manageIssues` — impliziert Zeiterfassung, vertrauliche Issues, Felder bearbeiten, Links bearbeiten, fremde Felder bearbeiten, einplanen.
- `scheduleIssues`, `accessTimeTracking`, `accessConfidentialIssues`.
- `createChildren` — Unterprojekte anlegen.
- `codePrivilege` — Enum `NONE | READ | WRITE`. `WRITE` impliziert `CreateWorkspaces`, `READ` impliziert `AccessProject`.
- `packPrivilege` — analog für Paket-Registries.
- `jobPrivileges` — Liste aus Job-Namensmustern plus Privileg (`ManageJob`, `RunJob`, `AccessBuildLog`, `AccessBuildReports` mit Report-Namensmustern, `AccessBuildPipeline`).
- `issueFieldSet` — welche Issue-Felder die Rolle bearbeiten darf (`AllIssueFields`, `IncludeIssueFields`, `ExcludeIssueFields`).
- `editableIssueLinks` — welche Issue-Verknüpfungen editierbar sind.

**Eingebaute Rolle:** Beim ersten Start legt `DefaultDataService` genau eine Rolle an: `Project Owner` (`Role.OWNER_ID`) mit `manageProject=true`. Es gibt keine Rollen `Guest`, `Reporter`, `Developer`, `Maintainer`. Weitere Rollen definiert der Administrator selbst.

**Effektive Berechtigung** eines Subjekts auf ein Projekt (`DefaultAuthorizingService`, `SecurityUtils`), in dieser Reihenfolge:

1. System-/Root-User (`User.ROOT_ID`, `User.SYSTEM_ID`) und Mitglieder einer Gruppe mit `administrator=true` dürfen alles (`SystemAdministration` impliziert alles).
2. Gruppen-Flags `createRootProjects` und `enforce2FA` wirken global, nicht pro Projekt.
3. Autorisierungen: `UserAuthorization` (User + Projekt + Rolle), `GroupAuthorization` (Gruppe + Projekt + Rolle) und `BaseAuthorization` (Default-Rolle eines Projekts, gilt für jeden mit Projektzugriff, auch anonym).
4. `IssueAuthorization` gibt einem User Rechte auf **ein einzelnes** Issue; `LinkAuthorization` regelt Issue-Verknüpfungen pro Rolle; `AccessTokenAuthorization` bindet ein Access-Token an Projekt + Rolle.
5. Projekt-Vererbung: `Project.getSettingsParent()` liefert das Elternprojekt, ohne aktives Abo ersatzweise das Default-Projekt (`Project.DEFAULT_ID = -1`). Branch-/Tag-Protection und Default-Rollen werden über diese Kette geerbt.
6. Sonderregeln in `SecurityUtils`: Dateirechte (`canReadFile`) berücksichtigen den Wiki-Ordner; ein Code-Kommentar darf auch vom Autor, vom PR-Reviewer oder vom PR-Ersteller geschlossen werden (`canChangeStatus`).
7. `Project.isMaintenance()` sperrt alle `WritePermission`-Privilegien, außer der User hat `MaintainProject`.

```
Subjekt
  ├─ root / system / Gruppe.administrator ──────────────► alles
  ├─ Gruppe.createRootProjects / enforce2FA ────────────► globale Flags
  └─ pro Projekt:
       UserAuthorization ──┐
       GroupAuthorization ─┼─► Rolle(n) ─► Privilegien ─► implies()-Kette
       BaseAuthorization ──┘   (Default-Rolle, auch anonym)
       IssueAuthorization ────► Einzel-Issue
       AccessTokenAuthorization ► Token statt User
       └─ Settings-Vererbung: Elternprojekt, sonst Default-Projekt (-1)
       └─ Maintenance-Modus: WritePermission nur mit MaintainProject
```

**Abbildung in Rust (Vorschlag):** `roles`-Tabelle mit den Flags als Spalten (Booleans + `code_privilege`/`pack_privilege` als Enum + `job_privileges jsonb`), Tabellen `user_authorizations(user_id, project_id, role_id)`, `group_authorizations(group_id, project_id, role_id)`, `base_authorizations(project_id, role_id)`, `issue_authorizations(user_id, issue_id)`, `access_token_authorizations(token_id, project_id, role_id)`. Privileg-Prüfung als eine Funktion `implies(held, required)`, nicht als Bitflag-Enum. Gruppen-Flags (`administrator`, `create_root_projects`, `enforce_2fa`) gehören an `groups`, nicht an Rollen.

## 5. Gap-Tabelle

| Feature | Vanilla | Rust | Status | Aufwand | Task |
|---|---|---|---|---|---|
| Frei definierbare Rollen mit Privilegien-Flags | `Role.java` | feste Rollen `Guest`…`Owner` | falsch | L | 1.1.2, 1.2.1 |
| `implies()`-Komposition je Privileg | `security/permission/*` | `Permission`-Trait vorhanden, Semantik abweichend | teilweise | M | 1.2.1 |
| User-/Gruppen-/Default-Autorisierung | `UserAuthorization`, `GroupAuthorization`, `BaseAuthorization` | keine Entsprechung | fehlt | M | 2.5 |
| Issue-/Link-/Token-Autorisierung | `IssueAuthorization`, `LinkAuthorization`, `AccessTokenAuthorization` | fehlt | fehlt | M | nein |
| Settings-Vererbung über Elternprojekt | `Project.getSettingsParent()` | fehlt | fehlt | M | nein |
| Maintenance-Modus sperrt Writes | `MaintainProject` | fehlt | fehlt | S | nein |
| Gruppe: administrator, createRootProjects, enforce2FA | `Group.java` | `GroupVisibility`-Enum (GitLab) | falsch | S | 1.1.1 |
| Effektive-Rechte-Berechnung + Cache | `DefaultAuthorizingService`, `AuthorizationInfoCache` | `authorization_service.rs` auf Bitflags | falsch | L | 1.2.2, 2.4, 2.5 |
| Permission-Checks | `SecurityUtils.canXxx` | `PermissionChecker` gibt `true` zurück | falsch | M | 2.3 |
| Passwort-Hashing | BCrypt (`DefaultPasswordService`) | Argon2id, konstante Parameter | abweichend, akzeptabel | S | 2.6 |
| Access-Token | Wert nur als Hash, projektbezogene Rollen | JWT (RS256) + Refresh-Rotation | abweichend | M | 2.7 |
| 2FA TOTP + Recovery-Codes | `TwoFactorAuthentication`, Login-Flow | `two_factor.rs`, SHA1/256/512 | teilweise | S | 2.8 |
| 2FA-Pflicht (global + per Gruppe) | `SecuritySetting`, `Group.enforce2FA` | fehlt | fehlt | S | nein |
| Remember-Me | verschlüsseltes Cookie (`CryptoService`) | Session-basiert | abweichend | S | 2.10 |
| Basic + Bearer an API und Git-HTTP | `BasicAuthenticationFilter`, `BearerAuthenticationFilter` | Extractor vorhanden, Git-HTTP fehlt | teilweise | M | 2.9 |
| SSH-Keys | `SshKey.java` | Modell vorhanden, keine Prüfung | Stub | M | 1.1.13 |
| GPG-Keys + Signaturprüfung | `GpgKey.java`, `BaseGpgKey` | Modell vorhanden, keine Prüfung | Stub | M | 1.1.13 |
| SSO OpenID + Discord | `server-plugin-sso-*` | `SsoAccount`-Modell | fehlt | L | 1.1.14 |
| LDAP | `server-plugin-authenticator-ldap` | fehlt | fehlt | L | nein |
| Selbstregistrierung, internes Login abschaltbar | `SecuritySetting` | fehlt | fehlt | S | nein |
| Brute-Force-Schutz | (Annahme: nicht im Kern) | `governor` im Workspace | offen | S | nein |

Aufwand: S ≤ 1 Tag, M ≤ 1 Woche, L ≤ 1 Monat, XL > 1 Monat, je ein Entwickler.

## 6. Abweichungen und Risiken

- **Privileg-Eskalation im Rust-Code:** `SystemPermission::AdminSystem` und die Abbildung von `ManageProject` auf „alle Write-Permissions" sind dem Vanilla-Verhalten zwar ähnlich (`implies() == true`), aber in Kombination mit `PermissionChecker`, der konstant `true` liefert, ist die gesamte Prüfung derzeit ohne Wirkung. Das darf so nicht in einen laufenden Server.
- **GitLab-Rollen löschen, nicht erweitern:** `Guest/Reporter/Developer/Maintainer/Owner/Admin` kommen in OneDev nicht vor. Sie in den Tabellen zu lassen erzeugt zwei parallele Rechte-Systeme.
- **Access-Token-Semantik:** Vanilla speichert nur einen Hash (`PROP_VALUE_HASH`, unique) und hängt Rechte über `AccessTokenAuthorization` an. Rust verwendet JWT mit Refresh-Rotation (`docs/rust-rules.md` 5.2). Beides ist vertretbar, aber ein Import bestehender Tokens und die projektbezogene Token-Autorisierung gehen mit JWT verloren. Entscheidung nötig.
- **Passwort-Kompatibilität:** Bestehende OneDev-Installationen haben BCrypt-Hashes (`$2a$`). Argon2id ist die bessere Wahl, aber ein Import muss BCrypt lesen und beim nächsten Login migrieren können.
- **Abo-abhängige Vererbung:** `getSettingsParent()` fällt nur bei aktivem `SubscriptionService` auf das Default-Projekt zurück. `server-ee` ist in diesem Tree leer — das Verhalten sollte für den Port explizit festgelegt werden (Empfehlung: Vererbung immer über das Elternprojekt, Default-Projekt nur als explizite Vorlage).

## 7. Empfohlene Reihenfolge

1. Entscheidung: Vanilla-Modell 1:1 übernehmen (Empfehlung) oder bewusst vereinfachen — dokumentiert in `docs/plan.md`.
2. `PermissionSet`-Bitflags und die festen Rollen in `role.rs` durch das Privilegien-Modell ersetzen; `PermissionChecker`-Platzhalter entfernen.
3. DB-Tabellen `roles`, `user_authorizations`, `group_authorizations`, `base_authorizations`, `issue_authorizations`, `access_token_authorizations` anlegen, Gruppen-Flags an `groups`.
4. `authorization_service.rs` auf die Kette aus Abschnitt 4 umschreiben, mit Tests gegen die `implies()`-Tabelle aus `security/permission/`.
5. Danach erst 2FA-Pflicht, Token-Autorisierung, SSO und LDAP.
