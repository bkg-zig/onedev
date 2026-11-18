//! Authentication Service - Main entry point for authentication operations

use crate::models::password::{
    generate_reset_token, hash_password, hash_reset_token, validate_password_strength,
    verify_password, verify_reset_token,
};
use crate::models::principal::{AccessTokenPrincipal, UserPrincipal};
use crate::models::session::{CreateSession, Session};
use crate::models::subject::Subject;
use crate::models::token::{Claims, JwtService, TokenConfig, TokenPair, TokenType};
use crate::models::two_factor::{TotpConfig, TwoFactorModel, TwoFactorSetup};
use crate::repositories::SessionRepository;
use crate::services::token_mapping::{access_token_from_db, scopes_to_db};
use chrono::{DateTime, Utc};
use onedev_core::models::access_token::{AccessToken, CreateAccessToken};
use onedev_core::models::user::{CreateUser, User, UserRef, UserStatus, UserType};
use onedev_db::DbPool;
use onedev_db::models::access_token::DbCreateAccessToken;
use onedev_db::models::user::DbUpdateUser;
use onedev_db::repositories::Repository;
use onedev_db::repositories::{AccessTokenRepository, UserRepository};
use secrecy::SecretString;
use std::sync::Arc;
use thiserror::Error;
use uuid::Uuid;

