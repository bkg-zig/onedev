# Compose & Web-UI testen

## Voraussetzungen

- Docker-Daemon läuft.
- Secrets liegen unter `/srv/bkg/admin-state/db.env` (DB-Passwort);
  `docker/secrets/` enthält `private_key.pem`, `public_key.pem`,
  `webhook_secret.txt`, `db_url.txt` (Beispiele anpassen).

## Starten

```bash
cd /srv/bkg/services/bkg-core-admin/infra/onedev
docker compose -f docker/compose.yml up -d --build db core web
```

## Was wo läuft

| Service   | Außen-Port | Innen-Port | Inhalt                        |
|-----------|------------|------------|-------------------------------|
| db        | 5432       | 5432       | Postgres (pgrust)             |
| core      | 46315      | 18080      | API + Server                  |
| web       | 46316      | 18081      | Web-UI (SSR, Tera + Vanilla-CSS) |
| gateway   | 36697      | 8080       | ruxen (nur Profil `gateway`)  |

Ports stehen in `docker/ports.env`
(`BKG_CORE_PORT`, `BKG_WEB_PORT`, `BKG_GATEWAY_PORT`, `BKG_DB_PORT`).

## Web-UI prüfen

```bash
curl -s -o /dev/null -w "root:%{http_code}\n" http://127.0.0.1:46316/
curl -s -o /dev/null -w "health:%{http_code}\n" http://127.0.0.1:46316/health
curl -s -o /dev/null -w "login:%{http_code}\n" http://127.0.0.1:46316/login
```

Erwartet: überall HTTP 200. Die Login-Seite nutzt Vanilla-Layout und
Vanilla-CSS (`/static/vanilla/...`); ohne Datenbank greift sie auf
Platzhalterdaten zurück, mit `ONEDEV_WEB_DATABASE_URL` auf echten
`AuthService` + Migrationen.

## Hinweis zu Build-Kontexten

Alle `context`-Pfade in `docker/compose.yml` sind relativ zum
`docker/`-Verzeichnis (Projektverzeichnis bei `-f docker/compose.yml`):
`..` = Repo-Root, `../ruxen` und `../pgrust/...` zeigen auf die
Nachbar-Repos unter `infra/`. Von einem anderen Arbeitsverzeichnis aus
ändert sich daran nichts.
