//! Application state and everything needed to build it: the database pool,
//! the storage directories, the JWT key material and the services the whole
//! server shares.

use std::fs::OpenOptions;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context as _, bail};
use onedev_core::events::{EventBus, InMemoryEventBus};
use onedev_db::DbPool;
use rsa::pkcs1::{DecodeRsaPrivateKey, DecodeRsaPublicKey};
use rsa::pkcs8::{DecodePrivateKey, DecodePublicKey, EncodePublicKey, LineEnding};
use rsa::{RsaPrivateKey, RsaPublicKey};
use secrecy::SecretString;
use sqlx::ConnectOptions as _;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use tracing::{info, warn};

use crate::config::{AppConfig, DatabaseConfig};
use crate::lifecycle::Lifecycle;
use crate::services::build_scheduler::BuildScheduler;
use crate::services::build_scheduler::trigger::{self, BuildTrigger};
use crate::services::git_cli::GitCli;
use crate::services::webhook_dispatcher::WebhookDispatcher;

/// File names of the generated JWT key pair (below `config.key_dir()`).
const JWT_PRIVATE_KEY_FILE: &str = "jwt_private.pem";
const JWT_PUBLIC_KEY_FILE: &str = "jwt_public.pem";
/// Connection attempts at startup (the database may still be starting).
const CONNECT_ATTEMPTS: u32 = 6;

/// Everything the HTTP layer and the background services share.
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub pool: DbPool,
    pub lifecycle: Arc<Lifecycle>,
    /// Domain events (pushes, tags, pull requests) published by the git and
    /// API layers.
    pub events: Arc<dyn EventBus>,
    /// State of the REST API.
    pub api: onedev_api::AppState,
    pub scheduler: Arc<BuildScheduler>,
    pub build_trigger: Arc<BuildTrigger>,
    pub webhooks: Arc<WebhookDispatcher>,
    pub started_at: Instant,
}

impl AppState {
    /// Wire up all services on top of an open (and migrated) database.
    pub async fn build(
        config: Arc<AppConfig>,
        pool: DbPool,
        lifecycle: Arc<Lifecycle>,
    ) -> anyhow::Result<AppState> {
        let keys = {
            let config = config.clone();
            tokio::task::spawn_blocking(move || load_jwt_keys(&config))
                .await
                .context("the JWT key loader panicked")??
        };
        let auth = auth_config(&config, &keys);
        let api = onedev_api::create_app_state(pool.clone(), auth)
            .await
            .context("cannot initialise the API services")?;

        let events: Arc<dyn EventBus> = Arc::new(InMemoryEventBus::new());
        let scheduler = BuildScheduler::new(pool.clone(), config.clone())
            .context("cannot initialise the build scheduler")?;
        let build_trigger = Arc::new(BuildTrigger::new(
            pool.clone(),
            GitCli::new(config.git.git_binary.clone()),
            config.git.repos_root.clone(),
            scheduler.wake_handle(),
        ));
        if config.features.builds {
            trigger::subscribe(&*events, build_trigger.clone())
                .await
                .context("cannot subscribe the build trigger to the event bus")?;
        }
        let webhooks = Arc::new(
            WebhookDispatcher::new(pool.clone(), config.webhook.clone())
                .context("cannot initialise the webhook dispatcher")?,
        );

        Ok(AppState {
            config,
            pool,
            lifecycle,
            events,
            api,
            scheduler,
            build_trigger,
            webhooks,
            started_at: Instant::now(),
        })
    }
}

// ---------------------------------------------------------------------------
// database
// ---------------------------------------------------------------------------

