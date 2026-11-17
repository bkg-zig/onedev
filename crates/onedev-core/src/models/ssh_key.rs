//! SSH Key Domain Model

use super::*;

/// SSH Key type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum SshKeyType {
    Rsa,
    Ed25519,
    Ecdsa,
    Dsa, // Deprecated
}

/// SSH Key entity
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SshKey {
    pub id: i64,
    pub uuid: Uuid,
    pub user_id: i64,
    pub title: String,
    pub key_type: SshKeyType,
    pub public_key: String,
    pub fingerprint_sha256: String,
    pub fingerprint_md5: String,
    pub key_size: u32,
    pub comment: Option<String>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub last_used_ip: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// SSH Key creation input
#[derive(Debug, Clone, Serialize, Deserialize, validator::Validate)]
pub struct CreateSshKey {
    #[validate(length(min = 1, max = 100))]
    pub title: String,

    #[validate(length(min = 100, max = 8192))]
    pub public_key: String,
}

/// SSH Key reference
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SshKeyRef {
    pub id: i64,
    pub uuid: Uuid,
    pub user_id: i64,
    pub title: String,
    pub key_type: SshKeyType,
    pub fingerprint_sha256: String,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
}

impl SshKey {
    pub fn is_expired(&self) -> bool {
        self.expires_at.map_or(false, |exp| exp < Utc::now())
    }

    pub fn is_valid(&self) -> bool {
        !self.is_expired()
    }

    pub fn mask_key(&self) -> String {
        // Show only first 10 and last 10 chars
        let len = self.public_key.len();
        if len > 20 {
            format!("{}...{}", &self.public_key[..10], &self.public_key[len-10..])
        } else {
            "****".to_string()
        }
    }
}