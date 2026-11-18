# OneDev Rust-Port: Fortschritts- und Entscheidungsdokumentation

## Ziel

Vollständige 1-to-1 Portierung von OneDev v17.2.1 von Java zu Rust. **KEIN MVP!**

---

## Status-Übersicht

| Bereich | Vanilla | Rust | Deckung | Status |
|---------|---------|------|---------|--------|
| Web-UI | 1502 Klassen, 552 HTML, 400 JS, 267 CSS | Stub | ~0% | ⏳ Offen |
| Domain/Persistenz | 104 Entity, 92 Service | 5,9k Zeilen (kompiliert nicht) | ~25% | ⚠️ Teilweise |
| Security | 57 Klassen, 40 Permissions | 4,3k Zeilen (falsches Modell) | ~15% | ⛔ Blockierend |
| Git | 84 Klassen | Implementiert | ~15% | ✅ Fortschritt |
| Build | buildspec 158, job 16, ~66 Plugins | Partial | ~5% | ⚠️ Teilweise |
| API | 75 Klassen, 47 Paths | Stub | ~0% | ⏳ Offen |

---

## Akzeptanzkriterien

| AC-ID | Kriterium | Status | Beweis |
|-------|-----------|--------|--------|
| AC-001 | Rollenmodell auf Vanilla-Shiro umgestellt | ❌ | - |
| AC-002 | Build-Spec-Parser implementiert | ❌ | - |
| AC-003 | Web-UI Template-Wiederholung abgeschlossen | ❌ | - |
| AC-004 | Alle 47 REST-API-Endpunkte implementiert | ❌ | - |
| AC-005 | Git-Smart-HTTP mit Hooks funktionsfähig | ⚠️ Teil | PR #123 |
| AC-006 | Datenbank kompiliert und migrierbar | ❌ | - |
| AC-007 | Build-Spec-Executor-Plugins vorhanden | ❌ | - |
| AC-008 | Search-Funktionalität implementiert | ❌ | - |

---

## Schlüssel-Entscheidungen

### Entscheidung 1: Rollenmodell (KRITISCH)

- **Status:** Offen
- **Auswahl:** Vanilla-Shiro `implies()`-Komposition statt GitLab-Bitflags
- **Begründung:** Vanilla OneDev verwendet frei definierbare Rollen mit Privilegien-Komposition
- **Auswirkung:** Blockiert Auth, DB, API-Schicht
- **Quelle:** `docs/port-review/03-security-roles-auth.md`

### Entscheidung 2: Web-UI-Strategie (KRITISCH)

- **Status:** Offen
- **Auswahl:** Wicket-HTMX Wiederholung der 552 HTML-Templates
- **Begründung:** Tera/Tailwind/dx reicht nicht für 1-to-1 Port
- **Auswirkung:** UI kann nicht kompatibel sein
- **Quelle:** `docs/port-review/01-web-ui.md`

### Entscheidung 3: Build-Modell

- **Status:** Falsch implementiert
- **Problem:** GitLab-Pipeline/Runner/Stage statt OneDev Build-Spec/Job/Trigger
- **Notiz:** `crates/onedev-build` modelliert falsche Konzepte
- **Quelle:** `docs/port-review/05-cicd-packages-workspaces.md`

---

## Offene Fragen (OQ-IDs)

| OQ-ID | Frage | Status | Trigger |
|-------|-------|--------|---------|
| OQ-001 | Wie werden die 47 REST-API-Endpunkte priorisiert? | Offen | Vor Slice 1 |
| OQ-002 | Welche Executor-Plugins werden zuerst implementiert? | Offen | Vor Slice 2 |
| OQ-003 | Wie wird die Wicket-HTMX Template-Wiederholung durchgeführt? | Offen | Vor Web-UI Slice |
| OQ-004 | Wie kompilierbar wird `onedev-db` wiederhergestellt? | Offen | Phase 1 |
| OQ-005 | Wie Search mit ANTLR → pest migriert wird? | Offen | Nach Git |

---

## Technische Schulden

### TD-001: Rollenmodell in Rust falsch

- **Datei:** `crates/onedev-core/src/models/role.rs`
- **Problem:** GitLab-Rollen als Bitflags statt OneDev-free-form Rollen
- **Auswirkung:** Blockiert alle berechtigungsbezogenen Features
- **Lösung:** Vanilla-Shiro Modell übernehmen

### TD-002: Build-Modell inkonsistent

- **Datei:** `crates/onedev-build/src/models.rs`
- **Problem:** Pipeline/Stage/Runner statt Build/Job/Trigger
- **Auswirkung:** Build-Spec kann nicht kompatibel sein
- **Lösung:** Modelle auf OneDev-Begriffe umbenennen

### TD-003: Web-UI fehlt vollständig

- **Pfad:** `crates/onedev-web/`
- **Problem:** Nur Stubs, keine Templates/Komponenten
- **Auswirkung:** Keine Benutzeroberfläche
- **Lösung:** 1-to-1 Wiederholung der 552 HTML-Templates

### TD-004: API-Endpunkte fehlen

- **Pfad:** `crates/onedev-api/`
- **Problem:** Keine REST-Endpunkte implementiert
- **Auswirkung:** Keine Backend-API
- **Lösung:** Alle 47 Endpoint-Gruppen implementieren

---

## Empfohlene nächste Schritte

1. **Sofort:** Rollenmodell-Entscheidung treffen und implementieren (OQ-001)
2. **Kurzfristig:** Build-Spec-Modelle umbenennen (TD-002)
3. **Mittelfristig:** Web-UI Portierungsstrategie definieren (OQ-003)
4. **Langfristig:** API-Endpunkte implementieren (OQ-001)

---

## Referenz-Dokumente

- `docs/port-review/README.md` - Gesamtdokumentation
- `docs/port-review/01-web-ui.md` - Web-UI Port-Anforderungen
- `docs/port-review/02-model-persistence.md` - Domain-Modelle
- `docs/port-review/03-security-roles-auth.md` - Sicherheitsmodell
- `docs/port-review/04-git-collaboration.md` - Git-Schicht
- `docs/port-review/05-cicd-packages-workspaces.md` - CI/CD
- `docs/port-review/06-platform-api-ops.md` - API/Plattform
- `docs/port-review/missing-functions-mapping.md` - Fehlende Funktionen

---

*Zuletzt aktualisiert: 2026-11-18*
*Status: Vollständiger 1-to-1 Port erforderlich*