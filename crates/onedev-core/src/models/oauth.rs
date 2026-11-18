//! OAuth / SSO Domain Model

use super::*;

/// OAuth Provider type
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum OAuthProviderType {
    Generic,
    GitHub,
    GitLab,
    Bitbucket,
    Google,
    Microsoft,
    Okta,
    Auth0,
    Keycloak,
    Ldap,
    Saml,
    OpenIdConnect,
}

/// OAuth Application (for external apps to connect to OneDev)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OAuthApplication {
    pub id: i64,
    pub uuid: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub owner_id: i64, // User who created the app
    pub redirect_uris: Vec<String>,
    pub scopes: Vec<String>,
    pub client_id: String,
    pub client_secret_hash: String,   // Hashed
    pub client_secret_prefix: String, // First 8 chars
    pub confidential: bool,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// OAuth Grant (authorization code / token)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OAuthGrant {
    pub id: i64,
    pub uuid: Uuid,
    pub application_id: i64,
    pub user_id: i64,
    pub code: Option<String>,               // Authorization code
    pub code_hash: Option<String>,          // Hashed code
    pub access_token_hash: Option<String>,  // Hashed access token
    pub refresh_token_hash: Option<String>, // Hashed refresh token
    pub scopes: Vec<String>,
    pub redirect_uri: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// OAuth Provider (for external login - SSO)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OAuthProvider {
    pub id: i64,
    pub uuid: Uuid,
    pub name: String,
    pub provider_type: OAuthProviderType,
    pub enabled: bool,
    pub config: serde_json::Value, // Provider-specific config
    // Client credentials
    pub client_id: String,
    pub client_secret: String, // Encrypted
    // Endpoints
    pub auth_url: String,
    pub token_url: String,
    pub user_info_url: String,
    pub jwks_url: Option<String>,
    // Scopes
    pub scopes: Vec<String>,
    // Mapping
    pub username_claim: String,
    pub email_claim: String,
    pub name_claim: Option<String>,
    pub avatar_claim: Option<String>,
    pub groups_claim: Option<String>,
    // Settings
    pub auto_create_user: bool,
    pub auto_link_user: bool,
    pub sync_groups: bool,
    pub allowed_domains: Vec<String>,
    pub blocked_domains: Vec<String>,
    // Timestamps
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// SSO Account (link between user and external identity)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SsoAccount {
    pub id: i64,
    pub uuid: Uuid,
    pub user_id: i64,
    pub provider_id: i64,
    pub external_id: String, // Provider's user ID
    pub external_username: Option<String>,
    pub external_email: Option<String>,
    pub external_name: Option<String>,
    pub external_avatar_url: Option<String>,
    pub raw_data: serde_json::Value,   // Full provider response
    pub access_token: Option<String>,  // Encrypted
    pub refresh_token: Option<String>, // Encrypted
    pub token_expires_at: Option<DateTime<Utc>>,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// OAuth Application creation input
#[derive(Debug, Clone, Serialize, Deserialize, validator::Validate)]
pub struct CreateOAuthApplication {
    #[validate(length(min = 1, max = 100))]
    pub name: String,

    pub description: Option<String>,
    pub redirect_uris: Vec<String>,
    pub scopes: Vec<String>,
    pub confidential: bool,
}

/// OAuth Provider creation input
#[derive(Debug, Clone, Serialize, Deserialize, validator::Validate)]
pub struct CreateOAuthProvider {
    #[validate(length(min = 1, max = 100))]
    pub name: String,

    pub provider_type: OAuthProviderType,
    pub client_id: String,
    pub client_secret: String,
    pub auth_url: String,
    pub token_url: String,
    pub user_info_url: String,
    pub jwks_url: Option<String>,
    pub scopes: Vec<String>,
    pub username_claim: String,
    pub email_claim: String,
    pub name_claim: Option<String>,
    pub avatar_claim: Option<String>,
    pub groups_claim: Option<String>,
    pub auto_create_user: bool,
    pub auto_link_user: bool,
    pub sync_groups: bool,
    pub allowed_domains: Vec<String>,
    pub blocked_domains: Vec<String>,
}

impl OAuthApplication {
    pub fn generate_client_id() -> String {
        format!(
            "od_{}",
            &uuid::Uuid::new_v4().simple().to_string().replace('-', "")[..20]
        )
    }

    pub fn generate_client_secret() -> String {
        use rand::Rng;
        const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-";
        let mut rng = rand::rng();
        (0..64)
            .map(|_| CHARSET[rng.random_range(0..CHARSET.len())] as char)
            .collect()
    }

    pub fn hash_secret(secret: &str) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(secret.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    pub fn verify_secret(&self, secret: &str) -> bool {
        Self::hash_secret(secret) == self.client_secret_hash
    }
}

impl OAuthProvider {
    pub fn default_scopes(&self) -> Vec<String> {
        match self.provider_type {
            OAuthProviderType::GitHub => vec!["read:user".to_string(), "user:email".to_string()],
            OAuthProviderType::GitLab => vec!["read_user".to_string(), "api".to_string()],
            OAuthProviderType::Google => vec![
                "openid".to_string(),
                "email".to_string(),
                "profile".to_string(),
            ],
            OAuthProviderType::Microsoft => vec![
                "openid".to_string(),
                "profile".to_string(),
                "email".to_string(),
            ],
            OAuthProviderType::OpenIdConnect => vec![
                "openid".to_string(),
                "profile".to_string(),
                "email".to_string(),
            ],
            _ => vec![
                "openid".to_string(),
                "profile".to_string(),
                "email".to_string(),
            ],
        }
    }
}
