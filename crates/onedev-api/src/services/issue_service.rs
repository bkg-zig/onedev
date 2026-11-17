//! Issue Service

use onedev_db::DbPool;

pub struct IssueService {
    db_pool: DbPool,
}

impl IssueService {
    pub fn new(db_pool: DbPool) -> Self {
        Self { db_pool }
    }
}