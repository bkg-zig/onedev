//! Group Service

use onedev_db::DbPool;

pub struct GroupService {
    db_pool: DbPool,
}

impl GroupService {
    pub fn new(db_pool: DbPool) -> Self {
        Self { db_pool }
    }
}