/// Open the connection pool, retrying while the database is unreachable
/// (it may be starting next to the server). Credential or database-name
/// errors fail immediately.
pub async fn connect_database(config: &DatabaseConfig) -> anyhow::Result<DbPool> {
    let mut options = PgConnectOptions::from_str(&config.url)
        .context("database.url is not a valid PostgreSQL connection URL")?;
    if !config.sqlx_logging {
        options = options.disable_statement_logging();
    }
    let target = format!(
        "{}:{}/{}",
        options.get_host(),
        options.get_port(),
        options.get_database().unwrap_or("")
    );
    let pool_options = PgPoolOptions::new()
        .max_connections(config.max_connections.max(1))
        .min_connections(config.min_connections.min(config.max_connections.max(1)))
        .acquire_timeout(Duration::from_secs(config.connect_timeout_secs.max(1)))
        .idle_timeout((config.idle_timeout_secs > 0).then(|| Duration::from_secs(config.idle_timeout_secs)))
        .max_lifetime((config.max_lifetime_secs > 0).then(|| Duration::from_secs(config.max_lifetime_secs)))
        .test_before_acquire(true);

    let mut delay = Duration::from_secs(1);
    for attempt in 1..=CONNECT_ATTEMPTS {
        match pool_options.clone().connect_with(options.clone()).await {
            Ok(pool) => {
                info!(%target, max_connections = config.max_connections, "connected to the database");
                return Ok(pool);
            }
            Err(error) if attempt < CONNECT_ATTEMPTS && is_transient(&error) => {
                warn!(%error, %target, attempt, "database not reachable yet, retrying");
                tokio::time::sleep(delay).await;
                delay = (delay * 2).min(Duration::from_secs(16));
            }
            Err(error) => {
                return Err(anyhow::Error::new(error)
                    .context(format!("cannot connect to the database at {target}")));
            }
        }
    }
    unreachable!("the last attempt returns")
}

fn is_transient(error: &sqlx::Error) -> bool {
    matches!(error, sqlx::Error::Io(_) | sqlx::Error::PoolTimedOut)
}

/// Apply the embedded migrations (sqlx serialises concurrent runs with an
/// advisory lock, so several instances may start together).
pub async fn migrate(pool: &DbPool) -> anyhow::Result<()> {
    onedev_db::run_migrations(pool)
        .await
        .context("database migrations failed")?;
    info!("database schema is up to date");
    Ok(())
}

// ---------------------------------------------------------------------------
// directories
// ---------------------------------------------------------------------------

/// Create the storage directories and prove that they are writable, so a
/// misconfigured volume fails at startup instead of during the first build.
pub fn prepare_directories(config: &AppConfig) -> anyhow::Result<()> {
    let mut dirs: Vec<PathBuf> = vec![
        config.storage.local_path.clone(),
        config.build_work_dir(),
        config.build_artifact_dir(),
        config.temp_dir(),
        config.git.repos_root.clone(),
    ];
    if let Some(lfs) = &config.git.lfs_storage_path {
        dirs.push(lfs.clone());
    }
    for dir in &dirs {
        std::fs::create_dir_all(dir)
            .with_context(|| format!("cannot create directory {}", dir.display()))?;
        ensure_writable(dir)?;
    }
    let keys = config.key_dir();
    std::fs::create_dir_all(&keys)
        .with_context(|| format!("cannot create directory {}", keys.display()))?;
    restrict_directory(&keys)?;
    ensure_writable(&keys)?;
    Ok(())
}

fn ensure_writable(dir: &Path) -> anyhow::Result<()> {
    let probe = dir.join(format!(".onedev-write-test-{}", std::process::id()));
    let result = std::fs::write(&probe, b"ok");
    let _ = std::fs::remove_file(&probe);
    result.with_context(|| format!("directory {} is not writable", dir.display()))
}

#[cfg(unix)]
fn restrict_directory(dir: &Path) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
        .with_context(|| format!("cannot restrict permissions of {}", dir.display()))
}

#[cfg(not(unix))]
fn restrict_directory(_dir: &Path) -> anyhow::Result<()> {
    Ok(())
}

// ---------------------------------------------------------------------------
// JWT key material
// ---------------------------------------------------------------------------

