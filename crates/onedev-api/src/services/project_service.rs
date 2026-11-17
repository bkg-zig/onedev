//! Project Service

use onedev_db::DbPool;

pub struct ProjectService {
    db_pool: DbPool,
}

impl ProjectService {
    pub fn new(db_pool: DbPool) -> Self {
        Self { db_pool }
    }
}