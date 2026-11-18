# OneDev Rust-Port — Statusbericht (Stand 18. November 2026, Branch `mango`)

## Kurzfassung in einem Satz

Der Port kompiliert fehlerfrei, alle Tests sind grün, und die besten Teile
aus der parallelen `mango-lead`-Arbeit sind übernommen und live auf `mango`.

## Was läuft

- **Gesamter Workspace kompiliert** ohne Fehler (`cargo check --workspace`).
- **Alle Tests grün**, darunter 130 Tests in der Build-Crate.
- **Web-UI erreichbar**: alle 28 Seiten plus Vanilla-CSS liefern HTTP 200
  (Smoke-Test gegen das laufende Binary, inklusive Login mit Feedback
  bei falschem Passwort).
- **API-Handler verdrahtet**: Auth, User, Projekt, Gruppe, Issue,
  Merge-Request, Build, Git-Smart-HTTP und LFS arbeiten gegen echte
  Services statt Stubs.

## Was heute übernommen wurde (aus `mango-lead`)

- Vanilla-treue Build-Ausführung: Variablen wie `@var@`, Secrets-Masking,
  Retry-Regeln, Job-Templates, Checkout- und Command-Steps, Pfad-Filter.
- Nativer OCI-Registry-Client mit Image-Store (für Container-Builds).
- Datenbank-Migrationen 004 und 005 (Webhook-Versand, Suchindex,
  Build-Metadaten).
- Commit [3ff309bd](https://github.com/bkg-zig/onedev/commit/3ff309bdad),
  direkt in `mango` gemergt und gepusht — kein Konflikt, nichts verloren.

## Was bewusst noch draußen ist

- Lead's Server-Umbau und Webhook-Rewrite (weicht zu stark ab,
  braucht eigene Prüfung).
- Lead's CLI-Fehler (betreffen nur den Lead-Branch, nicht `mango`).
- Web-UI: Hülle und Login sind 1:1 nach Vanilla gebaut, die restlichen
  500+ Seiten und echte Backend-Daten folgen in Scheiben.

## Offene Punkte für dich

- `mango-lead`-Branch ansehen, wenn du die Server-Arbeit prüfen willst.
- Entscheiden, ob der Server-Umbau aus Lead auch übernommen werden soll.
- Datenbank für lokale Web-Tests bereitstellen (`ONEDEV_WEB_DATABASE_URL`),
  damit die Login-Seite gegen echte Benutzer prüft.
