//! LFS Service

use onedev_db::DbPool;

pub struct LfsService {
    db_pool: DbPool,
}

impl LfsService {
    pub fn new(db_pool: DbPool) -> Self {
        Self { db_pool }
    }
}
