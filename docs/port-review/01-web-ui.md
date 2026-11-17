# 01 — Web-UI (1-to-1 PORT ERFORDERLICH!)

Quelle: `server-core/.../web/**` (1502 Java-Klassen plus die daneben liegenden HTML/JS/CSS-Dateien), gegen `crates/onedev-web` und `crates/onedev-server`.

## 1. Kurzfazit ⚠️ KEIN MVP - VOLLE 1-to-1 PORTIERUNG

**WICHTIG:** Dies ist ein **vollständiger 1-to-1 Port**, kein MVP!

Die Web-Oberfläche ist der größte einzelne Block von OneDev. **Muss 1-to-1 portiert werden** - kein Neubau mit Tera/Tailwind/dx!

`onedev-web` und `onedev-server` sind aktuell **leere Stubs** (19 Zeilen).

Vanilla bringt **1502 Java-Klassen, 552 HTML Templates, 400 JS und 267 CSS-Dateien** mit — das UI ist serverseitig (Apache Wicket) mit zustandsbehafteten Komponenten und partiellen AJAX-Updates.

**FÜR 1-to-1 PORT:**
- Alle 552 HTML-Templates müssen **wiederholt** werden (nicht neu gebaut!)
- 400 JS-Dateien integrieren
- 267 CSS-Dateien wiederverwenden
- HTMX/WebSocket für Interaktivität

---

## RV-FINDINGS

| RV-ID | Klasse | Schwere | Ort | Problem | Erforderliche Korrektur |
|-------|--------|---------|-----|---------|------------------------|
| RV-PORT-001 | fix-in-slice | Important | docs/rust-rules.md§9 | Tera/Tailwind/dx Stack wird als Alternative zur 1-to-1 Portierung vorgeschlagen | Web-UI muss Wicket-HTMX Ansatz mit Wiederholung der 552 HTML-Templates verwenden |
| RV-PORT-002 | fix-in-slice | Important | docs/port-review/01-web-ui.md§1 | Keine klare Spezifikation für Wicket-HTMX Portierungsstrategie | Detaillierte Anleitung für Template-Wiederholung mit HTMX-Attribute erstellen |
| RV-PORT-003 | backlog | Suggestion | docs/port-review/01-web-ui.md | PDF.js WASM-Integration nicht behandelt | WASM-Binary-Handling in Rust erläutern (optional) |

---

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

---

## 3. Rust-Stand

`crates/onedev-web/src` und `crates/onedev-server/src` enthalten je eine Datei mit 19 Zeilen.

---

## 7. Abweichungen und Risiken

- **Tera/Tailwind/dx vs Wicket/HTMX:** Der vorgeschlagene Stack reproduziert das OneDev-UI nicht. Für 1-to-1-Port muss Wicket-HTMX verwendet werden.
- **PDF.js WASM:** Inkludiert WASM und Fonts für Dateivorschau - muss in Rust eingebunden werden.