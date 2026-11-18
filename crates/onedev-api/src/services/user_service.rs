//! User Service — real DB-backed user operations.

use crate::errors::AuthError;
use crate::models::users::{
    AccessTokenResponse, CreateUserRequest, EmailResponse, GpgKeyResponse, SshKeyResponse,
    UpdateUserRequest, UserListItem, UserRef, UserResponse, UserStatus as ApiUserStatus,
    UserType as ApiUserType,
};
use onedev_db::DbPool;
use onedev_db::models::user::{DbCreateUser, DbUpdateUser, DbUser, DbUserType};
use onedev_db::repositories::{
    AccessTokenRepository, EmailAddressRepository, GpgKeyRepository, Repository, SshKeyRepository,
    UserRepository,
};

pub struct UserService {
    db_pool: DbPool,
    users: UserRepository,
}

pub fn user_type_api(t: DbUserType) -> ApiUserType {
    match t {
        DbUserType::Ordinary => ApiUserType::Ordinary,
        DbUserType::Service => ApiUserType::Service,
        DbUserType::Ai => ApiUserType::Ai,
    }
}

pub fn user_type_db(t: ApiUserType) -> DbUserType {
    match t {
        ApiUserType::Ordinary => DbUserType::Ordinary,
        ApiUserType::Service => DbUserType::Service,
        ApiUserType::Ai => DbUserType::Ai,
    }
}

pub fn user_response(user: &DbUser) -> UserResponse {
    UserResponse {
        id: user.id,
        uuid: user.uuid,
        name: user.name.clone(),
        full_name: user.full_name.clone(),
        email: user.email.clone(),
        user_type: user_type_api(user.user_type),
        status: match user.status {
            onedev_db::models::user::DbUserStatus::Active => ApiUserStatus::Active,
            onedev_db::models::user::DbUserStatus::Disabled => ApiUserStatus::Disabled,
            onedev_db::models::user::DbUserStatus::PendingVerification => {
                ApiUserStatus::PendingVerification
            }
            onedev_db::models::user::DbUserStatus::Locked => ApiUserStatus::Locked,
        },
        avatar_url: user.avatar_url.clone(),
        two_factor_enabled: user.two_factor_enabled,
        last_login_at: user.last_login_at,
        created_at: user.created_at,
        updated_at: user.updated_at,
        notify_own_events: user.notify_own_events,
        keep_emails_private: user.keep_emails_private,
        timezone: user.timezone.clone(),
        locale: user.locale.clone(),
    }
}

pub fn user_ref(user: &DbUser) -> UserRef {
    UserRef {
        id: user.id,
        uuid: user.uuid,
        name: user.name.clone(),
        full_name: user.full_name.clone(),
        avatar_url: user.avatar_url.clone(),
        user_type: user_type_api(user.user_type),
    }
}

fn db_err(err: onedev_db::DbError) -> AuthError {
    AuthError::Internal(err.to_string())
}

impl UserService {
    pub fn new(db_pool: DbPool) -> Self {
        Self {
            db_pool,
            users: UserRepository::new(),
        }
    }

    pub async fn list(&self, limit: i64, offset: i64) -> Result<Vec<UserListItem>, AuthError> {
        let rows = self
            .users
            .list(&self.db_pool, limit, offset)
            .await
            .map_err(db_err)?;
        Ok(rows
            .iter()
            .map(|user| UserListItem {
                user: user_ref(user),
                status: user_response(user).status,
                last_login_at: user.last_login_at,
                created_at: user.created_at,
            })
            .collect())
    }

    pub async fn count(&self) -> Result<i64, AuthError> {
        self.users.count(&self.db_pool).await.map_err(db_err)
    }

    pub async fn get(&self, id: i64) -> Result<UserResponse, AuthError> {
        let user = self
            .users
            .find_by_id(&self.db_pool, id)
            .await
            .map_err(db_err)?
            .ok_or(AuthError::UserNotFound)?;
        Ok(user_response(&user))
    }

