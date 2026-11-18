//! Persist the log lines a running build produces.
//!
//! The runner works on a blocking thread and pushes [`LogLine`]s into a
//! bounded channel (so a slow database slows the build down instead of
//! filling the memory); this task drains the channel and writes the lines to
//! `build_logs` in batches. Secrets are already masked by the runner's
//! `LogSink`, nothing sensitive reaches this module.

use std::time::Duration;

use onedev_build::models::LogLine;
use onedev_db::DbPool;
use tokio::task::JoinHandle;
use tracing::warn;

/// Lines written with one statement.
const MAX_BATCH: usize = 500;
/// Longest wait for more lines before a partial batch is written.
const BATCH_WINDOW: Duration = Duration::from_millis(200);
/// Attempts per batch before its lines are dropped.
const WRITE_ATTEMPTS: u32 = 4;

/// What the writer did, reported when it finishes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LogStats {
    pub stored: u64,
    /// Lines beyond `max_lines`.
    pub discarded: u64,
    /// Lines lost to database errors.
    pub lost: u64,
}

/// Drain `lines` into `build_logs` until every sender is gone.
///
/// At most `max_lines` lines are stored per build; the first line over the
/// limit is replaced by a notice and the rest is discarded (the channel keeps
/// being drained so the build never blocks on a full log).
pub fn spawn_writer(
    pool: DbPool,
    build_id: i64,
    lines: flume::Receiver<LogLine>,
    max_lines: u64,
) -> JoinHandle<LogStats> {
    tokio::spawn(async move {
        let mut stats = LogStats::default();
        // Continue after existing lines (the build may have been logging before).
        let mut next_seq = match sqlx::query_scalar::<_, Option<i64>>(
            "SELECT MAX(seq) FROM build_logs WHERE build_id = $1",
        )
        .bind(build_id)
        .fetch_one(&pool)
        .await
        {
            Ok(max) => max.map_or(0, |max| max + 1),
            Err(error) => {
                warn!(%error, build_id, "cannot read the log position, starting at 0");
                0
            }
        };
        let mut notice_written = false;
        let mut closed = false;

        while !closed {
            let Ok(first) = lines.recv_async().await else {
                break;
            };
            let mut batch = vec![first];
            let window = tokio::time::sleep(BATCH_WINDOW);
            tokio::pin!(window);
            while batch.len() < MAX_BATCH {
                tokio::select! {
                    received = lines.recv_async() => match received {
                        Ok(line) => batch.push(line),
                        Err(_) => {
                            closed = true;
                            break;
                        }
                    },
                    _ = &mut window => break,
                }
            }

            // apply the line limit
            let room = max_lines.saturating_sub(stats.stored);
            if (batch.len() as u64) > room {
                let excess = batch.len() as u64 - room;
                batch.truncate(room as usize);
                stats.discarded += excess;
            }
            if stats.discarded > 0 && !notice_written {
                notice_written = true;
                batch.push(LogLine {
                    job: String::new(),
                    step: String::new(),
                    stderr: true,
                    line: format!(
                        "[log output truncated: a build stores at most {max_lines} lines]"
                    ),
                });
            }
            if batch.is_empty() {
                continue;
            }

            let count = batch.len() as u64;
            match write_batch(&pool, build_id, next_seq, &batch).await {
                Ok(()) => stats.stored += count,
                Err(error) => {
                    stats.lost += count;
                    metrics::counter!("build_log_lines_lost_total").increment(count);
                    warn!(%error, build_id, lines = count, "dropping log lines after repeated database errors");
                }
            }
            next_seq += count as i64;
        }
        stats
    })
}

