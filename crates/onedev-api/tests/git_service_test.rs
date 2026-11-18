use onedev_api::services::GitService;
use onedev_git::models::{GitConfig, GitService as GitProtocol};
use onedev_git::services::smart_http::SmartHttpService;

/// End-to-end through the API's GitService: seed a repo via onedev-git,
/// advertise refs and serve an upload-pack request through the wired service.
#[test]
fn git_smart_http_serves_seeded_repo() {
    let root = tempfile::tempdir().unwrap();
    let config = GitConfig {
        root: root.path().to_path_buf(),
        ..GitConfig::default()
    };
    let seed = SmartHttpService::new(config);
    seed.repositories().create_repository("acme/app").unwrap();

    let service = GitService::with_root(root.path().to_path_buf());

    let advertisement = service.info_refs("acme/app", "git-upload-pack").unwrap();
    assert_eq!(
        advertisement.content_type,
        GitProtocol::UploadPack.advertisement_type()
    );
    assert!(String::from_utf8_lossy(&advertisement.body)
        .contains("# service=git-upload-pack"));

    // Empty request against an empty repo answers cleanly with 200.
    let response = service.upload_pack("acme/app", b"0000").unwrap();
    assert_eq!(response.status, 200);

    // Unknown project maps to NotFound.
    let err = service.info_refs("no/such", "git-upload-pack").unwrap_err();
    assert!(matches!(err, onedev_git::GitError::NotFound(_)));
}