/// RS256 key pair used to sign and verify access tokens.
pub struct JwtKeys {
    pub private_pem: SecretString,
    pub public_pem: String,
}

/// Load the configured key pair, or the generated one below the key
/// directory (created on first start).
///
/// A generated key lives next to the server's data and is therefore specific
/// to one installation: several instances sharing a database have to be given
/// the same `auth.jwt_private_key_file` / `auth.jwt_public_key_file`.
pub fn load_jwt_keys(config: &AppConfig) -> anyhow::Result<JwtKeys> {
    if let (Some(private_file), Some(public_file)) = (
        &config.auth.jwt_private_key_file,
        &config.auth.jwt_public_key_file,
    ) {
        warn_if_world_readable(private_file);
        let private = read_pem(private_file, "auth.jwt_private_key_file")?;
        let public = read_pem(public_file, "auth.jwt_public_key_file")?;
        let derived = parse_private(&private)
            .context("auth.jwt_private_key_file is not a PEM encoded RSA private key")?
            .to_public_key();
        let configured = parse_public(&public)
            .context("auth.jwt_public_key_file is not a PEM encoded RSA public key")?;
        if derived != configured {
            bail!("auth.jwt_public_key_file does not belong to auth.jwt_private_key_file");
        }
        return Ok(JwtKeys {
            private_pem: SecretString::from(private),
            public_pem: public,
        });
    }

    let dir = config.key_dir();
    let private_path = dir.join(JWT_PRIVATE_KEY_FILE);
    let public_path = dir.join(JWT_PUBLIC_KEY_FILE);

    if !private_path.exists() {
        let (private, _) = onedev_auth::generate_rsa_keys()
            .map_err(|error| anyhow::anyhow!("cannot generate the JWT signing key: {error}"))?;
        // Another instance starting at the same moment may win; its key is
        // the one everybody then uses.
        if create_exclusive(&private_path, private.as_bytes(), 0o600)
            .with_context(|| format!("cannot store {}", private_path.display()))?
        {
            info!(path = %private_path.display(), "generated a new JWT signing key");
        }
    }
    warn_if_world_readable(&private_path);
    let private = read_pem(&private_path, "the generated JWT signing key")?;
    let public = derive_public_pem(&private)
        .context("the stored JWT signing key is not a PEM encoded RSA private key")?;
    // The public key is derived from the private one, so writing it is
    // idempotent; it is stored for tools that verify tokens.
    if std::fs::read_to_string(&public_path).ok().as_deref() != Some(public.as_str()) {
        if let Err(error) = write_replace(&public_path, public.as_bytes(), 0o644) {
            warn!(%error, path = %public_path.display(), "cannot store the JWT public key");
        }
    }
    Ok(JwtKeys {
        private_pem: SecretString::from(private),
        public_pem: public,
    })
}

fn read_pem(path: &Path, what: &str) -> anyhow::Result<String> {
    std::fs::read_to_string(path).with_context(|| format!("cannot read {what} ({})", path.display()))
}

fn parse_private(pem: &str) -> anyhow::Result<RsaPrivateKey> {
    RsaPrivateKey::from_pkcs8_pem(pem)
        .or_else(|_| RsaPrivateKey::from_pkcs1_pem(pem))
        .map_err(|error| anyhow::anyhow!("{error}"))
}

fn parse_public(pem: &str) -> anyhow::Result<RsaPublicKey> {
    RsaPublicKey::from_public_key_pem(pem)
        .or_else(|_| RsaPublicKey::from_pkcs1_pem(pem))
        .map_err(|error| anyhow::anyhow!("{error}"))
}

/// SPKI PEM of the public half of a private key.
pub fn derive_public_pem(private_pem: &str) -> anyhow::Result<String> {
    parse_private(private_pem)?
        .to_public_key()
        .to_public_key_pem(LineEnding::LF)
        .map_err(|error| anyhow::anyhow!("cannot encode the public key: {error}"))
}

