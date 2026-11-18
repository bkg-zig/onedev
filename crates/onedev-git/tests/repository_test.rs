use std::process::Command;

use onedev_git::models::{GitConfig, GitService, TreeEntryKind};
use onedev_git::services::repository::RepositoryService;
use onedev_git::services::smart_http::SmartHttpService;
use onedev_git::utils::encode_pkt_line;

fn git(args: &[&str], dir: &std::path::Path) {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Create a bare repo through the service, push one commit with the real
/// git client, then read it back through gix.
fn seeded_service(root: &std::path::Path) -> RepositoryService {
    let service = RepositoryService::new(GitConfig {
        root: root.to_path_buf(),
        ..GitConfig::default()
    });
    service.create_repository("acme/app").unwrap();
    assert!(service.repository_exists("acme/app"));

    let work = root.join("work");
    std::fs::create_dir_all(&work).unwrap();
    let bare = root.join("acme/app.git");
    git(&["clone", bare.to_str().unwrap(), "clone"], root);
    let clone = root.join("clone");
    git(&["checkout", "-b", "main"], &clone);
    std::fs::write(clone.join("README.md"), "# App\n").unwrap();
    std::fs::create_dir_all(clone.join("src")).unwrap();
    std::fs::write(clone.join("src/main.rs"), "fn main() {}\n").unwrap();
    git(&["add", "."], &clone);
    git(&["commit", "-m", "initial commit"], &clone);
    git(&["push", "origin", "main"], &clone);
    drop(work);
    service
}

#[test]
fn reads_refs_tree_blob_and_history() {
    let root = tempfile::tempdir().unwrap();
    let service = seeded_service(root.path());

    let branches = service.list_branches("acme/app").unwrap();
    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0].short_name(), "main");

    let commit = service.resolve_commit("acme/app", "main").unwrap();
    assert_eq!(commit.message.trim(), "initial commit");
    assert_eq!(commit.author.name, "Test");
    assert!(commit.parents.is_empty());

    let tree = service.list_tree("acme/app", "main", "").unwrap();
    let names: Vec<&str> = tree.iter().map(|e| e.path.as_str()).collect();
    assert!(names.contains(&"README.md"));
    assert!(names.contains(&"src"));
    assert!(tree.iter().any(|e| e.kind == TreeEntryKind::Tree));

    let src = service.list_tree("acme/app", "main", "src").unwrap();
    assert_eq!(src.len(), 1);
    assert_eq!(src[0].path, "src/main.rs");

    let blob = service.read_blob("acme/app", "main", "README.md").unwrap();
    assert_eq!(blob.text(), Some("# App\n"));
    assert!(!blob.is_lfs_pointer);

    let history = service.history("acme/app", "main", 10).unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].id, commit.id);
}

#[test]
fn blame_submodules_and_last_commits() {
    let root = tempfile::tempdir().unwrap();
    let service = seeded_service(root.path());

    let blame = service.blame("acme/app", "main", "README.md").unwrap();
    assert_eq!(blame.len(), 1);
    assert_eq!(blame[0].author.name, "Test");
    assert_eq!(blame[0].lines, vec![1]);

    assert!(service.submodules("acme/app", "main").unwrap().is_empty());

    let children = service
        .last_commits_of_children("acme/app", "main", "")
        .unwrap();
    assert_eq!(children.len(), 2);
    assert!(
        children
            .iter()
            .all(|child| child.commit.message.trim() == "initial commit")
    );
}

#[test]
fn smart_http_advertises_refs_and_serves_upload_pack() {
    let root = tempfile::tempdir().unwrap();
    let service = seeded_service(root.path());
    let http = SmartHttpService::new(service.config().clone());

    let advertisement = http.info_refs("acme/app", GitService::UploadPack).unwrap();
    assert_eq!(
        advertisement.content_type,
        "application/x-git-upload-pack-advertisement"
    );
    let body = String::from_utf8_lossy(&advertisement.body);
    assert!(body.contains("# service=git-upload-pack"));
    assert!(body.contains("refs/heads/main"));

    // Minimal stateless upload-pack request: want HEAD, then done.
    let head = service.resolve_commit("acme/app", "main").unwrap().id;
    let mut request = encode_pkt_line(format!("want {head}\n").as_bytes());
    request.extend_from_slice(b"0000");
    request.extend_from_slice(&encode_pkt_line(b"done\n"));

    let response = http
        .rpc("acme/app", GitService::UploadPack, &request, None, None)
        .unwrap();
    assert_eq!(response.status, 200);
    assert!(
        response.body.windows(4).any(|w| w == b"PACK"),
        "upload-pack response must contain a pack file"
    );
}
