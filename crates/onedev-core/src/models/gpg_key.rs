//! GPG Key Domain Model

use super::*;

/// GPG Key entity
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GpgKey {
    pub id: i64,
    pub uuid: Uuid,
    pub user_id: i64,
    pub key_id: String, // 8 or 16 char key ID
    pub fingerprint: String, // Full fingerprint
    pub public_key: String, // Armored public key
    pub primary_key_id: Option<String>,
    pub subkeys: Vec<GpgSubkey>,
    pub user_ids: Vec<GpgUserId>,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// GPG Subkey
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GpgSubkey {
    pub key_id: String,
    pub fingerprint: String,
    pub algorithm: String,
    pub key_size: u32,
    pub capabilities: Vec<GpgKeyCapability>,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, strum::EnumString, strum::Display)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum GpgKeyCapability {
    Sign,
    Encrypt,
    Authenticate,
    Certify,
}

/// GPG User ID
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GpgUserId {
    pub name: String,
    pub email: Option<String>,
    pub comment: Option<String>,
    pub is_primary: bool,
}

/// GPG Key creation input
#[derive(Debug, Clone, Serialize, Deserialize, validator::Validate)]
pub struct CreateGpgKey {
    #[validate(length(min = 100, max = 65536))]
    pub public_key: String,
}

/// GPG Key reference
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GpgKeyRef {
    pub id: i64,
    pub uuid: Uuid,
    pub user_id: i64,
    pub key_id: String,
    pub fingerprint: String,
    pub user_ids: Vec<GpgUserId>,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl GpgKey {
    pub fn is_expired(&self) -> bool {
        self.expires_at.map_or(false, |exp| exp < Utc::now())
    }

    pub fn is_valid(&self) -> bool {
        !self.is_expired() && !self.primary_key_id.is_none()
    }

    pub fn can_sign(&self) -> bool {
        self.subkeys.iter().any(|sk| sk.capabilities.contains(&GpgKeyCapability::Sign)) ||
        self.capabilities().contains(&GpgKeyCapability::Sign)
    }

    pub fn can_encrypt(&self) -> bool {
        self.subkeys.iter().any(|sk| sk.capabilities.contains(&GpgKeyCapability::Encrypt)) ||
        self.capabilities().contains(&GpgKeyCapability::Encrypt)
    }

    pub fn can_authenticate(&self) -> bool {
        self.subkeys.iter().any(|sk| sk.capabilities.contains(&GpgKeyCapability::Authenticate)) ||
        self.capabilities().contains(&GpgKeyCapability::Authenticate)
    }

    pub fn capabilities(&self) -> Vec<GpgKeyCapability> {
        self.subkeys.iter()
            .flat_map(|sk| sk.capabilities.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect()
    }
}