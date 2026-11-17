//! User Service

use onedev_db::DbPool;
use std::sync::Arc;

pub struct UserService {
    db_pool: DbPool,
}

impl UserService {
    pub fn new(db_pool: DbPool) -> Self {
        Self { db_pool }
    }
}