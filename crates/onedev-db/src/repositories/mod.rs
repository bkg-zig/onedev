//! Repository Traits and Implementations

pub mod user;
pub mod project;
pub mod issue;
pub mod pull_request;
pub mod build;
pub mod workspace;
pub mod pack;
pub mod group;
pub mod role;
pub mod setting;
pub mod access_token;
pub mod email;
pub mod ssh_key;
pub mod gpg_key;
pub mod oauth;
pub mod audit;
pub mod dashboard;
pub mod code_comment;
pub mod webhook;
pub mod label;
pub mod membership;
pub mod sso_account;
pub mod agent;
pub mod pipeline;
pub mod authorization;
pub mod watch;

pub use user::*;
pub use project::*;
pub use issue::*;
pub use pull_request::*;
pub use build::*;
pub use workspace::*;
pub use pack::*;
pub use group::*;
pub use role::*;
pub use setting::*;
pub use access_token::*;
pub use email::*;
pub use ssh_key::*;
pub use gpg_key::*;
pub use oauth::*;
pub use audit::*;
pub use dashboard::*;
pub use code_comment::*;
pub use webhook::*;
pub use label::*;
pub use membership::*;
pub use sso_account::*;
pub use agent::*;
pub use pipeline::*;
pub use authorization::*;
pub use watch::*;

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