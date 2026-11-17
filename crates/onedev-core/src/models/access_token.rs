//! Access Token Domain Model

use super::*;

/// Access token type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, strum::EnumString, strum::Display)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum AccessTokenType {
    /// Personal access token (user-created)
    Personal,
    /// Deploy token (project-scoped, for CI/CD)
    Deploy,
    /// Runner token (for build runners)
    Runner,
    /// Temporary token (short-lived, for specific operations)
    Temporary,
    /// OAuth token (from external provider)
    OAuth,
    /// Service account token
    ServiceAccount,
}

bitflags::bitflags! {
    /// Access token scope
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
    pub struct TokenScope: u64 {
        const API                  = 1 << 0;
        const READ_REPOSITORY      = 1 << 1;
        const WRITE_REPOSITORY     = 1 << 2;
        const READ_REGISTRY        = 1 << 3;
        const WRITE_REGISTRY       = 1 << 4;
        const READ_PACKAGE         = 1 << 5;
        const WRITE_PACKAGE        = 1 << 6;
        const READ_USER            = 1 << 7;
        const WRITE_USER           = 1 << 8;
        const ADMIN_USER           = 1 << 9;
        const ADMIN_PROJECT        = 1 << 10;
        const ADMIN_GROUP          = 1 << 11;
        const ADMIN_SYSTEM         = 1 << 12;
        const RUN_BUILDS           = 1 << 13;
        const MANAGE_BUILDS        = 1 << 14;
        const MANAGE_RUNNERS       = 1 << 15;
        const MANAGE_WEBHOOKS      = 1 << 16;
        const MANAGE_HOOKS         = 1 << 17;
        const READ_AUDIT_LOG       = 1 << 18;
        const MANAGE_CLUSTER       = 1 << 19;
        const MANAGE_KUBERNETES    = 1 << 20;
        const SUDO                 = 1 << 21;
    }
}

/// Core Access Token entity
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AccessToken {
    pub id: i64,
    pub uuid: Uuid,
    pub name: String,
    pub token_hash: String, // SHA256 hash of the token
    pub token_prefix: String, // First 8 chars for identification
    pub token_type: AccessTokenType,
    pub scopes: TokenScope,
    pub owner_id: i64,
    pub impersonated_user_id: Option<i64>, // For sudo
    // Project/Group scoping
    pub project_id: Option<i64>,
    pub group_id: Option<i64>,
    // Expiration
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub last_used_ip: Option<String>,
    // Status
    pub revoked: bool,
    pub revoked_at: Option<DateTime<Utc>>,
    pub revoked_by_id: Option<i64>,
    // Timestamps
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Access token with raw token (only returned on creation)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessTokenWithRaw {
    #[serde(flatten)]
    pub token: AccessToken,
    pub raw_token: String, // Only present on creation
}

/// Access token authorization (for project/group scoping)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AccessTokenAuthorization {
    pub id: i64,
    pub uuid: Uuid,
    pub access_token_id: i64,
    pub project_id: Option<i64>,
    pub group_id: Option<i64>,
    pub role_id: i64,
    pub created_at: DateTime<Utc>,
    pub created_by_id: i64,
}

/// Access token creation input
#[derive(Debug, Clone, Serialize, Deserialize, validator::Validate)]
pub struct CreateAccessToken {
    #[validate(length(min = 1, max = 100))]
    pub name: String,

    pub token_type: AccessTokenType,
    pub scopes: TokenScope,
    pub expires_at: Option<DateTime<Utc>>,
    pub project_id: Option<i64>,
    pub group_id: Option<i64>,
    pub impersonated_user_id: Option<i64>,
}

/// Access token update input
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateAccessToken {
    pub name: Option<String>,
    pub scopes: Option<TokenScope>,
    pub expires_at: Option<Option<DateTime<Utc>>>,
    pub revoked: Option<bool>,
}

/// Access token query params
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AccessTokenQuery {
    pub token_type: Option<AccessTokenType>,
    pub owner_id: Option<i64>,
    pub project_id: Option<i64>,
    pub group_id: Option<i64>,
    pub revoked: Option<bool>,
    pub expired: Option<bool>,
    pub sort: Option<String>,
    pub order: Option<SortOrder>,
    pub page: u32,
    pub per_page: u32,
}

/// Token validation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenValidation {
    pub valid: bool,
    pub token: Option<AccessToken>,
    pub user: Option<UserRef>,
    pub scopes: TokenScope,
    pub error: Option<String>,
}

impl AccessToken {
    pub fn is_expired(&self) -> bool {
        self.expires_at.map_or(false, |exp| exp < Utc::now())
    }

    pub fn is_active(&self) -> bool {
        !self.revoked && !self.is_expired()
    }

    pub fn can_impersonate(&self) -> bool {
        self.scopes.contains(TokenScope::SUDO) && self.impersonated_user_id.is_some()
    }

    pub fn generate_raw_token() -> String {
        // Format: onedev_<type>_<random>
        let token_type_prefix = match rand::random::<u8>() % 4 {
            0 => "pat",    // Personal Access Token
            1 => "dpt",    // Deploy Token
            2 => "rnt",    // Runner Token
            _ => "tmp",    // Temporary
        };
        let random_part: String = (0..32)
            .map(|_| {
                let idx = rand::random::<u8>() % 62;
                match idx {
                    0..=9 => (b'0' + idx) as char,
                    10..=35 => (b'a' + idx - 10) as char,
                    _ => (b'A' + idx - 36) as char,
                }
            })
            .collect();
        format!("onedev_{}_{}", token_type_prefix, random_part)
    }

    pub fn hash_token(token: &str) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(token.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    pub fn token_prefix(token: &str) -> String {
        token.chars().take(8).collect()
    }

    pub fn verify_token(&self, raw_token: &str) -> bool {
        Self::hash_token(raw_token) == self.token_hash
    }
}

/// Default scopes by token type
impl AccessTokenType {
    pub fn default_scopes(&self) -> TokenScope {
        match self {
            AccessTokenType::Personal => TokenScope::API
                | TokenScope::READ_REPOSITORY
                | TokenScope::READ_REGISTRY
                | TokenScope::READ_PACKAGE
                | TokenScope::READ_USER,
            AccessTokenType::Deploy => TokenScope::READ_REPOSITORY
                | TokenScope::WRITE_REPOSITORY
                | TokenScope::READ_REGISTRY
                | TokenScope::WRITE_REGISTRY
                | TokenScope::READ_PACKAGE
                | TokenScope::WRITE_PACKAGE
                | TokenScope::RUN_BUILDS,
            AccessTokenType::Runner => TokenScope::READ_REPOSITORY
                | TokenScope::RUN_BUILDS
                | TokenScope::MANAGE_BUILDS,
            AccessTokenType::Temporary => TokenScope::API,
            AccessTokenType::OAuth => TokenScope::API
                | TokenScope::READ_USER
                | TokenScope::READ_REPOSITORY,
            AccessTokenType::ServiceAccount => TokenScope::all(),
        }
    }
}