/// Authentication errors
#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Invalid credentials")]
    InvalidCredentials,
    #[error("User not found")]
    UserNotFound,
    #[error("User disabled")]
    UserDisabled,
    #[error("User pending verification")]
    PendingVerification,
    #[error("User locked")]
    Locked,
    #[error("Invalid token")]
    InvalidToken,
    #[error("Token expired")]
    TokenExpired,
    #[error("Token revoked")]
    TokenRevoked,
    #[error("Invalid 2FA code")]
    InvalidTwoFactorCode,
    #[error("2FA required")]
    TwoFactorRequired,
    #[error("2FA not enabled")]
    TwoFactorNotEnabled,
    #[error("2FA already enabled")]
    TwoFactorAlreadyEnabled,
    #[error("Invalid backup code")]
    InvalidBackupCode,
    #[error("No backup codes remaining")]
    NoBackupCodes,
    #[error("Password too weak: {0}")]
    PasswordTooWeak(String),
    #[error("Current password incorrect")]
    CurrentPasswordIncorrect,
    #[error("Session not found")]
    SessionNotFound,
    #[error("Session expired")]
    SessionExpired,
    #[error("Access token not found")]
    AccessTokenNotFound,
    #[error("Insufficient permissions")]
    InsufficientPermissions,
    #[error("Database error: {0}")]
    Database(#[from] onedev_db::DbError),
    #[error("JWT error: {0}")]
    Jwt(#[from] crate::models::token::JwtError),
    #[error("Two-factor error: {0}")]
    TwoFactor(#[from] crate::models::two_factor::TwoFactorError),
    #[error("Password error: {0}")]
    Password(#[from] crate::models::password::PasswordError),
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Authentication service configuration
#[derive(Clone)]
pub struct AuthConfig {
    pub jwt_config: TokenConfig,
    pub totp_config: TotpConfig,
    pub session_ttl_seconds: i64,
    pub remember_me_ttl_seconds: i64,
    pub max_failed_attempts: u32,
    pub lockout_duration_seconds: i64,
    pub password_reset_ttl_seconds: i64,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            jwt_config: TokenConfig::default(),
            totp_config: TotpConfig::default(),
            session_ttl_seconds: 24 * 60 * 60,          // 24 hours
            remember_me_ttl_seconds: 30 * 24 * 60 * 60, // 30 days
            max_failed_attempts: 5,
            lockout_duration_seconds: 15 * 60,   // 15 minutes
            password_reset_ttl_seconds: 60 * 60, // 1 hour
        }
    }
}

/// Authentication result
#[derive(Debug, Clone)]
pub struct AuthResult {
    pub subject: Subject,
    pub session: Option<Session>,
    pub token_pair: Option<TokenPair>,
    pub requires_two_factor: bool,
    pub two_factor_setup: Option<TwoFactorSetup>,
}

/// Main authentication service
pub struct AuthService {
    config: AuthConfig,
    pub(crate) user_repo: UserRepository,
    token_repo: AccessTokenRepository,
    session_repo: SessionRepository,
    jwt_service: JwtService,
    two_factor_service: TwoFactorModel,
    pub(crate) db_pool: DbPool,
}

impl AuthService {
    /// Create a new authentication service
    pub fn new(config: AuthConfig, db_pool: DbPool) -> Result<Self, AuthError> {
        let jwt_service = JwtService::new(config.jwt_config.clone())?;
        let two_factor_service = TwoFactorModel::new(config.totp_config.clone());

        Ok(Self {
            config,
            user_repo: UserRepository::new(),
            token_repo: AccessTokenRepository::new(),
            session_repo: SessionRepository::new(),
            jwt_service,
            two_factor_service,
            db_pool,
        })
    }

    /// Authenticate with username/email and password
    pub async fn authenticate_password(
        &self,
        identifier: &str,
        password: &str,
        remember_me: bool,
        user_agent: Option<String>,
        ip_address: Option<String>,
    ) -> Result<AuthResult, AuthError> {
        // Find user by username or email
        let user = if identifier.contains('@') {
            self.user_repo
                .find_by_email(&self.db_pool, identifier)
                .await?
        } else {
            self.user_repo
                .find_by_name(&self.db_pool, identifier)
                .await?
        }
        .ok_or(AuthError::UserNotFound)?
        .to_core();

        // Check user status
        match user.status {
            UserStatus::Active => {}
            UserStatus::Disabled => return Err(AuthError::UserDisabled),
            UserStatus::PendingVerification => return Err(AuthError::PendingVerification),
            UserStatus::Locked => return Err(AuthError::Locked),
        }

        // Verify password
        let password_hash = user
            .password_hash
            .as_ref()
            .ok_or(AuthError::InvalidCredentials)?;

        if !verify_password(password, password_hash)? {
            // TODO: Increment failed attempts, lock if needed
            return Err(AuthError::InvalidCredentials);
        }

        // Check if 2FA is enabled
        if user.two_factor_enabled {
            return Ok(AuthResult {
                subject: Subject::Anonymous, // Will be set after 2FA
                session: None,
                token_pair: None,
                requires_two_factor: true,
                two_factor_setup: None,
            });
        }

        // Create session
        let session = self
            .create_session(&user, remember_me, user_agent, ip_address)
            .await?;

        // Generate JWT tokens
        let scopes = vec!["api".to_string(), "read_repository".to_string()]; // TODO: Get from user roles
        let token_pair = self
            .jwt_service
            .create_access_token(user.id, &user.name, scopes)?;

        let principal = UserPrincipal {
            user_id: user.id,
            username: user.name.clone(),
            user_type: user.user_type,
        };

        Ok(AuthResult {
            subject: Subject::User(principal),
            session: Some(session),
            token_pair: Some(token_pair),
            requires_two_factor: false,
            two_factor_setup: None,
        })
    }

    /// Verify 2FA code and complete authentication
    pub async fn verify_two_factor(
        &self,
        user_id: i64,
        code: &str,
        backup_code: Option<&str>,
        remember_me: bool,
        user_agent: Option<String>,
        ip_address: Option<String>,
    ) -> Result<AuthResult, AuthError> {
        let mut user = self
            .user_repo
            .find_by_id(&self.db_pool, user_id)
            .await?
            .ok_or(AuthError::UserNotFound)?
            .to_core();

        // Verify TOTP code
        if let Some(secret) = &user.two_factor_secret {
            if self.two_factor_service.verify_totp(secret, code, None) {
                // Success - create session
            } else if let Some(bc) = backup_code {
                // Try backup code
                let mut codes = user.backup_codes.clone();
                if !self
                    .two_factor_service
                    .verify_hashed_backup_code(bc, &mut codes)
                {
                    return Err(AuthError::InvalidBackupCode);
                }
                // Update backup codes
                user.backup_codes = codes;
                self.user_repo
                    .update(
                        &self.db_pool,
                        user.id,
                        DbUpdateUser {
                            backup_codes: Some(user.backup_codes.clone()),
                            ..Default::default()
                        },
                    )
                    .await?;
            } else {
                return Err(AuthError::InvalidTwoFactorCode);
            }
        } else {
            return Err(AuthError::TwoFactorNotEnabled);
        }

        // Create session
        let session = self
            .create_session(&user, remember_me, user_agent, ip_address)
            .await?;

        // Generate JWT tokens
        let scopes = vec!["api".to_string(), "read_repository".to_string()];
        let token_pair = self
            .jwt_service
            .create_access_token(user.id, &user.name, scopes)?;

        let principal = UserPrincipal {
            user_id: user.id,
            username: user.name.clone(),
            user_type: user.user_type,
        };

        Ok(AuthResult {
            subject: Subject::User(principal),
            session: Some(session),
            token_pair: Some(token_pair),
            requires_two_factor: false,
            two_factor_setup: None,
        })
    }

    /// Authenticate with access token
    pub async fn authenticate_token(&self, token: &str) -> Result<Subject, AuthError> {
        // First try JWT validation
        if let Ok(claims) = self.jwt_service.validate_access_token(token) {
            return self.subject_from_claims(&claims).await;
        }

        // Fall back to access token lookup (for PATs)
        let token_hash = AccessToken::hash_token(token);
        let db_token = self
            .token_repo
            .find_by_token_hash(&self.db_pool, &token_hash)
            .await?
            .map(access_token_from_db)
            .ok_or(AuthError::InvalidToken)?;

        if !db_token.is_active() {
            return Err(if db_token.is_expired() {
                AuthError::TokenExpired
            } else {
                AuthError::TokenRevoked
            });
        }

        // The column is stamped with NOW(); the caller clock is not stored.
        self.token_repo
            .update_last_used(&self.db_pool, db_token.id)
            .await?;

        // Get owner user
        let owner = self
            .user_repo
            .find_by_id(&self.db_pool, db_token.owner_id)
            .await?
            .ok_or(AuthError::Internal("Token owner not found".to_string()))?
            .to_core();

        let principal = AccessTokenPrincipal {
            token_id: db_token.id,
            token_name: db_token.name,
            token_type: db_token.token_type,
            owner_id: db_token.owner_id,
            owner_username: owner.name,
            scopes: db_token.scopes,
            impersonated_user_id: db_token.impersonated_user_id,
            impersonated_username: None, // TODO: Look up if needed
            project_id: db_token.project_id,
            group_id: db_token.group_id,
            expires_at: db_token.expires_at,
        };

        Ok(Subject::AccessToken(principal))
    }

    /// Create subject from JWT claims
    pub(crate) async fn subject_from_claims(&self, claims: &Claims) -> Result<Subject, AuthError> {
        if claims.sub.starts_with("u:") {
            let user_id = claims.sub[2..]
                .parse::<i64>()
                .map_err(|_| AuthError::InvalidToken)?;

            let user = self
                .user_repo
                .find_by_id(&self.db_pool, user_id)
                .await?
                .ok_or(AuthError::UserNotFound)?
                .to_core();

            if !matches!(user.status, UserStatus::Active) {
                return Err(AuthError::UserDisabled);
            }

            let principal = UserPrincipal {
                user_id: user.id,
                username: user.name.clone(),
                user_type: user.user_type,
            };

            Ok(Subject::User(principal))
        } else if claims.sub.starts_with("a:") {
            let token_id = claims.sub[2..]
                .parse::<i64>()
                .map_err(|_| AuthError::InvalidToken)?;

            let db_token = self
                .token_repo
                .find_by_id(&self.db_pool, token_id)
                .await?
                .map(access_token_from_db)
                .ok_or(AuthError::AccessTokenNotFound)?;

            if !db_token.is_active() {
                return Err(if db_token.is_expired() {
                    AuthError::TokenExpired
                } else {
                    AuthError::TokenRevoked
                });
            }

            let owner = self
                .user_repo
                .find_by_id(&self.db_pool, db_token.owner_id)
                .await?
                .ok_or(AuthError::Internal("Token owner not found".to_string()))?
                .to_core();

            let principal = AccessTokenPrincipal {
                token_id: db_token.id,
                token_name: db_token.name,
                token_type: db_token.token_type,
                owner_id: db_token.owner_id,
                owner_username: owner.name,
                scopes: db_token.scopes,
                impersonated_user_id: db_token.impersonated_user_id,
                impersonated_username: None,
                project_id: db_token.project_id,
                group_id: db_token.group_id,
                expires_at: db_token.expires_at,
            };

            Ok(Subject::AccessToken(principal))
        } else {
            Err(AuthError::InvalidToken)
        }
    }

    /// Create a user session
    async fn create_session(
        &self,
        user: &User,
        remember_me: bool,
        user_agent: Option<String>,
        ip_address: Option<String>,
    ) -> Result<Session, AuthError> {
        let create = CreateSession {
            user_id: user.id,
            username: user.name.clone(),
            user_agent,
            ip_address,
            remember_me,
            two_factor_verified: user.two_factor_enabled,
        };

        let session = Session::new(create);
        self.session_repo
            .create(&self.db_pool, session.clone())
            .await?;
        Ok(session)
    }

    /// Validate session and return subject
    pub async fn validate_session(&self, session_id: Uuid) -> Result<Subject, AuthError> {
        let session = self
            .session_repo
            .find_by_id(&self.db_pool, session_id)
            .await?
            .ok_or(AuthError::SessionNotFound)?;

        if session.is_expired() {
            self.session_repo.delete(&self.db_pool, session_id).await?;
            return Err(AuthError::SessionExpired);
        }

        let user = self
            .user_repo
            .find_by_id(&self.db_pool, session.user_id)
            .await?
            .ok_or(AuthError::UserNotFound)?
            .to_core();

        if !matches!(user.status, UserStatus::Active) {
            return Err(AuthError::UserDisabled);
        }

        // Update last accessed
        self.session_repo
            .update_last_accessed(&self.db_pool, session_id, Utc::now())
            .await?;

        let principal = UserPrincipal {
            user_id: user.id,
            username: user.name.clone(),
            user_type: user.user_type,
        };

        Ok(Subject::User(principal))
    }

    /// Revoke a session
    pub async fn revoke_session(&self, session_id: Uuid) -> Result<(), AuthError> {
        self.session_repo.delete(&self.db_pool, session_id).await?;
        Ok(())
    }

    /// Revoke all sessions for a user
    pub async fn revoke_all_sessions(&self, user_id: i64) -> Result<(), AuthError> {
        self.session_repo
            .delete_by_user(&self.db_pool, user_id)
            .await?;
        Ok(())
    }

    /// Change user password
    pub async fn change_password(
        &self,
        user_id: i64,
        current_password: &str,
        new_password: &str,
    ) -> Result<(), AuthError> {
        let user = self
            .user_repo
            .find_by_id(&self.db_pool, user_id)
            .await?
            .ok_or(AuthError::UserNotFound)?
            .to_core();

        // Verify current password
        let current_hash = user
            .password_hash
            .as_ref()
            .ok_or(AuthError::CurrentPasswordIncorrect)?;

        if !verify_password(current_password, current_hash)? {
            return Err(AuthError::CurrentPasswordIncorrect);
        }

        // Validate and hash new password
        validate_password_strength(new_password)?;
        let new_hash = hash_password(new_password)?;

        self.user_repo
            .update(
                &self.db_pool,
                user.id,
                DbUpdateUser {
                    password_hash: Some(new_hash),
                    ..Default::default()
                },
            )
            .await?;

        // Revoke all sessions (force re-login)
        self.revoke_all_sessions(user_id).await?;

        Ok(())
    }

    /// Request password reset
    pub async fn request_password_reset(&self, email: &str) -> Result<String, AuthError> {
        let user = self
            .user_repo
            .find_by_email(&self.db_pool, email)
            .await?
            .ok_or(AuthError::UserNotFound)?;

        // Generate reset token
        let token = generate_reset_token();
        let token_hash = hash_reset_token(&token);

        // Store token hash (in practice, you'd store this in a password_reset_tokens table)
        // For now, we'll use a simple approach - update user with reset token
        // TODO: Implement proper password reset token storage

        Ok(token)
    }

    /// Confirm password reset
    pub async fn confirm_password_reset(
        &self,
        token: &str,
        new_password: &str,
    ) -> Result<(), AuthError> {
        // TODO: Look up token hash from database
        // For now, placeholder
        Err(AuthError::Internal("Not implemented".to_string()))
    }

    /// Setup 2FA for a user
    pub async fn setup_two_factor(
        &self,
        user_id: i64,
        issuer: &str,
    ) -> Result<TwoFactorSetup, AuthError> {
        let user = self
            .user_repo
            .find_by_id(&self.db_pool, user_id)
            .await?
            .ok_or(AuthError::UserNotFound)?;

        if user.two_factor_enabled {
            return Err(AuthError::TwoFactorAlreadyEnabled);
        }

        let setup = self.two_factor_service.create_setup(&user.name, issuer);
        Ok(setup)
    }

    /// Enable 2FA for a user (after verification)
    pub async fn enable_two_factor(
        &self,
        user_id: i64,
        code: &str,
        secret: &str,
        backup_codes: Vec<String>,
    ) -> Result<(), AuthError> {
        let user = self
            .user_repo
            .find_by_id(&self.db_pool, user_id)
            .await?
            .ok_or(AuthError::UserNotFound)?;

        // Verify the code
        if !self.two_factor_service.verify_totp(secret, code, None) {
            return Err(AuthError::InvalidTwoFactorCode);
        }

        // Hash backup codes
        let hashed_backup_codes = TwoFactorModel::hash_backup_codes(&backup_codes);

        self.user_repo
            .update(
                &self.db_pool,
                user.id,
                DbUpdateUser {
                    two_factor_enabled: Some(true),
                    two_factor_secret: Some(secret.to_string()),
                    backup_codes: Some(hashed_backup_codes),
                    ..Default::default()
                },
            )
            .await?;

        Ok(())
    }

    /// Disable 2FA for a user
    pub async fn disable_two_factor(&self, user_id: i64, password: &str) -> Result<(), AuthError> {
        let user = self
            .user_repo
            .find_by_id(&self.db_pool, user_id)
            .await?
            .ok_or(AuthError::UserNotFound)?;

        // Verify password
        let current_hash = user
            .password_hash
            .as_ref()
            .ok_or(AuthError::CurrentPasswordIncorrect)?;

        if !verify_password(password, current_hash)? {
            return Err(AuthError::CurrentPasswordIncorrect);
        }

        // COALESCE keeps the old secret when the bind is NULL, so write an
        // empty string to wipe it. Login checks two_factor_enabled first.
        self.user_repo
            .update(
                &self.db_pool,
                user.id,
                DbUpdateUser {
                    two_factor_enabled: Some(false),
                    two_factor_secret: Some(String::new()),
                    backup_codes: Some(Vec::new()),
                    ..Default::default()
                },
            )
            .await?;

        Ok(())
    }

    /// Generate new backup codes
    pub async fn regenerate_backup_codes(
        &self,
        user_id: i64,
        password: &str,
    ) -> Result<Vec<String>, AuthError> {
        let user = self
            .user_repo
            .find_by_id(&self.db_pool, user_id)
            .await?
            .ok_or(AuthError::UserNotFound)?;

        if !user.two_factor_enabled {
            return Err(AuthError::TwoFactorNotEnabled);
        }

        // Verify password
        let current_hash = user
            .password_hash
            .as_ref()
            .ok_or(AuthError::CurrentPasswordIncorrect)?;

        if !verify_password(password, current_hash)? {
            return Err(AuthError::CurrentPasswordIncorrect);
        }

        // Generate new backup codes
        let backup_codes = TwoFactorModel::generate_backup_codes(10);
        let hashed_backup_codes = TwoFactorModel::hash_backup_codes(&backup_codes);

        self.user_repo
            .update(
                &self.db_pool,
                user.id,
                DbUpdateUser {
                    backup_codes: Some(hashed_backup_codes),
                    ..Default::default()
                },
            )
            .await?;

        Ok(backup_codes)
    }

    /// Create an access token (PAT)
    pub async fn create_access_token(
        &self,
        owner_id: i64,
        create: CreateAccessToken,
    ) -> Result<AccessToken, AuthError> {
        let owner = self
            .user_repo
            .find_by_id(&self.db_pool, owner_id)
            .await?
            .ok_or(AuthError::UserNotFound)?;

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

        let mut token =
            access_token_from_db(self.token_repo.create(&self.db_pool, db_create).await?);
        token.token_prefix = token_prefix;
        let _ = raw_token;
        Ok(token)
    }

    /// Revoke an access token.
    ///
    /// `revoked_by` is not stored: `access_tokens` only has `revoked_at`.
    pub async fn revoke_access_token(
        &self,
        token_id: i64,
        revoked_by: i64,
    ) -> Result<(), AuthError> {
        let _ = revoked_by;
        self.token_repo.revoke(&self.db_pool, token_id).await?;
        Ok(())
    }

    /// List access tokens for a user
    pub async fn list_access_tokens(&self, owner_id: i64) -> Result<Vec<AccessToken>, AuthError> {
        let tokens = self
            .token_repo
            .find_by_user_id(&self.db_pool, owner_id)
            .await?;
        Ok(tokens.into_iter().map(access_token_from_db).collect())
    }

    /// Load a user by id (core model).
    pub async fn user_by_id(&self, user_id: i64) -> Result<User, AuthError> {
        Ok(self
            .user_repo
            .find_by_id(&self.db_pool, user_id)
            .await?
            .ok_or(AuthError::UserNotFound)?
            .to_core())
    }

    /// Load a user by username or email (core model).
    pub async fn user_by_identifier(&self, identifier: &str) -> Result<User, AuthError> {
        let user = if identifier.contains('@') {
            self.user_repo
                .find_by_email(&self.db_pool, identifier)
                .await?
        } else {
            self.user_repo
                .find_by_name(&self.db_pool, identifier)
                .await?
        };
        Ok(user.ok_or(AuthError::UserNotFound)?.to_core())
    }

    /// Get the JWT service
    pub fn jwt_service(&self) -> &JwtService {
        &self.jwt_service
    }

    /// Get the 2FA service
    pub fn two_factor_service(&self) -> &TwoFactorModel {
        &self.two_factor_service
    }
}
