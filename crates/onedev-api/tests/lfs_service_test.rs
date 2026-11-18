use onedev_api::services::LfsService;
use onedev_git::models::LfsOperation;

/// End-to-end through the API's LFS service: store an object, verify the
/// batch API reports it present, download it back, and exercise locks.
#[test]
fn lfs_store_batch_download_and_locks() {
    let root = tempfile::tempdir().unwrap();
    let service = LfsService::with_root(root.path().to_path_buf());

    let content = b"large file content";
    let oid = onedev_git::utils::sha256_hex(content);
    service.upload("acme/app", &oid, content).unwrap();

    let batch = service
        .batch("acme/app", "download", vec![(oid.clone(), content.len() as u64)], |project, oid, operation| {
            format!("/api/git/{project}/info/lfs/objects/{oid}?operation={operation}")
        })
        .unwrap();
    assert_eq!(batch.objects.len(), 1);
    let action = batch.objects[0]
        .actions
        .as_ref()
        .and_then(|actions| actions.download.as_ref())
        .expect("download action");
    assert!(action.href.contains(&oid));
    assert!(matches!(
        LfsOperation::parse("download").unwrap(),
        LfsOperation::Download
    ));

    assert_eq!(service.download("acme/app", &oid).unwrap(), content);

    let lock = service.create_lock("acme/app", "assets/big.bin", "u-1").unwrap();
    assert_eq!(service.locks("acme/app", None).unwrap().len(), 1);
    service.unlock("acme/app", &lock.id, "u-1").unwrap();
    assert!(service.locks("acme/app", None).unwrap().is_empty());
}
