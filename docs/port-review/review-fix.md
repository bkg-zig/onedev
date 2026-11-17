# 07 — Code Review & Fixes

## Review Summary

Dieses Dokument listet Code-Review-Ergebnisse, korrigierte Probleme und offene Fragen auf der Grundlage des **vollständigen 1-to-1 Ports** (kein MVP!).

---

## 7.1 Code-Fehler und Korrekturen

### 7.1.1 `crates/onedev-git/src/services/smart_http.rs`

**Issue:** Inkonsistente Subcommand-Namen

- **Problem:** In `info_refs()` wurde `service.as_str()` verwendet, was "git-upload-pack" und "git-receive-pack" zurückgibt. Für die Git-Executable-Befehle werden jedoch nur "upload-pack" und "receive-pack" erwartet.
- **Status:** ✅ Korrigiert (Commit: siehe Git-History)
- **Fix:** Einführung einer expliziten `subcommand` Variable mit korrekten Werten:
  ```rust
  let subcommand = match service {
      GitService::UploadPack => "upload-pack",
      GitService::ReceivePack => "receive-pack",
  };
  ```

### 7.1.2 `crates/onedev-git/src/models.rs`

**Issue:** Fehlende Utility-Methoden für `LfsOperation`

- **Problem:** Das `LfsOperation`-Enum hatte zwar `parse()` und `from_str()`-ähnliche Logik, aber keine `as_str()` Methode und kein `Display` Trait-Implementierung.
- **Status:** ✅ Ergänzt
- **Fix:** 
  - Hinzufügen von `as_str()` Methode
  - Hinzufügen von `Display` Trait-Implementierung

### 7.1.3 `crates/onedev-git/src/services/hooks.rs` (neu)

**Issue:** Hook-Evaluation für Branch-Protection

- **Status:** ✅ Neu erstellt
- **Inhalt:** `HookEngine` mit Pre-Receive-Validierung für Protection Rules
- **Code-Gap:** Die vollständige Implementierung muss die Shiro-Style-Autorisierung mit `implies()`-Komposition unterstützen

---

## 7.2 Architektonische Reviews

### 7.2.1 Rollenmodell-Konsistenz ⚠️ KRITISCH

**Issue:** Das aktuelle Rollenmodell verwendet GitLab-stil `PermissionSet`-Bitflags statt OneDev's frei definierbaren Rollen mit Privilegien-Flags.

- **Status:** ❌ Nicht korrigiert
- **Auswirkung:** Inkompatibel mit Vanilla-OneDev-Implementierung
- **Blockiert:** Auth, DB, API-Schicht
- **Empfehlung:** Verwende das Vanilla-Modell mit `implies()`-Komposition (siehe `03-security-roles-auth.md`)

### 7.2.2 Git-Service-Interface

**Review:** Die `GitService`-Enum-Namen sind mit Vanilla-Standards abgeglichen, aber die String-Repräsentation muss sorgfältig getestet werden.

- **Status:** ✅ Validiert
- **Prüfpunkte:**
  - `UploadPack` → "git-upload-pack"
  - `ReceivePack` → "git-receive-pack"

### 7.2.3 Web-UI-Strategie ⚠️ KEIN MVP

**Issue:** Für 1-to-1-Port muss die Web-UI **wiederholt** werden (kein Tera/Tailwind-Neubau).

- **Status:** Nicht begonnen
- **Aufwand:** 552 HTML-Templates + 400 JS + 267 CSS
- **Empfehlung:** Wicket-HTMX-Approch mit bestehenden Templates

---

## 7.3 Performance- und Sicherheitsaspekte

### 7.3.1 LFS-Speicher

**Review:** Die LFS-Implementierung verwendet Streaming für große Objekte.

- **Empfehlung:** Memory-Boundaries für große Dateien beachten
- **Status:** Teilweise implementiert

### 7.3.2 Auth-Session-Management

