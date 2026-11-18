//! Test fixtures: an isolated, fully migrated Postgres database per test and
//! helpers to seed rows and create git repositories.
//!
//! Database tests need a reachable server: set `ONEDEV_TEST_DATABASE_URL`
//! (any database on that server, the role must be allowed to `CREATE
//! DATABASE`). Without it the tests skip, unless `ONEDEV_REQUIRE_DB_TESTS`
//! is set, in which case a missing database is a hard failure.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;

use onedev_db::DbPool;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};

/// One throw-away database; dropped (forcefully) together with the value.
pub struct TestDb {
    pub pool: DbPool,
    pub url: String,
    name: String,
    admin_options: PgConnectOptions,
}

impl TestDb {
    /// `None` (test should return early) when no test database is configured.
    pub async fn new() -> Option<TestDb> {
        let admin_url = match std::env::var("ONEDEV_TEST_DATABASE_URL") {
            Ok(url) if !url.is_empty() => url,
            _ => {
                assert!(
                    std::env::var("ONEDEV_REQUIRE_DB_TESTS").is_err(),
                    "ONEDEV_REQUIRE_DB_TESTS is set but ONEDEV_TEST_DATABASE_URL is missing"
                );
                eprintln!("skipping database test: ONEDEV_TEST_DATABASE_URL is not set");
                return None;
            }
        };
        let admin_options = PgConnectOptions::from_str(&admin_url).expect("valid ONEDEV_TEST_DATABASE_URL");
        let name = format!("onedev_test_{}", uuid::Uuid::new_v4().simple());

        let admin = PgPoolOptions::new()
            .max_connections(1)
            .connect_with(admin_options.clone())
            .await
            .expect("connect to the test database server");
        sqlx::query(&format!("CREATE DATABASE {name}"))
            .execute(&admin)
            .await
            .expect("create the per-test database");
        admin.close().await;

        let options = admin_options.clone().database(&name);
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect_with(options)
            .await
            .expect("connect to the per-test database");
        onedev_db::run_migrations(&pool).await.expect("migrations apply");

        let url = admin_url
            .rsplit_once('/')
            .map(|(base, _)| format!("{base}/{name}"))
            .unwrap_or(admin_url);
        Some(TestDb {
            pool,
            url,
            name,
            admin_options,
        })
    }
}

impl Drop for TestDb {
    fn drop(&mut self) {
        let name = self.name.clone();
        let options = self.admin_options.clone();
        // Own thread + runtime: works during unwinding and inside a test runtime.
        let _ = std::thread::spawn(move || {
            let Ok(runtime) = tokio::runtime::Builder::new_current_thread().enable_all().build() else {
                return;
            };
            runtime.block_on(async move {
                if let Ok(admin) = PgPoolOptions::new().max_connections(1).connect_with(options).await {
                    let _ = sqlx::query(&format!("DROP DATABASE IF EXISTS {name} WITH (FORCE)"))
                        .execute(&admin)
                        .await;
                }
            });
        })
        .join();
    }
}

pub async fn seed_user(pool: &DbPool, name: &str) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "INSERT INTO users (name, email, status) VALUES ($1, $2, 'active') RETURNING id",
    )
    .bind(name)
    .bind(format!("{name}@example.com"))
    .fetch_one(pool)
    .await
    .expect("seed user")
}

pub async fn seed_project(pool: &DbPool, owner_id: i64, path: &str, parent_id: Option<i64>) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "INSERT INTO projects (name, path, owner_id, parent_id) VALUES ($1, $1, $2, $3) RETURNING id",
    )
    .bind(path)
    .bind(owner_id)
    .bind(parent_id)
    .fetch_one(pool)
    .await
    .expect("seed project")
}

fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(dir)
        .args(["-c", "user.name=Test", "-c", "user.email=test@example.com", "-c", "commit.gpgsign=false"])
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output()
        .expect("git is installed");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// Create a bare repository at `<repos_root>/<full_path>.git` containing one
/// commit with the given files (on branch `main`). Returns (repo dir, commit).
pub fn make_repo(repos_root: &Path, full_path: &str, files: &[(&str, &str)]) -> (PathBuf, String) {
    let bare = repos_root.join(format!("{full_path}.git"));
    let scratch = tempfile::tempdir().expect("scratch dir");
    let work = scratch.path();
    git(work, &["init", "-q", "-b", "main"]);
    for (path, content) in files {
        let target = work.join(path);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(target, content).unwrap();
    }
    git(work, &["add", "-A"]);
    git(work, &["commit", "-q", "-m", "initial commit"]);
    let commit = git(work, &["rev-parse", "HEAD"]);
    std::fs::create_dir_all(bare.parent().unwrap()).unwrap();
    git(
        work,
        &["clone", "-q", "--bare", work.to_str().unwrap(), bare.to_str().unwrap()],
    );
    (bare, commit)
}

/// Add one more commit on `main` of a bare repository created by [`make_repo`].
pub fn push_commit(bare: &Path, files: &[(&str, Option<&str>)], message: &str) -> String {
    let scratch = tempfile::tempdir().expect("scratch dir");
    let work = scratch.path().join("w");
    git(
        scratch.path(),
        &["clone", "-q", bare.to_str().unwrap(), work.to_str().unwrap()],
    );
    for (path, content) in files {
        let target = work.join(path);
        match content {
            Some(content) => {
                std::fs::create_dir_all(target.parent().unwrap()).unwrap();
                std::fs::write(target, content).unwrap();
            }
            None => {
                let _ = std::fs::remove_file(target);
            }
        }
    }
    git(&work, &["add", "-A"]);
    git(&work, &["commit", "-q", "-m", message]);
    git(&work, &["push", "-q", "origin", "HEAD:main"]);
    git(&work, &["rev-parse", "HEAD"])
}
