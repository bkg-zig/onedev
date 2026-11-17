//! Merge Request Service

use onedev_db::DbPool;

pub struct MergeRequestService {
    db_pool: DbPool,
}

impl MergeRequestService {
    pub fn new(db_pool: DbPool) -> Self {
        Self { db_pool }
    }
}