# Compose & Web-UI testen

## Voraussetzungen

- Docker-Daemon läuft.
- DB-Passwort unter `/srv/bkg/admin-state/db.env`.
- Secrets in `docker/secrets/` (gitignored, pro Maschine erzeugen):
  ```bash
  openssl genrsa -out docker/secrets/private_key.pem 2048
  openssl rsa -in docker/secrets/private_key.pem -pubout -out docker/secrets/public_key.pem
  chmod 600 docker/secrets/private_key.pem
  openssl rand -hex 32 > docker/secrets/webhook_secret.txt
  printf 'postgres://postgres:<PW>@db:5432/onedev' > docker/secrets/db_url.txt
  ```

## Starten

```bash
cd /srv/bkg/services/bkg-core-admin/infra/onedev
scripts/docker-build.sh up          # baut db/core/web und startet sie
```

Direkt mit Compose geht es auch (Projektverzeichnis pinnen!):

```bash
docker compose --project-directory . -f docker/compose.yml up -d --build db core web
```

Alle Befehle laufen über ein Skript (`scripts/docker-build.sh` —
`build|up|up-all|down|stop|start|restart|update|logs|ps|config`):

> Wichtig: immer mit `--project-directory .` aus dem Repo-Root starten.
> Ohne den Pin hängt das Projektverzeichnis von der Compose-Version ab
> (Aufrufverzeichnis vs. Verzeichnis der Compose-Datei) und die
> Build-Kontexte zeigen ins Leere (`docker/docker not found`,
> `onedev/pgrust not found`).

## Was wo läuft

| Service   | Außen-Port | Innen-Port | Inhalt                        |
|-----------|------------|------------|-------------------------------|
| db        | 5432       | 5432       | Postgres (pgrust)             |
| core      | 46315      | 18080      | API + Server                  |
| web       | 46316      | 18081      | Web-UI (SSR, Tera + Vanilla-CSS) |
| gateway   | 36697      | 8080       | ruxen (nur Profil `gateway`)  |

Ports stehen in `docker/ports.env`
(`BKG_CORE_PORT`, `BKG_WEB_PORT`, `BKG_GATEWAY_PORT`, `BKG_DB_PORT`).

## Temp-DB für Tests (berührt die echte DB nicht)

```bash
scripts/temp-db.sh start   # pgrust auf Port 55432, URL wird ausgegeben
scripts/temp-db.sh psql    # psql-Shell
scripts/temp-db.sh stop    # Container + Daten verwerfen
```

Web-UI gegen die Temp-DB testen:

```bash
ONEDEV_WEB_DATABASE_URL=$(scripts/temp-db.sh url) ./target/debug/onedev-web
```

Skripte: `scripts/docker-build.sh` (bauen/starten, Standard: `db core web`),
`scripts/temp-db.sh` (`start|stop|url|psql`).

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