    pub async fn create(&self, request: CreateUserRequest) -> Result<UserResponse, AuthError> {
        let password_hash = match &request.password {
            Some(password) => Some(
                onedev_auth::models::password::hash_password(password)
                    .map_err(|e| AuthError::Internal(e.to_string()))?,
            ),
            None => None,
        };
        let created = self
            .users
            .create(
                &self.db_pool,
                DbCreateUser {
                    name: request.name,
                    full_name: request.full_name,
                    email: request.email,
                    password_hash,
                    user_type: user_type_db(request.user_type),
                    avatar_url: None,
                    notify_own_events: true,
                    keep_emails_private: false,
                    timezone: "UTC".to_owned(),
                    locale: "en".to_owned(),
                },
            )
            .await
            .map_err(db_err)?;
        Ok(user_response(&created))
    }

    pub async fn update(
        &self,
        id: i64,
        request: UpdateUserRequest,
    ) -> Result<UserResponse, AuthError> {
        let updated = self
            .users
            .update(
                &self.db_pool,
                id,
                DbUpdateUser {
                    full_name: request.full_name,
                    email: request.email,
                    avatar_url: request.avatar_url,
                    notify_own_events: request.notify_own_events,
                    keep_emails_private: request.keep_emails_private,
                    timezone: request.timezone,
                    locale: request.locale,
                    ..Default::default()
                },
            )
            .await
            .map_err(db_err)?;
        Ok(user_response(&updated))
    }

    pub async fn delete(&self, id: i64) -> Result<(), AuthError> {
        self.users.delete(&self.db_pool, id).await.map_err(db_err)
    }

    pub async fn emails(&self, user_id: i64) -> Result<Vec<EmailResponse>, AuthError> {
        let rows = EmailAddressRepository::new()
            .find_by_user_id(&self.db_pool, user_id)
            .await
            .map_err(db_err)?;
        Ok(rows
            .into_iter()
            .map(|email| EmailResponse {
                id: email.id,
                uuid: email.uuid,
                email: email.value,
                verified: email.verified,
                primary: email.primary,
                created_at: email.created_at,
            })
            .collect())
    }

    pub async fn ssh_keys(&self, user_id: i64) -> Result<Vec<SshKeyResponse>, AuthError> {
        let rows = SshKeyRepository::new()
            .find_by_user_id(&self.db_pool, user_id)
            .await
            .map_err(db_err)?;
        Ok(rows
            .into_iter()
            .map(|key| SshKeyResponse {
                id: key.id,
                uuid: key.uuid,
                title: key.title,
                key_type: format!("{:?}", key.key_type),
                fingerprint: key.fingerprint_sha256,
                public_key: key.public_key,
                last_used_at: key.last_used_at,
                created_at: key.created_at,
                expires_at: key.expires_at,
            })
            .collect())
    }

    pub async fn gpg_keys(&self, user_id: i64) -> Result<Vec<GpgKeyResponse>, AuthError> {
        let rows = GpgKeyRepository::new()
            .find_by_user_id(&self.db_pool, user_id)
            .await
            .map_err(db_err)?;
        Ok(rows
            .into_iter()
            .map(|key| GpgKeyResponse {
                id: key.id,
                uuid: key.uuid,
                key_id: key.key_id,
                fingerprint: key.fingerprint,
                public_key: key.public_key,
                verified: false,
                created_at: key.created_at,
                expires_at: key.expires_at,
            })
            .collect())
    }

    pub async fn tokens(&self, user_id: i64) -> Result<Vec<AccessTokenResponse>, AuthError> {
        let rows = AccessTokenRepository::new()
            .find_by_user_id(&self.db_pool, user_id)
            .await
            .map_err(db_err)?;
        Ok(rows
            .into_iter()
            .map(|token| {
                let core = onedev_auth::services::token_mapping::access_token_from_db(token);
                AccessTokenResponse {
                    id: core.id,
                    uuid: core.uuid,
                    name: core.name,
                    token_prefix: core.token_prefix,
                    token_type: crate::models::access_tokens::AccessTokenType::Personal,
                    scopes: crate::models::access_tokens::TokenScope::API,
                    owner_id: core.owner_id,
                    impersonated_user_id: core.impersonated_user_id,
                    project_id: core.project_id,
                    group_id: core.group_id,
                    expires_at: core.expires_at,
                    last_used_at: core.last_used_at,
                    last_used_ip: core.last_used_ip,
                    revoked: core.revoked_at.is_some(),
                    revoked_at: core.revoked_at,
                    created_at: core.created_at,
                }
            })
            .collect())
    }
}
