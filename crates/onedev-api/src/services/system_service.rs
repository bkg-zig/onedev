//! System Service

use onedev_db::DbPool;

pub struct SystemService {
    db_pool: DbPool,
}

impl SystemService {
    pub fn new(db_pool: DbPool) -> Self {
        Self { db_pool }
    }
}