**Review:** Die Session-Management-Implementierung in `onedev-auth` verwendet JWT mit Refresh-Rotation.

- **Sicherheitsaspekt:** Passwort-Migration von BCrypt zu Argon2id
- **Status:** Teilweise implementiert

---

## 7.4 Testabdeckung

| Datei | Unit-Tests | Integration-Tests | E2E-Tests |
|-------|------------|-------------------|-----------|
| `git_models.rs` | ✅ | - | - |
| `smart_http.rs` | ✅ | - | - |
| `errors.rs` | ✅ | - | - |
| `hooks.rs` | ⏳ | - | - |

### Fehlende Tests

- **Git Hook Integration**: Die Pre/Post-Receive Hooks werden nicht vollständig getestet
- **Branch-Protection**: Unit-Tests für `ProtectionRule` fehlen
- **LFS Operations**: Tests für Batch-Operationen nicht vorhanden

---

## 7.5 API-Konsistenz mit Vanilla

### 7.5.1 Git-Smart-HTTP Endpoints

| Vanilla-Pfad | Rust-Pfad | Status |
|--------------|-----------|--------|
| `GET info/refs?service=git-upload-pack` | ✅ implementiert | ✅ |
| `POST git-upload-pack` | ✅ implementiert | ✅ |
| `GET info/refs?service=git-receive-pack` | ✅ implementiert | ✅ |
| `POST git-receive-pack` | ✅ implementiert | ✅ |

### 7.5.2 LFS-Endpoint

| Vanilla | Rust | Status |
|---------|------|--------|
| `POST /info/lfs/*` | `POST /info/lfs/*` | ✅ implementiert |

---

## 7.6 Kritische Code-Gaps (FULL PORT erfordert)

### 7.6.1 Rollenmodell-Überarbeitung erforderlich

**Problem:** Das aktuelle GitLab-ähnliche Rollenmodell muss durch OneDevs Shiro-basiertes Modell ersetzt werden:

1. **Frei definierbare Rollen** mit `Role.java` (527 LOC)
2. **Privilege-Klassen** mit `implies()`-Komposition
3. **Authorization-Tabellen**:
   - `user_authorizations`
   - `group_authorizations`  
   - `base_authorizations`
   - `issue_authorizations`
   - `access_token_authorizations`

### 7.6.2 Web-UI 1-to-1 Portierung

**Erforderlich für 1-to-1 Port (KEIN MVP):**
- 552 HTML-Templates wiederholen
- 400 JS-Dateien integrieren
- 267 CSS-Dateien wiederverwenden
- HTMX/WebSocket für Interaktivität

### 7.6.3 Antlr-basierte Abfragesprachen

**Probleme aus `04-git-collaboration.md`:**
- 333 Lucene Such-Klassen
- 11 ANTLR Grammatiken für Abfragesprachen
- Autovervollständigung erforderlich

---

## 7.7 nächste Schritte

1. **Kritisch:** Rollenmodell auf Vanilla-Modell umstellen (blockiert alles andere)
2. **Hoch:** Branch-Protection und Hook-Tests schreiben
3. **Mittel:** Web-UI 1-to-1 Portierung starten
4. **Langfristig:** Antlr-basierte Abfragesprachen implementieren
5. **Unterstützung:** Remote-Agent Protokoll dokumentieren

---

## 7.8 Commit-History für aktuelle Änderungen

```
feat(git): add models, services, utils, handlers
- Implement GitService enum with UploadPack/ReceivePack
- Add LfsOperation with as_str() and Display trait
- SmartHttpService RPC improvements
- Error handling with GitError enum

feat(build): initial package, model and service interfaces
- BuildError, BuildResult types
- Build, Pack, Workspace model structures
- Service interfaces defined
```

---

*Erstellt: 2026-11-17*
*Zuletzt aktualisiert: 2026-11-17*
*Status: FULL PORT - KEIN MVP*