#[cfg(unix)]
fn warn_if_world_readable(path: &Path) {
    use std::os::unix::fs::PermissionsExt as _;
    if let Ok(metadata) = std::fs::metadata(path) {
        if metadata.permissions().mode() & 0o077 != 0 {
            warn!(
                path = %path.display(),
                "the JWT private key is readable by other users, restrict it to the server user (chmod 600)"
            );
        }
    }
}

#[cfg(not(unix))]
fn warn_if_world_readable(_path: &Path) {}

/// Create `path` with `contents` unless it already exists. Returns whether
/// this call created it. The content becomes visible atomically (a hard link
/// of a fully written temporary file), so a concurrent reader never sees a
/// partial key.
fn create_exclusive(path: &Path, contents: &[u8], mode: u32) -> std::io::Result<bool> {
    let tmp = temp_sibling(path)?;
    write_new(&tmp, contents, mode)?;
    let linked = std::fs::hard_link(&tmp, path);
    let _ = std::fs::remove_file(&tmp);
    match linked {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
        // The file system has no hard links: still exclusive, not atomic.
        Err(_) => match write_new(path, contents, mode) {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
            Err(error) => Err(error),
        },
    }
}

/// Replace `path` atomically.
fn write_replace(path: &Path, contents: &[u8], mode: u32) -> std::io::Result<()> {
    let tmp = temp_sibling(path)?;
    write_new(&tmp, contents, mode)?;
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

fn temp_sibling(path: &Path) -> std::io::Result<PathBuf> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "path has no file name"))?;
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    Ok(dir.join(format!(".{name}.{}.tmp", uuid::Uuid::new_v4().simple())))
}

/// Write a new file (it must not exist) with the given permission bits.
fn write_new(path: &Path, contents: &[u8], mode: u32) -> std::io::Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(mode);
    }
    #[cfg(not(unix))]
    let _ = mode;
    let mut file = options.open(path)?;
    file.write_all(contents)?;
    file.sync_all()
}

