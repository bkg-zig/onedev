//! Thin async wrapper around the `git` binary for maintenance and indexing.
//!
//! Commands run with a scrubbed environment (no user config, no prompts, no
//! pager), a timeout and `kill_on_drop`, so a stuck git can never wedge a
//! background service.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use anyhow::{Context, bail};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

const MAX_STDERR: usize = 2048;

#[derive(Debug, Clone)]
pub struct GitCli {
    binary: PathBuf,
}

/// One blob of a tree listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeBlob {
    pub sha: String,
    pub size: u64,
    pub path: String,
}

/// A path changed between two commits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathChange {
    /// Added or modified.
    Upsert(String),
    Deleted(String),
}

/// `git count-objects -v` figures.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ObjectStats {
    pub loose_objects: u64,
    pub loose_kib: u64,
    pub packed_objects: u64,
    pub packs: u64,
    pub pack_kib: u64,
    pub prune_packable: u64,
    pub garbage: u64,
    pub garbage_kib: u64,
}

/// A commit for the commit search index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitRecord {
    pub sha: String,
    pub author_name: String,
    pub author_email: String,
    pub committed_at: String,
    pub message: String,
}

impl GitCli {
    pub fn new(binary: impl Into<PathBuf>) -> Self {
        Self { binary: binary.into() }
    }

