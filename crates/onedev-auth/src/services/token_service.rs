//! Token Service - Access token management

use crate::models::token::{Claims, JwtError, JwtService, TokenConfig, TokenPair, TokenType};
use crate::services::token_mapping::{access_token_from_db, scopes_to_db};
use chrono::{DateTime, Utc};
use onedev_core::models::access_token::{AccessToken, CreateAccessToken};
use onedev_db::models::access_token::{DbCreateAccessToken, DbUpdateAccessToken};
use onedev_db::repositories::{AccessTokenRepository, Repository};
use onedev_db::{DbPool, DbResult};

/// Token service
pub struct TokenService {
    repo: AccessTokenRepository,
    jwt_service: JwtService,
    db_pool: DbPool,
}

impl TokenService {
    pub fn new(config: TokenConfig, db_pool: DbPool) -> Result<Self, JwtError> {
        let jwt_service = JwtService::new(config)?;
        Ok(Self {
            repo: AccessTokenRepository::new(),
            jwt_service,
            db_pool,
        })
    }

    /// Create an access token
    pub async fn create(&self, owner_id: i64, create: CreateAccessToken) -> DbResult<AccessToken> {
        let raw_token = AccessToken::generate_raw_token();
        let token_hash = AccessToken::hash_token(&raw_token);
        let token_prefix = AccessToken::token_prefix(&raw_token);

        let db_create = DbCreateAccessToken {
            user_id: owner_id,
            name: create.name,
            token_hash,
            scopes: scopes_to_db(create.token_type, create.scopes),
            expires_at: create.expires_at,
        };

        let mut token = access_token_from_db(self.repo.create(&self.db_pool, db_create).await?);
        token.token_prefix = token_prefix;
        let _ = raw_token;
        Ok(token)
    }

    /// Find token by hash (for PAT authentication)
    pub async fn find_by_hash(&self, hash: &str) -> DbResult<Option<AccessToken>> {
        Ok(self
            .repo
            .find_by_token_hash(&self.db_pool, hash)
            .await?
            .map(access_token_from_db))
    }

    /// Find token by ID
    pub async fn find_by_id(&self, id: i64) -> DbResult<Option<AccessToken>> {
        Ok(self
            .repo
            .find_by_id(&self.db_pool, id)
            .await?
            .map(access_token_from_db))
    }

    /// List tokens by owner
    pub async fn list_by_owner(&self, owner_id: i64) -> DbResult<Vec<AccessToken>> {
        let tokens = self.repo.find_by_user_id(&self.db_pool, owner_id).await?;
        Ok(tokens.into_iter().map(access_token_from_db).collect())
    }

    /// List tokens by project.
    ///
    /// `access_tokens` is user-scoped, so a project filter matches nothing.
    pub async fn list_by_project(&self, project_id: i64) -> DbResult<Vec<AccessToken>> {
        let _ = project_id;
        Ok(Vec::new())
    }

    /// List tokens by group.
    ///
    /// `access_tokens` is user-scoped, so a group filter matches nothing.
    pub async fn list_by_group(&self, group_id: i64) -> DbResult<Vec<AccessToken>> {
        let _ = group_id;
        Ok(Vec::new())
    }

    /// Update token
    pub async fn update(&self, id: i64, update: DbUpdateAccessToken) -> DbResult<AccessToken> {
        Ok(access_token_from_db(
            self.repo.update(&self.db_pool, id, update).await?,
        ))
    }

    /// Revoke a token.
    ///
    /// `revoked_by` is not stored: the table only records `revoked_at`.
    pub async fn revoke(&self, id: i64, revoked_by: i64) -> DbResult<()> {
        let _ = revoked_by;
        self.repo.revoke(&self.db_pool, id).await
    }

    /// Update last used timestamp.
    ///
    /// The repository stamps `NOW()` and does not store the client address.
    pub async fn update_last_used(
        &self,
        id: i64,
        last_used_at: DateTime<Utc>,
        ip: Option<String>,
    ) -> DbResult<()> {
        let _ = (last_used_at, ip);
        self.repo.update_last_used(&self.db_pool, id).await
    }

    /// Delete expired/revoked tokens
    pub async fn cleanup(&self) -> DbResult<u64> {
        self.repo.cleanup_expired(&self.db_pool).await
    }

    /// Get JWT service
    pub fn jwt_service(&self) -> &JwtService {
        &self.jwt_service
    }

    /// Validate JWT access token and return claims
    pub fn validate_access_token(&self, token: &str) -> Result<Claims, JwtError> {
        self.jwt_service.validate_access_token(token)
    }

    /// Validate JWT refresh token and return claims
    pub fn validate_refresh_token(&self, token: &str) -> Result<Claims, JwtError> {
        self.jwt_service.validate_refresh_token(token)
    }

    /// Create JWT token pair for a user
    pub fn create_user_token_pair(
        &self,
        user_id: i64,
        username: &str,
        scopes: Vec<String>,
    ) -> Result<TokenPair, JwtError> {
        self.jwt_service
            .create_access_token(user_id, username, scopes)
    }

    /// Create JWT token pair for an access token
    pub fn create_token_token_pair(
        &self,
        token_id: i64,
        token_name: &str,
        owner_id: i64,
        scopes: Vec<String>,
    ) -> Result<TokenPair, JwtError> {
        self.jwt_service
            .create_token_access_token(token_id, token_name, owner_id, scopes)
    }

    /// Create temporary JWT token
    pub fn create_temporary_token(
        &self,
        subject: &str,
        scopes: Vec<String>,
        ttl_seconds: Option<i64>,
    ) -> Result<String, JwtError> {
        self.jwt_service
            .create_temporary_token(subject, scopes, ttl_seconds)
    }
}