/// Map the server configuration onto the settings of the auth service.
pub fn auth_config(config: &AppConfig, keys: &JwtKeys) -> onedev_auth::AuthConfig {
    let auth = &config.auth;
    onedev_auth::AuthConfig {
        jwt_config: onedev_auth::TokenConfig {
            issuer: auth.jwt_issuer.clone(),
            audience: auth.jwt_audience.clone(),
            access_ttl_seconds: auth.access_token_ttl_minutes.saturating_mul(60),
            refresh_ttl_seconds: auth.refresh_token_ttl_days.saturating_mul(24 * 60 * 60),
            private_key: keys.private_pem.clone(),
            public_key: SecretString::from(keys.public_pem.clone()),
            ..onedev_auth::TokenConfig::default()
        },
        totp_config: onedev_auth::TotpConfig::default(),
        session_ttl_seconds: auth.session_timeout_minutes.saturating_mul(60),
        remember_me_ttl_seconds: auth.remember_me_days.saturating_mul(24 * 60 * 60),
        max_failed_attempts: auth.max_failed_attempts,
        lockout_duration_seconds: auth.lockout_duration_minutes.saturating_mul(60),
        password_reset_ttl_seconds: auth.password_reset_ttl_minutes.saturating_mul(60),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use secrecy::ExposeSecret as _;

    fn config_in(dir: &Path) -> AppConfig {
        let mut config = AppConfig::default();
        config.storage.local_path = dir.join("storage");
        config.git.repos_root = dir.join("repos");
        config
    }

    #[test]
    fn directories_are_created_and_the_key_directory_is_private() {
        let dir = tempfile::tempdir().unwrap();
        let config = config_in(dir.path());
        prepare_directories(&config).unwrap();
        for path in [
            config.storage.local_path.clone(),
            config.build_work_dir(),
            config.build_artifact_dir(),
            config.temp_dir(),
            config.git.repos_root.clone(),
        ] {
            assert!(path.is_dir(), "{} must exist", path.display());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(config.key_dir()).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o700);
        }
        // idempotent
        prepare_directories(&config).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn an_unwritable_directory_is_reported_with_its_path() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = tempfile::tempdir().unwrap();
        let mut config = config_in(dir.path());
        let locked = dir.path().join("locked");
        std::fs::create_dir(&locked).unwrap();
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o500)).unwrap();
        if std::fs::write(locked.join("probe"), b"x").is_ok() {
            // running as root: permissions do not restrict us
            return;
        }
        config.git.repos_root = locked.clone();
        let error = prepare_directories(&config).unwrap_err().to_string();
        assert!(error.contains("locked"), "{error}");
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o700)).unwrap();
    }

    #[test]
    fn a_key_pair_is_generated_once_and_reused() {
        let dir = tempfile::tempdir().unwrap();
        let config = config_in(dir.path());
        prepare_directories(&config).unwrap();

        let first = load_jwt_keys(&config).unwrap();
        let second = load_jwt_keys(&config).unwrap();
        assert_eq!(first.private_pem.expose_secret(), second.private_pem.expose_secret());
        assert_eq!(first.public_pem, second.public_pem);
        assert!(first.private_pem.expose_secret().contains("PRIVATE KEY"));
        assert!(first.public_pem.contains("PUBLIC KEY"));
        assert_eq!(
            derive_public_pem(first.private_pem.expose_secret()).unwrap(),
            first.public_pem
        );

        let private_path = config.key_dir().join(JWT_PRIVATE_KEY_FILE);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(&private_path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600, "the signing key must be private");
        }
        // a missing public key file is restored from the private key
        std::fs::remove_file(config.key_dir().join(JWT_PUBLIC_KEY_FILE)).unwrap();
        let third = load_jwt_keys(&config).unwrap();
        assert_eq!(third.public_pem, first.public_pem);
        assert!(config.key_dir().join(JWT_PUBLIC_KEY_FILE).is_file());
        // no temporary files are left behind
        let leftovers: Vec<_> = std::fs::read_dir(config.key_dir())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
    }

    #[test]
    fn configured_keys_must_belong_together() {
        let dir = tempfile::tempdir().unwrap();
        let (private_a, public_a) = onedev_auth::generate_rsa_keys().unwrap();
        let (_, public_b) = onedev_auth::generate_rsa_keys().unwrap();
        let private_path = dir.path().join("private.pem");
        let public_path = dir.path().join("public.pem");
        std::fs::write(&private_path, &private_a).unwrap();

        let mut config = config_in(dir.path());
        config.auth.jwt_private_key_file = Some(private_path.clone());
        config.auth.jwt_public_key_file = Some(public_path.clone());

        std::fs::write(&public_path, &public_a).unwrap();
        let keys = load_jwt_keys(&config).unwrap();
        assert_eq!(keys.public_pem, public_a);

        std::fs::write(&public_path, &public_b).unwrap();
        let error = load_jwt_keys(&config).err().expect("mismatch must fail").to_string();
        assert!(error.contains("does not belong"), "{error}");

        std::fs::write(&public_path, "not a key").unwrap();
        let error = format!("{:#}", load_jwt_keys(&config).err().expect("garbage must fail"));
        assert!(error.contains("public key"), "{error}");

        std::fs::remove_file(&public_path).unwrap();
        let error = format!("{:#}", load_jwt_keys(&config).err().expect("missing file must fail"));
        assert!(error.contains("auth.jwt_public_key_file"), "{error}");
    }

    #[test]
    fn a_configured_private_key_in_pkcs1_form_is_accepted() {
        use rsa::pkcs1::EncodeRsaPrivateKey;
        let dir = tempfile::tempdir().unwrap();
        let (private_pkcs8, public) = onedev_auth::generate_rsa_keys().unwrap();
        let key = RsaPrivateKey::from_pkcs8_pem(&private_pkcs8).unwrap();
        let pkcs1 = key.to_pkcs1_pem(LineEnding::LF).unwrap();
        assert!(pkcs1.contains("BEGIN RSA PRIVATE KEY"));
        let private_path = dir.path().join("private.pem");
        let public_path = dir.path().join("public.pem");
        std::fs::write(&private_path, pkcs1.as_bytes()).unwrap();
        std::fs::write(&public_path, &public).unwrap();
        let mut config = config_in(dir.path());
        config.auth.jwt_private_key_file = Some(private_path);
        config.auth.jwt_public_key_file = Some(public_path);
        assert!(load_jwt_keys(&config).is_ok());
    }

    #[test]
    fn exclusive_creation_never_replaces_an_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("secret");
        assert!(create_exclusive(&path, b"first", 0o600).unwrap());
        assert!(!create_exclusive(&path, b"second", 0o600).unwrap());
        assert_eq!(std::fs::read(&path).unwrap(), b"first");
        write_replace(&path, b"third", 0o600).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"third");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1, "no temporary files");
    }

    #[test]
    fn the_auth_configuration_follows_the_server_configuration() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = config_in(dir.path());
        config.auth.access_token_ttl_minutes = 5;
        config.auth.refresh_token_ttl_days = 2;
        config.auth.session_timeout_minutes = 30;
        config.auth.remember_me_days = 10;
        config.auth.max_failed_attempts = 3;
        config.auth.lockout_duration_minutes = 7;
        config.auth.jwt_issuer = "issuer-x".to_owned();
        config.auth.jwt_audience = "audience-y".to_owned();
        let keys = JwtKeys {
            private_pem: SecretString::from("private".to_owned()),
            public_pem: "public".to_owned(),
        };
        let auth = auth_config(&config, &keys);
        assert_eq!(auth.jwt_config.access_ttl_seconds, 300);
        assert_eq!(auth.jwt_config.refresh_ttl_seconds, 2 * 86_400);
        assert_eq!(auth.jwt_config.issuer, "issuer-x");
        assert_eq!(auth.jwt_config.audience, "audience-y");
        assert_eq!(auth.session_ttl_seconds, 1800);
        assert_eq!(auth.remember_me_ttl_seconds, 10 * 86_400);
        assert_eq!(auth.max_failed_attempts, 3);
        assert_eq!(auth.lockout_duration_seconds, 420);
    }

    #[test]
    fn only_connection_level_errors_are_retried() {
        assert!(is_transient(&sqlx::Error::PoolTimedOut));
        assert!(is_transient(&sqlx::Error::Io(std::io::Error::other("refused"))));
        assert!(!is_transient(&sqlx::Error::RowNotFound));
        assert!(!is_transient(&sqlx::Error::Configuration("bad url".into())));
    }

    #[tokio::test]
    async fn an_invalid_database_url_is_rejected_without_retrying() {
        let config = DatabaseConfig {
            url: "this is not a url".to_owned(),
            ..DatabaseConfig::default()
        };
        let started = Instant::now();
        let error = connect_database(&config).await.err().expect("must fail").to_string();
        assert!(error.contains("database.url"), "{error}");
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[tokio::test]
    async fn the_pool_connects_and_migrations_are_idempotent() {
        let Some(db) = crate::testsupport::TestDb::new().await else { return };
        let config = DatabaseConfig {
            url: db.url.clone(),
            max_connections: 3,
            min_connections: 1,
            ..DatabaseConfig::default()
        };
        let pool = connect_database(&config).await.unwrap();
        migrate(&pool).await.unwrap();
        migrate(&pool).await.unwrap();
        let one: i32 = sqlx::query_scalar("SELECT 1").fetch_one(&pool).await.unwrap();
        assert_eq!(one, 1);
        pool.close().await;
    }
}