    fn command(&self, repo: &Path) -> Command {
        let mut cmd = Command::new(&self.binary);
        cmd.arg("--git-dir")
            .arg(repo)
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_else(|| "/usr/local/bin:/usr/bin:/bin".into()))
            .env("HOME", "/nonexistent")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_OPTIONAL_LOCKS", "0")
            .env("GIT_PAGER", "cat")
            .env("LC_ALL", "C")
            .stdin(Stdio::null())
            .kill_on_drop(true);
        cmd
    }

    /// Run git and return stdout; a non-zero exit becomes an error carrying stderr.
    pub async fn output(&self, repo: &Path, args: &[&str], timeout: Duration) -> anyhow::Result<Vec<u8>> {
        let mut cmd = self.command(repo);
        cmd.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());
        let child = cmd
            .spawn()
            .with_context(|| format!("failed to start {}", self.binary.display()))?;
        let output = tokio::time::timeout(timeout, child.wait_with_output())
            .await
            .map_err(|_| anyhow::anyhow!("git {} timed out after {:?}", args.first().unwrap_or(&""), timeout))?
            .context("failed to wait for git")?;
        if !output.status.success() {
            let mut stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            stderr.truncate(MAX_STDERR);
            bail!("git {} failed ({}): {}", args.first().unwrap_or(&""), output.status, stderr.trim());
        }
        Ok(output.stdout)
    }

    /// Resolve `rev` to a commit id; `None` when it does not exist.
    pub async fn resolve_commit(&self, repo: &Path, rev: &str) -> anyhow::Result<Option<String>> {
        let spec = format!("{rev}^{{commit}}");
        match self
            .output(repo, &["rev-parse", "--verify", "--quiet", &spec], Duration::from_secs(30))
            .await
        {
            Ok(out) => {
                let sha = String::from_utf8_lossy(&out).trim().to_owned();
                Ok((!sha.is_empty()).then_some(sha))
            }
            Err(error) => {
                // `--quiet --verify` exits 1 without output for unknown revisions.
                let message = error.to_string();
                if message.contains("failed (exit status: 1)") && message.trim_end().ends_with(':') {
                    Ok(None)
                } else {
                    Err(error)
                }
            }
        }
    }

    /// Short name of the branch `HEAD` points to (`main`), if any.
    pub async fn head_branch(&self, repo: &Path) -> anyhow::Result<Option<String>> {
        match self
            .output(repo, &["symbolic-ref", "--quiet", "--short", "HEAD"], Duration::from_secs(30))
            .await
        {
            Ok(out) => {
                let name = String::from_utf8_lossy(&out).trim().to_owned();
                Ok((!name.is_empty()).then_some(name))
            }
            Err(_) => Ok(None),
        }
    }

    /// Short names of the branches whose history contains `commit`
    /// (`git branch --contains`). Empty when the commit is on no branch
    /// (e.g. the head of a fork's pull request).
    pub async fn branches_containing(&self, repo: &Path, commit: &str) -> anyhow::Result<Vec<String>> {
        let out = self
            .output(
                repo,
                &["for-each-ref", "--contains", commit, "--format=%(refname)", "refs/heads/"],
                Duration::from_secs(60),
            )
            .await?;
        Ok(String::from_utf8_lossy(&out)
            .lines()
            .filter_map(|line| line.trim().strip_prefix("refs/heads/"))
            .map(str::to_owned)
            .collect())
    }

    /// Content of `path` at `commit`; `None` when the file does not exist.
    pub async fn show_file(&self, repo: &Path, commit: &str, path: &str) -> anyhow::Result<Option<Vec<u8>>> {
        // `ls-tree` prints nothing for a missing path but fails for a broken
        // repository or an unknown commit, so "missing" is told apart from
        // real failures without parsing error messages.
        let listing = self
            .output(
                repo,
                &["ls-tree", "--full-tree", commit, "--", path],
                Duration::from_secs(30),
            )
            .await?;
        let listing = String::from_utf8_lossy(&listing);
        let Some(entry) = listing.lines().next() else {
            return Ok(None);
        };
        // `<mode> SP <type> SP <sha> TAB <path>`
        let mut fields = entry.split(['\t', ' ']);
        let (_mode, kind, sha) = (fields.next(), fields.next(), fields.next());
        match (kind, sha) {
            (Some("blob"), Some(sha)) => Ok(Some(
                self.output(repo, &["cat-file", "blob", sha], Duration::from_secs(60))
                    .await?,
            )),
            _ => Ok(None),
        }
    }

    /// Message of a commit (`None` when there is none or the commit is unknown).
    pub async fn commit_message(&self, repo: &Path, commit: &str) -> anyhow::Result<Option<String>> {
        let out = self
            .output(repo, &["log", "-1", "--format=%B", commit], Duration::from_secs(30))
            .await?;
        let message = String::from_utf8_lossy(&out).trim().to_owned();
        Ok((!message.is_empty()).then_some(message))
    }

    /// Files a single commit changes relative to its first parent (all files
    /// of a root commit).
    pub async fn commit_changed_paths(&self, repo: &Path, commit: &str) -> anyhow::Result<Vec<String>> {
        let out = self
            .output(
                repo,
                &[
                    "show",
                    "-m",
                    "--first-parent",
                    "--no-renames",
                    "--name-only",
                    "--format=",
                    "-z",
                    commit,
                ],
                Duration::from_secs(300),
            )
            .await?;
        let mut paths: Vec<String> = out
            .split(|byte| *byte == 0)
            .filter(|name| !name.is_empty())
            .map(|name| String::from_utf8_lossy(name).into_owned())
            .collect();
        paths.sort_unstable();
        paths.dedup();
        Ok(paths)
    }

    /// Whether `ancestor` is reachable from `descendant`.
    pub async fn is_ancestor(&self, repo: &Path, ancestor: &str, descendant: &str) -> bool {
        self.output(
            repo,
            &["merge-base", "--is-ancestor", ancestor, descendant],
            Duration::from_secs(60),
        )
        .await
        .is_ok()
    }

    /// All blobs of a commit (`ls-tree -r -l`), skipping submodules.
    pub async fn list_blobs(&self, repo: &Path, commit: &str) -> anyhow::Result<Vec<TreeBlob>> {
        let out = self
            .output(
                repo,
                &["ls-tree", "-r", "-z", "-l", "--full-tree", commit],
                Duration::from_secs(300),
            )
            .await?;
        Ok(parse_ls_tree(&out))
    }

    /// Paths changed between two commits.
    pub async fn changed_paths(&self, repo: &Path, old: &str, new: &str) -> anyhow::Result<Vec<PathChange>> {
        let out = self
            .output(
                repo,
                &["diff-tree", "-r", "-z", "--no-renames", "--name-status", "--no-commit-id", old, new],
                Duration::from_secs(300),
            )
            .await?;
        Ok(parse_name_status(&out))
    }

    /// Blob ids (`<commit>:<path>`) of the given paths at `commit`.
    pub async fn blob_ids(&self, repo: &Path, commit: &str, paths: &[String]) -> anyhow::Result<Vec<TreeBlob>> {
        if paths.is_empty() {
            return Ok(Vec::new());
        }
        let mut args: Vec<&str> = vec!["ls-tree", "-z", "-l", "--full-tree", commit, "--"];
        args.extend(paths.iter().map(String::as_str));
        let out = self.output(repo, &args, Duration::from_secs(300)).await?;
        Ok(parse_ls_tree(&out))
    }

    /// Read blob contents with one `cat-file --batch` process.
    pub async fn read_blobs(&self, repo: &Path, shas: &[String]) -> anyhow::Result<Vec<(String, Vec<u8>)>> {
        if shas.is_empty() {
            return Ok(Vec::new());
        }
        let mut cmd = self.command(repo);
        cmd.args(["cat-file", "--batch"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = cmd.spawn().context("failed to start git cat-file")?;
        let mut stdin = child.stdin.take().context("cat-file stdin")?;
        let stdout = child.stdout.take().context("cat-file stdout")?;

        let requested: Vec<String> = shas.to_vec();
        // Feed requests concurrently with reading so neither pipe can fill up.
        let writer = tokio::spawn(async move {
            for sha in requested {
                if stdin.write_all(sha.as_bytes()).await.is_err() || stdin.write_all(b"\n").await.is_err() {
                    break;
                }
            }
            let _ = stdin.shutdown().await;
        });

        let mut reader = BufReader::new(stdout);
        let mut blobs = Vec::with_capacity(shas.len());
        let result: anyhow::Result<()> = async {
            for _ in 0..shas.len() {
                let mut header = String::new();
                if reader.read_line(&mut header).await? == 0 {
                    bail!("git cat-file ended early");
                }
                let parts: Vec<&str> = header.trim_end().split(' ').collect();
                match parts.as_slice() {
                    [sha, "blob", size] => {
                        let size: usize = size.parse().context("invalid blob size")?;
                        let mut content = vec![0u8; size];
                        reader.read_exact(&mut content).await?;
                        let mut newline = [0u8; 1];
                        reader.read_exact(&mut newline).await?;
                        blobs.push(((*sha).to_owned(), content));
                    }
                    [_, "missing"] => {}
                    [sha, kind, size] => {
                        // not a blob: skip its payload
                        let size: usize = size.parse().context("invalid object size")?;
                        let mut skip = vec![0u8; size + 1];
                        reader.read_exact(&mut skip).await?;
                        tracing::debug!(sha, kind, "skipping non-blob object");
                    }
                    _ => bail!("unexpected cat-file header: {}", header.trim_end()),
                }
            }
            Ok(())
        }
        .await;
        let _ = writer.await;
        let _ = child.wait().await;
        result?;
        Ok(blobs)
    }

    /// Commits reachable from `new` (and not from `old`), newest first.
    pub async fn log_commits(
        &self,
        repo: &Path,
        old: Option<&str>,
        new: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<CommitRecord>> {
        let range = match old {
            Some(old) => format!("{old}..{new}"),
            None => new.to_owned(),
        };
        let max = format!("--max-count={limit}");
        let out = self
            .output(
                repo,
                &[
                    "log",
                    "-z",
                    &max,
                    "--no-show-signature",
                    "--format=%H%x1f%an%x1f%ae%x1f%cI%x1f%B",
                    &range,
                ],
                Duration::from_secs(300),
            )
            .await?;
        Ok(parse_log(&out))
    }

    pub async fn count_objects(&self, repo: &Path) -> anyhow::Result<ObjectStats> {
        let out = self
            .output(repo, &["count-objects", "-v"], Duration::from_secs(120))
            .await?;
        Ok(parse_count_objects(&String::from_utf8_lossy(&out)))
    }
}

fn parse_ls_tree(out: &[u8]) -> Vec<TreeBlob> {
    out.split(|b| *b == 0)
        .filter(|record| !record.is_empty())
        .filter_map(|record| {
            // "<mode> <type> <sha> <size>\t<path>"
            let tab = record.iter().position(|b| *b == b'\t')?;
            let (meta, path) = record.split_at(tab);
            let path = std::str::from_utf8(&path[1..]).ok()?.to_owned();
            let meta = std::str::from_utf8(meta).ok()?;
            let mut fields = meta.split_whitespace();
            let _mode = fields.next()?;
            let kind = fields.next()?;
            let sha = fields.next()?;
            let size = fields.next()?;
            if kind != "blob" {
                return None;
            }
            Some(TreeBlob {
                sha: sha.to_owned(),
                size: size.parse().ok()?,
                path,
            })
        })
        .collect()
}

fn parse_name_status(out: &[u8]) -> Vec<PathChange> {
    let mut fields = out.split(|b| *b == 0).filter(|f| !f.is_empty());
    let mut changes = Vec::new();
    while let Some(status) = fields.next() {
        let Some(path) = fields.next() else { break };
        let Ok(path) = std::str::from_utf8(path) else { continue };
        match status.first() {
            Some(b'D') => changes.push(PathChange::Deleted(path.to_owned())),
            Some(b'A' | b'M' | b'T') => changes.push(PathChange::Upsert(path.to_owned())),
            _ => {}
        }
    }
    changes
}

fn parse_log(out: &[u8]) -> Vec<CommitRecord> {
    out.split(|b| *b == 0)
        .filter(|record| !record.is_empty())
        .filter_map(|record| {
            let text = std::str::from_utf8(record).ok()?;
            let mut parts = text.splitn(5, '\u{1f}');
            Some(CommitRecord {
                sha: parts.next()?.trim().to_owned(),
                author_name: parts.next()?.to_owned(),
                author_email: parts.next()?.to_owned(),
                committed_at: parts.next()?.to_owned(),
                message: parts.next()?.trim_end().to_owned(),
            })
        })
        .collect()
}

fn parse_count_objects(text: &str) -> ObjectStats {
    let mut stats = ObjectStats::default();
    for line in text.lines() {
        let Some((key, value)) = line.split_once(':') else { continue };
        let value: u64 = value.trim().parse().unwrap_or(0);
        match key.trim() {
            "count" => stats.loose_objects = value,
            "size" => stats.loose_kib = value,
            "in-pack" => stats.packed_objects = value,
            "packs" => stats.packs = value,
            "size-pack" => stats.pack_kib = value,
            "prune-packable" => stats.prune_packable = value,
            "garbage" => stats.garbage = value,
            "size-garbage" => stats.garbage_kib = value,
            _ => {}
        }
    }
    stats
}

/// Heuristic used by the code indexer: UTF-8 without NUL bytes.
pub fn is_indexable_text(bytes: &[u8]) -> bool {
    !bytes.contains(&0) && std::str::from_utf8(bytes).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command as StdCommand;

    fn git(dir: &Path, args: &[&str]) {
        let status = StdCommand::new("git")
            .current_dir(dir)
            .args(["-c", "user.name=T", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false"])
            .args(args)
            .env("GIT_AUTHOR_DATE", "2024-01-02T03:04:05Z")
            .env("GIT_COMMITTER_DATE", "2024-01-02T03:04:05Z")
            .status()
            .expect("git runs");
        assert!(status.success(), "git {args:?} failed");
    }

    /// Bare repository with two commits: (a.txt, dir/b.txt) then (a.txt changed, dir/b.txt removed, bin).
    fn fixture() -> (tempfile::TempDir, PathBuf, String, String) {
        let tmp = tempfile::tempdir().unwrap();
        let work = tmp.path().join("work");
        std::fs::create_dir_all(work.join("dir")).unwrap();
        git(&work, &["init", "-q", "-b", "main"]);
        std::fs::write(work.join("a.txt"), "alpha\n").unwrap();
        std::fs::write(work.join("dir/b.txt"), "bravo\n").unwrap();
        git(&work, &["add", "."]);
        git(&work, &["commit", "-q", "-m", "first commit\n\nbody line"]);
        let first = String::from_utf8(
            StdCommand::new("git").current_dir(&work).args(["rev-parse", "HEAD"]).output().unwrap().stdout,
        )
        .unwrap()
        .trim()
        .to_owned();
        std::fs::write(work.join("a.txt"), "alpha changed\n").unwrap();
        std::fs::remove_file(work.join("dir/b.txt")).unwrap();
        std::fs::write(work.join("bin.dat"), [0u8, 1, 2, 3]).unwrap();
        git(&work, &["add", "-A"]);
        git(&work, &["commit", "-q", "-m", "second"]);
        let second = String::from_utf8(
            StdCommand::new("git").current_dir(&work).args(["rev-parse", "HEAD"]).output().unwrap().stdout,
        )
        .unwrap()
        .trim()
        .to_owned();
        let bare = tmp.path().join("repo.git");
        let status = StdCommand::new("git")
            .args(["clone", "-q", "--bare"])
            .arg(&work)
            .arg(&bare)
            .status()
            .unwrap();
        assert!(status.success());
        (tmp, bare, first, second)
    }

    #[tokio::test]
    async fn resolves_commits_and_head() {
        let (_tmp, repo, first, second) = fixture();
        let cli = GitCli::new("git");
        assert_eq!(cli.resolve_commit(&repo, "main").await.unwrap(), Some(second.clone()));
        assert_eq!(cli.resolve_commit(&repo, &first).await.unwrap(), Some(first.clone()));
        assert_eq!(cli.resolve_commit(&repo, "does-not-exist").await.unwrap(), None);
        assert_eq!(cli.head_branch(&repo).await.unwrap().as_deref(), Some("main"));
        assert!(cli.is_ancestor(&repo, &first, &second).await);
        assert!(!cli.is_ancestor(&repo, &second, &first).await);
    }

    #[tokio::test]
    async fn lists_blobs_diffs_and_reads_content() {
        let (_tmp, repo, first, second) = fixture();
        let cli = GitCli::new("git");

        let mut blobs = cli.list_blobs(&repo, &second).await.unwrap();
        blobs.sort_by(|a, b| a.path.cmp(&b.path));
        assert_eq!(
            blobs.iter().map(|b| b.path.as_str()).collect::<Vec<_>>(),
            vec!["a.txt", "bin.dat"]
        );
        assert_eq!(blobs[0].size, "alpha changed\n".len() as u64);

        let mut changes = cli.changed_paths(&repo, &first, &second).await.unwrap();
        changes.sort_by_key(|c| format!("{c:?}"));
        assert!(changes.contains(&PathChange::Upsert("a.txt".into())));
        assert!(changes.contains(&PathChange::Upsert("bin.dat".into())));
        assert!(changes.contains(&PathChange::Deleted("dir/b.txt".into())));

        let ids = cli
            .blob_ids(&repo, &second, &["a.txt".to_owned(), "bin.dat".to_owned()])
            .await
            .unwrap();
        assert_eq!(ids.len(), 2);
        let shas: Vec<String> = ids.iter().map(|b| b.sha.clone()).collect();
        let contents = cli.read_blobs(&repo, &shas).await.unwrap();
        assert_eq!(contents.len(), 2);
        let by_sha: std::collections::HashMap<_, _> = contents.into_iter().collect();
        let a = ids.iter().find(|b| b.path == "a.txt").unwrap();
        let bin = ids.iter().find(|b| b.path == "bin.dat").unwrap();
        assert_eq!(by_sha[&a.sha], b"alpha changed\n");
        assert!(is_indexable_text(&by_sha[&a.sha]));
        assert!(!is_indexable_text(&by_sha[&bin.sha]));
    }

    #[tokio::test]
    async fn reads_commit_log_incrementally() {
        let (_tmp, repo, first, second) = fixture();
        let cli = GitCli::new("git");
        let all = cli.log_commits(&repo, None, "main", 100).await.unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].sha, second);
        assert_eq!(all[1].sha, first);
        assert_eq!(all[1].author_name, "T");
        assert_eq!(all[1].author_email, "t@example.com");
        assert!(all[1].message.starts_with("first commit"));
        assert!(all[1].message.contains("body line"));
        assert!(all[1].committed_at.starts_with("2024-01-02T03:04:05"));
        let incremental = cli.log_commits(&repo, Some(&first), "main", 100).await.unwrap();
        assert_eq!(incremental.len(), 1);
        assert_eq!(incremental[0].sha, second);
    }

    #[tokio::test]
    async fn counts_objects_and_reports_git_failures() {
        let (_tmp, repo, _, _) = fixture();
        let cli = GitCli::new("git");
        let stats = cli.count_objects(&repo).await.unwrap();
        assert!(stats.packed_objects + stats.loose_objects > 0);
        let error = cli
            .output(&repo, &["definitely-not-a-command"], Duration::from_secs(10))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("failed"), "{error}");
        let missing = cli
            .output(&repo.join("nope.git"), &["rev-parse", "HEAD"], Duration::from_secs(10))
            .await;
        assert!(missing.is_err());
    }

    #[test]
    fn parsers_handle_edge_cases() {
        let tree = b"100644 blob abc123   12\tdir/with space.txt\0160000 commit def456       -\tsubmodule\0";
        let blobs = parse_ls_tree(tree);
        assert_eq!(blobs.len(), 1);
        assert_eq!(blobs[0].path, "dir/with space.txt");
        assert_eq!(blobs[0].size, 12);

        let status = b"M\0a.txt\0D\0old.txt\0A\0new.txt\0";
        assert_eq!(parse_name_status(status).len(), 3);

        let stats = parse_count_objects("count: 3\nsize: 12\nin-pack: 10\npacks: 1\nsize-pack: 4\nprune-packable: 0\ngarbage: 2\nsize-garbage: 1\n");
        assert_eq!(stats.loose_objects, 3);
        assert_eq!(stats.packed_objects, 10);
        assert_eq!(stats.garbage, 2);
    }
}