async fn write_batch(
    pool: &DbPool,
    build_id: i64,
    first_seq: i64,
    batch: &[LogLine],
) -> Result<(), sqlx::Error> {
    // NUL bytes are not allowed in Postgres TEXT.
    let clean = |text: &str| text.replace('\0', "\u{fffd}");
    let seqs: Vec<i64> = (0..batch.len() as i64).map(|i| first_seq + i).collect();
    let jobs: Vec<String> = batch.iter().map(|l| clean(&l.job)).collect();
    let steps: Vec<String> = batch.iter().map(|l| clean(&l.step)).collect();
    let streams: Vec<&str> = batch
        .iter()
        .map(|l| if l.stderr { "stderr" } else { "stdout" })
        .collect();
    let texts: Vec<String> = batch.iter().map(|l| clean(&l.line)).collect();

    let mut delay = Duration::from_millis(250);
    let mut attempt = 0;
    loop {
        attempt += 1;
        let result = sqlx::query(
            r#"
            INSERT INTO build_logs (build_id, seq, job_name, step_name, stream, line)
            SELECT $1, t.seq, t.job_name, t.step_name, t.stream, t.line
            FROM UNNEST($2::bigint[], $3::text[], $4::text[], $5::text[], $6::text[])
                 AS t(seq, job_name, step_name, stream, line)
            ON CONFLICT (build_id, seq) DO NOTHING
            "#,
        )
        .bind(build_id)
        .bind(&seqs)
        .bind(&jobs)
        .bind(&steps)
        .bind(&streams)
        .bind(&texts)
        .execute(pool)
        .await;
        match result {
            Ok(_) => return Ok(()),
            Err(error) if attempt < WRITE_ATTEMPTS => {
                warn!(%error, attempt, "writing build log lines failed, retrying");
                tokio::time::sleep(delay).await;
                delay *= 2;
            }
            Err(error) => return Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testsupport::{TestDb, seed_project, seed_user};

    async fn seed_build(pool: &DbPool) -> i64 {
        let user = seed_user(pool, "log-user").await;
        let project = seed_project(pool, user, "log-project", None).await;
        sqlx::query_scalar::<_, i64>(
            "INSERT INTO builds (project_id, number, ref_name, commit_sha, author_id, spec) \
             VALUES ($1, 1, 'main', 'abc', $2, '{}'::jsonb) RETURNING id",
        )
        .bind(project)
        .bind(user)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    fn line(job: &str, text: &str, stderr: bool) -> LogLine {
        LogLine {
            job: job.to_owned(),
            step: "step".to_owned(),
            stderr,
            line: text.to_owned(),
        }
    }

    #[tokio::test]
    async fn lines_are_stored_in_order_with_stream_and_sanitized() {
        let Some(db) = TestDb::new().await else { return };
        let build = seed_build(&db.pool).await;
        let (tx, rx) = flume::bounded(16);
        let writer = spawn_writer(db.pool.clone(), build, rx, 1000);
        tx.send(line("build", "first", false)).unwrap();
        tx.send(line("build", "second\0with nul", true)).unwrap();
        // a pause longer than the batch window: two separate batches
        tokio::time::sleep(BATCH_WINDOW * 2).await;
        tx.send(line("test", "third", false)).unwrap();
        drop(tx);
        let stats = writer.await.unwrap();
        assert_eq!(stats, LogStats { stored: 3, discarded: 0, lost: 0 });

        let rows: Vec<(i64, String, String, String)> = sqlx::query_as(
            "SELECT seq, job_name, stream, line FROM build_logs WHERE build_id = $1 ORDER BY seq",
        )
        .bind(build)
        .fetch_all(&db.pool)
        .await
        .unwrap();
        assert_eq!(
            rows,
            vec![
                (0, "build".into(), "stdout".into(), "first".into()),
                (1, "build".into(), "stderr".into(), "second\u{fffd}with nul".into()),
                (2, "test".into(), "stdout".into(), "third".into()),
            ]
        );
    }

    #[tokio::test]
    async fn the_line_limit_stores_a_notice_and_keeps_draining() {
        let Some(db) = TestDb::new().await else { return };
        let build = seed_build(&db.pool).await;
        let (tx, rx) = flume::bounded(4);
        let writer = spawn_writer(db.pool.clone(), build, rx, 5);
        // the bounded channel only makes progress when the writer drains it
        let producer = tokio::task::spawn_blocking(move || {
            for i in 0..50 {
                tx.send(line("build", &format!("line {i}"), false)).unwrap();
            }
        });
        producer.await.unwrap();
        let stats = writer.await.unwrap();
        assert_eq!(stats.discarded, 45);
        assert_eq!(stats.lost, 0);

        let texts: Vec<String> =
            sqlx::query_scalar("SELECT line FROM build_logs WHERE build_id = $1 ORDER BY seq")
                .bind(build)
                .fetch_all(&db.pool)
                .await
                .unwrap();
        assert_eq!(texts.len(), 6, "5 lines and the truncation notice: {texts:?}");
        assert_eq!(texts[0], "line 0");
        assert!(texts[5].contains("log output truncated"), "{texts:?}");
    }

    #[tokio::test]
    async fn a_restarted_writer_continues_after_the_stored_lines() {
        let Some(db) = TestDb::new().await else { return };
        let build = seed_build(&db.pool).await;
        for round in 0..2 {
            let (tx, rx) = flume::unbounded();
            let writer = spawn_writer(db.pool.clone(), build, rx, 100);
            tx.send(line("build", &format!("round {round}"), false)).unwrap();
            drop(tx);
            writer.await.unwrap();
        }
        let seqs: Vec<i64> =
            sqlx::query_scalar("SELECT seq FROM build_logs WHERE build_id = $1 ORDER BY seq")
                .bind(build)
                .fetch_all(&db.pool)
                .await
                .unwrap();
        assert_eq!(seqs, vec![0, 1]);
    }
}
