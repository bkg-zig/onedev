//! OAuth Database Model

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// OAuth application row from database
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbOAuthApplication {
    pub id: i64,
    pub uuid: Uuid,
    pub owner_id: i64,
    pub owner_type: DbOAuthOwnerType,
    pub name: String,
    pub client_id: String,
    pub client_secret_hash: String,
    /// Stored as JSONB array in the `redirect_uris` column
    pub redirect_uris: sqlx::types::Json<Vec<String>>,
    /// Stored as JSONB array in the `scopes` column
    pub scopes: sqlx::types::Json<Vec<String>>,
    pub confidential: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// OAuth owner type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "oauth_owner_type", rename_all = "lowercase")]
pub enum DbOAuthOwnerType {
    User,
    Group,
}

/// OAuth application creation input for database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbCreateOAuthApplication {
    pub owner_id: i64,
    pub owner_type: DbOAuthOwnerType,
    pub name: String,
    pub client_id: String,
    pub client_secret_hash: String,
    pub redirect_uris: Vec<String>,
    pub scopes: Vec<String>,
    pub confidential: bool,
}

/// OAuth application update input for database
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DbUpdateOAuthApplication {
    pub name: Option<String>,
    pub redirect_uris: Option<Vec<String>>,
    pub scopes: Option<Vec<String>>,
    pub confidential: Option<bool>,
}

/// OAuth grant row from database
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbOAuthGrant {
    pub id: i64,
    pub uuid: Uuid,
    pub application_id: i64,
    pub user_id: i64,
    /// Stored as JSONB array in the `scopes` column
    pub scopes: sqlx::types::Json<Vec<String>>,
    pub redirect_uri: String,
    pub code: Option<String>,
    pub code_hash: Option<String>,
    pub code_expires_at: Option<DateTime<Utc>>,
    pub access_token_hash: Option<String>,
    pub refresh_token_hash: Option<String>,
    pub access_token_expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// OAuth grant creation input for database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbCreateOAuthGrant {
    pub application_id: i64,
    pub user_id: i64,
    pub scopes: Vec<String>,
    pub redirect_uri: String,
    pub code_hash: Option<String>,
    pub code_expires_at: Option<DateTime<Utc>>,
}

/// OAuth grant update input for database
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DbUpdateOAuthGrant {
    pub code: Option<String>,
    pub code_hash: Option<String>,
    pub access_token_hash: Option<String>,
    pub refresh_token_hash: Option<String>,
    pub access_token_expires_at: Option<DateTime<Utc>>,
}

/// Database query builder for OAuth applications
pub struct OAuthApplicationQueryBuilder;

impl OAuthApplicationQueryBuilder {
    pub fn select_all() -> &'static str {
        r#"
        SELECT id, uuid, owner_id, owner_type, name, client_id, client_secret_hash,
               redirect_uris, scopes, confidential, created_at, updated_at
        FROM oauth_applications
        "#
    }

    pub fn select_by_id() -> &'static str {
        r#"
        SELECT id, uuid, owner_id, owner_type, name, client_id, client_secret_hash,
               redirect_uris, scopes, confidential, created_at, updated_at
        FROM oauth_applications WHERE id = $1
        "#
    }

    pub fn select_by_client_id() -> &'static str {
        r#"
        SELECT id, uuid, owner_id, owner_type, name, client_id, client_secret_hash,
               redirect_uris, scopes, confidential, created_at, updated_at
        FROM oauth_applications WHERE client_id = $1
        "#
    }

    pub fn select_by_owner() -> &'static str {
        r#"
        SELECT id, uuid, owner_id, owner_type, name, client_id, client_secret_hash,
               redirect_uris, scopes, confidential, created_at, updated_at
        FROM oauth_applications WHERE owner_id = $1 AND owner_type = $2
        ORDER BY created_at DESC
        "#
    }

    pub fn insert() -> &'static str {
        r#"
        INSERT INTO oauth_applications (uuid, owner_id, owner_type, name, client_id,
                                        client_secret_hash, redirect_uris, scopes,
                                        confidential, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
        RETURNING id
        "#
    }

    pub fn update() -> &'static str {
        r#"
        UPDATE oauth_applications SET
            name = COALESCE($2, name),
            redirect_uris = COALESCE($3, redirect_uris),
            scopes = COALESCE($4, scopes),
            confidential = COALESCE($5, confidential),
            updated_at = NOW()
        WHERE id = $1
        RETURNING id, uuid, owner_id, owner_type, name, client_id, client_secret_hash,
                  redirect_uris, scopes, confidential, created_at, updated_at
        "#
    }

    pub fn delete() -> &'static str {
        "DELETE FROM oauth_applications WHERE id = $1"
    }
}

/// Database query builder for OAuth grants
pub struct OAuthGrantQueryBuilder;

impl OAuthGrantQueryBuilder {
    pub fn select_by_id() -> &'static str {
        r#"
        SELECT id, uuid, application_id, user_id, scopes, redirect_uri, code,
               code_hash, code_expires_at, access_token_hash, refresh_token_hash,
               access_token_expires_at, created_at, updated_at
        FROM oauth_grants WHERE id = $1
        "#
    }

    pub fn select_by_code_hash() -> &'static str {
        r#"
        SELECT id, uuid, application_id, user_id, scopes, redirect_uri, code,
               code_hash, code_expires_at, access_token_hash, refresh_token_hash,
               access_token_expires_at, created_at, updated_at
        FROM oauth_grants WHERE code_hash = $1
        "#
    }

    pub fn select_by_access_token_hash() -> &'static str {
        r#"
        SELECT id, uuid, application_id, user_id, scopes, redirect_uri, code,
               code_hash, code_expires_at, access_token_hash, refresh_token_hash,
               access_token_expires_at, created_at, updated_at
        FROM oauth_grants WHERE access_token_hash = $1
        "#
    }

    pub fn select_by_refresh_token_hash() -> &'static str {
        r#"
        SELECT id, uuid, application_id, user_id, scopes, redirect_uri, code,
               code_hash, code_expires_at, access_token_hash, refresh_token_hash,
               access_token_expires_at, created_at, updated_at
        FROM oauth_grants WHERE refresh_token_hash = $1
        "#
    }

    pub fn select_by_application_and_user() -> &'static str {
        r#"
        SELECT id, uuid, application_id, user_id, scopes, redirect_uri, code,
               code_hash, code_expires_at, access_token_hash, refresh_token_hash,
               access_token_expires_at, created_at, updated_at
        FROM oauth_grants WHERE application_id = $1 AND user_id = $2
        ORDER BY created_at DESC
        "#
    }

    pub fn insert() -> &'static str {
        r#"
        INSERT INTO oauth_grants (uuid, application_id, user_id, scopes, redirect_uri,
                                  code_hash, code_expires_at, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        RETURNING id
        "#
    }

    pub fn update() -> &'static str {
        r#"
        UPDATE oauth_grants SET
            code = COALESCE($2, code),
            code_hash = COALESCE($3, code_hash),
            access_token_hash = COALESCE($4, access_token_hash),
            refresh_token_hash = COALESCE($5, refresh_token_hash),
            access_token_expires_at = COALESCE($6, access_token_expires_at),
            updated_at = NOW()
        WHERE id = $1
        RETURNING id, uuid, application_id, user_id, scopes, redirect_uri, code,
                  code_hash, code_expires_at, access_token_hash, refresh_token_hash,
                  access_token_expires_at, created_at, updated_at
        "#
    }

    pub fn delete() -> &'static str {
        "DELETE FROM oauth_grants WHERE id = $1"
    }
}
