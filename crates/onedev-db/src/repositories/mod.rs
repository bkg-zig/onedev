//! Repository Traits and Implementations

pub mod access_token;
pub mod agent;
pub mod audit;
pub mod authorization;
pub mod build;
pub mod code_comment;
pub mod dashboard;
pub mod email;
pub mod gpg_key;
pub mod group;
pub mod issue;
pub mod label;
pub mod membership;
pub mod oauth;
pub mod pack;
pub mod pipeline;
pub mod project;
pub mod pull_request;
pub mod role;
pub mod setting;
pub mod ssh_key;
pub mod sso_account;
pub mod user;
pub mod watch;
pub mod webhook;
pub mod workspace;

pub use access_token::*;
pub use agent::*;
pub use audit::*;
pub use authorization::*;
pub use build::*;
pub use code_comment::*;
pub use dashboard::*;
pub use email::*;
pub use gpg_key::*;
pub use group::*;
pub use issue::*;
pub use label::*;
pub use membership::*;
pub use oauth::*;
pub use pack::*;
pub use pipeline::*;
pub use project::*;
pub use pull_request::*;
pub use role::*;
pub use setting::*;
pub use ssh_key::*;
pub use sso_account::*;
pub use user::*;
pub use watch::*;
pub use webhook::*;
pub use workspace::*;

use crate::{DbPool, DbResult};
use async_trait::async_trait;

/// Base repository trait
#[async_trait]
pub trait Repository<T, Create, Update> {
    async fn create(&self, pool: &DbPool, create: Create) -> DbResult<T>;
    async fn find_by_id(&self, pool: &DbPool, id: i64) -> DbResult<Option<T>>;
    async fn update(&self, pool: &DbPool, id: i64, update: Update) -> DbResult<T>;
    async fn delete(&self, pool: &DbPool, id: i64) -> DbResult<()>;
    async fn list(&self, pool: &DbPool, limit: i64, offset: i64) -> DbResult<Vec<T>>;
    async fn count(&self, pool: &DbPool) -> DbResult<i64>;
}
