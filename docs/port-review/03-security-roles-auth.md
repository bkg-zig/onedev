# 03 — Security, Rollenmodell, Authentifizierung (1-to-1 PORT)

Quelle: `vendor/vanilla-onedev` v17.2.1 (`server/security`, `model`, `model/support/role`, `model/support/administration`), gegen `crates/onedev-core/src/permissions`, `crates/onedev-core/src/models/role.rs`, `crates/onedev-auth`.

## 1. Kurzfazit ⚠️ KRITISCH - BLOCKIERT ALLE SCHICHTEN

**WICHTIG:** Dies ist ein **vollständiger 1-to-1 Port**, kein MVP!

**DIE GRÖSSTE FEHLLENTSCHEID:**

| Vanilla (OneDev) | Rust (aktuell) | Folge |
|-----------------|----------------|-------|
| Frei definierbare Rollen mit `implies()`-Komposition | Feste GitLab-Rollen als Bitflags | **Blockiert DB, API, Auth, Permissions** |

Das Vanilla-Berechtigungsmodell ist **kein festes Rollenset**, sondern ein Shiro-basiertes Privilegien-System: Rollen sind frei definierbare Datensätze, jedes Privileg ist eine eigene Klasse mit `implies()`, und die effektive Berechtigung entsteht aus einer Kette (Administrator → Gruppen-Flags → User-/Gruppen-/Default-Autorisierungen → Projekt-Vererbung).

**Rust modelliert falscherweise** feste GitLab-Rollen als `PermissionSet`-Bitflags.

Die Mechanik in `onedev-auth` (Argon2id, JWT, TOTP) ist solide und an mehreren Stellen sogar strenger als Vanilla, sitzt aber auf dem **falschen Modell**. Zusätzlich geben die `PermissionChecker::check_*`-Methoden in `onedev-core` pauschal `true` zurück.

**Deckung:** Authentifizierung ~40 %, Autorisierung ~15 %, und der vorhandene Teil ist semantisch **falsch**.

### ⚠️ Dies ist die teuerste Fehlentscheidung im Port!

**BLOCKIERT:** Rechte-, DB-, API-, und Auth-Schicht – **alles**!

### 1-to-1 Portierung erfordert:

1. **Vanilla-Modell übernehmen:** Frei definierbare Rollen mit `implies()`-Komposition
2. **Permission-Trait implementieren** mit korrekter Semantik
3. **Alle Autorisierungs-Tabellen anlegen:**
   - `user_authorizations`
   - `group_authorizations`  
   - `base_authorizations`
   - `issue_authorizations`
   - `access_token_authorizations`
4. **PermissionChecker-Implementierung** ohne Stub-Returns
5. **Role-Reset**: GitLab-Rollen entfernen, Vanilla-Rollen übernehmen

> **KEIN MVP:** Ohne korrektes Rollenmodell funktioniert kein Vanilla-kompatibler Port!

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

- `crates/onedev-auth` (25 Dateien, ~4,3k Zeilen): `auth_service.rs` (Login, Sessions, Passwort-Reset), `authorization_service.rs`, `models/{password,token,two_factor}.rs`, Axum-Extractor/Middleware. Passwort-Hashing Argon2id mit m=19456, t=2, p=1.
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

## 5. Empfohlene Reihenfolge für 1-to-1 Port

1. Rollenmodell auf Vanilla-Modell umstellen (KRITISCH!)
2. Permission-Trait mit implizierender Logik implementieren
3. Autorisierungs-Tabellen anlegen und mit Daten füllen
4. PermissionChecker-Implementierung testen
5. Auth-Service mit korrektem Rollenmodell verknüpfen