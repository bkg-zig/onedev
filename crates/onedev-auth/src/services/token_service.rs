//! Token Service - Access token management

use crate::models::token::{JwtService, TokenConfig, TokenPair, Claims, TokenType, JwtError};
use onedev_core::models::access_token::{AccessToken, AccessTokenType, TokenScope, CreateAccessToken};
use onedev_db::repositories::AccessTokenRepository;
use onedev_db::{DbPool, DbResult};
use chrono::{DateTime, Utc};

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

        let db_create = onedev_db::models::access_token::DbCreateAccessToken {
            name: create.name,
            token_hash,
            token_prefix,
            token_type: create.token_type,
            scopes: create.scopes,
            owner_id,
            impersonated_user_id: create.impersonated_user_id,
            project_id: create.project_id,
            group_id: create.group_id,
            expires_at: create.expires_at,
        };

        let token = self.repo.create(&self.db_pool, db_create).await?;
        
        Ok(AccessToken {
            id: token.id,
            uuid: token.uuid,
            name: token.name,
            token_hash: token.token_hash,
            token_prefix: token.token_prefix,
            token_type: token.token_type,
            scopes: token.scopes,
            owner_id: token.owner_id,
            impersonated_user_id: token.impersonated_user_id,
            project_id: token.project_id,
            group_id: token.group_id,
            expires_at: token.expires_at,
            last_used_at: token.last_used_at,
            last_used_ip: token.last_used_ip,
            revoked: token.revoked,
            revoked_at: token.revoked_at,
            revoked_by_id: token.revoked_by_id,
            created_at: token.created_at,
            updated_at: token.updated_at,
        })
    }

    /// Find token by hash (for PAT authentication)
    pub async fn find_by_hash(&self, hash: &str) -> DbResult<Option<AccessToken>> {
        self.repo.find_by_hash(&self.db_pool, hash).await
    }

    /// Find token by ID
    pub async fn find_by_id(&self, id: i64) -> DbResult<Option<AccessToken>> {
        self.repo.find_by_id(&self.db_pool, id).await
    }

    /// List tokens by owner
    pub async fn list_by_owner(&self, owner_id: i64) -> DbResult<Vec<AccessToken>> {
        self.repo.list_by_owner(&self.db_pool, owner_id).await
    }

    /// List tokens by project
    pub async fn list_by_project(&self, project_id: i64) -> DbResult<Vec<AccessToken>> {
        self.repo.list_by_project(&self.db_pool, project_id).await
    }

    /// List tokens by group
    pub async fn list_by_group(&self, group_id: i64) -> DbResult<Vec<AccessToken>> {
        self.repo.list_by_group(&self.db_pool, group_id).await
    }

    /// Update token
    pub async fn update(&self, id: i64, update: onedev_db::models::access_token::DbUpdateAccessToken) -> DbResult<AccessToken> {
        self.repo.update(&self.db_pool, id, update).await
    }

    /// Revoke a token
    pub async fn revoke(&self, id: i64, revoked_by: i64) -> DbResult<()> {
        self.repo.revoke(&self.db_pool, id, revoked_by).await
    }

    /// Update last used timestamp
    pub async fn update_last_used(&self, id: i64, last_used_at: DateTime<Utc>, ip: Option<String>) -> DbResult<()> {
        self.repo.update_last_used(&self.db_pool, id, last_used_at, ip).await
    }

    /// Delete expired/revoked tokens
    pub async fn cleanup(&self) -> DbResult<u64> {
        self.repo.cleanup(&self.db_pool).await
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
    pub fn create_user_token_pair(&self, user_id: i64, username: &str, scopes: Vec<String>) -> Result<TokenPair, JwtError> {
        self.jwt_service.create_access_token(user_id, username, scopes)
    }

    /// Create JWT token pair for an access token
    pub fn create_token_token_pair(&self, token_id: i64, token_name: &str, owner_id: i64, scopes: Vec<String>) -> Result<TokenPair, JwtError> {
        self.jwt_service.create_token_access_token(token_id, token_name, owner_id, scopes)
    }

    /// Create temporary JWT token
    pub fn create_temporary_token(&self, subject: &str, scopes: Vec<String>, ttl_seconds: Option<i64>) -> Result<String, JwtError> {
        self.jwt_service.create_temporary_token(subject, scopes, ttl_seconds)
    }
}