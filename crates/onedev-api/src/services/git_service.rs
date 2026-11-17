//! Git Service

use onedev_db::DbPool;

pub struct GitService {
    db_pool: DbPool,
}

impl GitService {
    pub fn new(db_pool: DbPool) -> Self {
        Self { db_pool }
    }
}