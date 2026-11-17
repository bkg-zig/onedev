use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{CoreError, CoreResult};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum UserType {
    ORDINARY,
    SERVICE,
    AI,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: i64,
    #[serde(default)]
    pub disabled: bool,
    #[serde(rename = "type")]
    pub user_type: UserType,
    pub name: String,
    #[serde(default)]
    pub full_name: String,
    #[serde(default)]
    pub notify_own_events: bool,
    pub create_date: DateTime<Utc>,
    #[serde(default)]
    pub email_address: String,
    #[serde(default)]
    pub authentication_sources: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserCreate {
    #[serde(rename = "type")]
    pub user_type: UserType,
    pub name: String,
    #[serde(default)]
    pub full_name: String,
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub email_address: String,
    #[serde(default)]
    pub notify_own_events: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserUpdate {
    pub name: Option<String>,
    pub full_name: Option<String>,
    pub notify_own_events: Option<bool>,
    pub keep_email_addresses_private: Option<bool>,
}

impl UserCreate {
    pub fn validate(&self) -> CoreResult<()> {
        let name = self.name.trim();
        if name.is_empty() || name.len() > 255 {
            return Err(CoreError::validation(
                "user name must contain between 1 and 255 characters",
            ));
        }
        if name.chars().any(char::is_whitespace) {
            return Err(CoreError::validation(
                "user name must not contain whitespace",
            ));
        }
        if self.user_type == UserType::ORDINARY {
            if self.email_address.trim().is_empty() || !self.email_address.contains('@') {
                return Err(CoreError::validation(
                    "ordinary users require a valid email address",
                ));
            }
            if self.password.is_empty() {
                return Err(CoreError::validation(
                    "ordinary users require a password",
                ));
            }
        }
        Ok(())
    }
}
