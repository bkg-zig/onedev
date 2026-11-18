//! Two-Factor Service - Wrapper for 2FA operations with database

use crate::models::two_factor::{TotpAlgorithm, TotpConfig, TwoFactorModel, TwoFactorSetup};
use onedev_core::models::user::User;
use onedev_db::models::user::DbUpdateUser;
use onedev_db::repositories::{Repository, UserRepository};
use onedev_db::{DbPool, DbResult};

/// Two-factor authentication service with database integration
pub struct TwoFactorService {
    model: TwoFactorModel,
    user_repo: UserRepository,
    db_pool: DbPool,
}

impl TwoFactorService {
    pub fn new(config: TotpConfig, db_pool: DbPool) -> Self {
        Self {
            model: TwoFactorModel::new(config),
            user_repo: UserRepository::new(),
            db_pool,
        }
    }

    /// Create 2FA setup for a user
    pub async fn create_setup(&self, user_id: i64, issuer: &str) -> Result<TwoFactorSetup, String> {
        let user = self
            .user_repo
            .find_by_id(&self.db_pool, user_id)
            .await
            .map_err(|e| format!("Database error: {}", e))?
            .ok_or("User not found")?;

        if user.two_factor_enabled {
            return Err("2FA already enabled".to_string());
        }

        Ok(self.model.create_setup(&user.name, issuer))
    }

    /// Enable 2FA for a user
    pub async fn enable(
        &self,
        user_id: i64,
        code: &str,
        secret: &str,
        backup_codes: Vec<String>,
    ) -> Result<(), String> {
        let mut user = self
            .user_repo
            .find_by_id(&self.db_pool, user_id)
            .await
            .map_err(|e| format!("Database error: {}", e))?
            .ok_or("User not found")?;

        // Verify the code
        if !self.model.verify_totp(secret, code, None) {
            return Err("Invalid 2FA code".to_string());
        }

        // Hash backup codes
        let hashed_backup_codes = TwoFactorModel::hash_backup_codes(&backup_codes);

        // Update user
        user.two_factor_enabled = true;
        user.two_factor_secret = Some(secret.to_string());
        user.backup_codes = hashed_backup_codes;

        self.user_repo
            .update(
                &self.db_pool,
                user.id,
                DbUpdateUser {
                    two_factor_enabled: Some(user.two_factor_enabled),
                    two_factor_secret: user.two_factor_secret.clone(),
                    backup_codes: Some(user.backup_codes.clone()),
                    ..Default::default()
                },
            )
            .await
            .map_err(|e| format!("Database error: {}", e))?;

        Ok(())
    }

    /// Disable 2FA for a user
    pub async fn disable(
        &self,
        user_id: i64,
        password: &str,
        password_hash: &str,
    ) -> Result<(), String> {
        let mut user = self
            .user_repo
            .find_by_id(&self.db_pool, user_id)
            .await
            .map_err(|e| format!("Database error: {}", e))?
            .ok_or("User not found")?;

        // Verify password
        if !crate::models::password::verify_password(password, password_hash)
            .map_err(|e| e.to_string())?
        {
            return Err("Invalid password".to_string());
        }

        // Disable 2FA
        user.two_factor_enabled = false;
        user.two_factor_secret = None;
        user.backup_codes = Vec::new();

        self.user_repo
            .update(
                &self.db_pool,
                user.id,
                DbUpdateUser {
                    two_factor_enabled: Some(user.two_factor_enabled),
                    two_factor_secret: user.two_factor_secret.clone(),
                    backup_codes: Some(user.backup_codes.clone()),
                    ..Default::default()
                },
            )
            .await
            .map_err(|e| format!("Database error: {}", e))?;

        Ok(())
    }

    /// Verify 2FA code for login
    pub async fn verify_login(
        &self,
        user: &User,
        code: &str,
        backup_code: Option<&str>,
    ) -> Result<bool, String> {
        if !user.two_factor_enabled {
            return Ok(true); // 2FA not enabled
        }

        // Try TOTP code
        if let Some(secret) = &user.two_factor_secret {
            if self.model.verify_totp(secret, code, None) {
                return Ok(true);
            }
        }

        // Try backup code
        if let Some(bc) = backup_code {
            let mut codes = user.backup_codes.clone();
            if self.model.verify_hashed_backup_code(bc, &mut codes) {
                self.user_repo
                    .update(
                        &self.db_pool,
                        user.id,
                        DbUpdateUser {
                            backup_codes: Some(codes),
                            ..Default::default()
                        },
                    )
                    .await
                    .map_err(|e| format!("Database error: {}", e))?;
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Regenerate backup codes
    pub async fn regenerate_backup_codes(
        &self,
        user_id: i64,
        password: &str,
        password_hash: &str,
    ) -> Result<Vec<String>, String> {
        let mut user = self
            .user_repo
            .find_by_id(&self.db_pool, user_id)
            .await
            .map_err(|e| format!("Database error: {}", e))?
            .ok_or("User not found")?;

        if !user.two_factor_enabled {
            return Err("2FA not enabled".to_string());
        }

        // Verify password
        if !crate::models::password::verify_password(password, password_hash)
            .map_err(|e| e.to_string())?
        {
            return Err("Invalid password".to_string());
        }

        // Generate new backup codes
        let backup_codes = TwoFactorModel::generate_backup_codes(10);
        let hashed_backup_codes = TwoFactorModel::hash_backup_codes(&backup_codes);

        user.backup_codes = hashed_backup_codes;
        self.user_repo
            .update(
                &self.db_pool,
                user.id,
                DbUpdateUser {
                    two_factor_enabled: Some(user.two_factor_enabled),
                    two_factor_secret: user.two_factor_secret.clone(),
                    backup_codes: Some(user.backup_codes.clone()),
                    ..Default::default()
                },
            )
            .await
            .map_err(|e| format!("Database error: {}", e))?;

        Ok(backup_codes)
    }

    /// Get current TOTP code (for testing/admin)
    pub fn current_code(&self, secret: &str) -> String {
        self.model.current_code(secret)
    }

    /// Get the underlying model service
    pub fn model(&self) -> &TwoFactorModel {
        &self.model
    }
}
