//! Build Service

use onedev_db::DbPool;

pub struct BuildService {
    db_pool: DbPool,
}

impl BuildService {
    pub fn new(db_pool: DbPool) -> Self {
        Self { db_pool }
    }
}