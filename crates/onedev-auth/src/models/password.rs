//! Password hashing with Argon2id

use argon2::{
    Algorithm, Argon2, Params, Version,
    password_hash::{
        PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng as SaltRng,
    },
};
use base64::Engine;
use rand::TryRngCore;
use rand::rngs::OsRng;
use secrecy::{ExposeSecret, SecretString};
use thiserror::Error;

/// Argon2id configuration (OWASP recommended parameters)
/// m=19456 (19 MiB), t=2, p=1
pub const ARGON2_MEMORY_COST: u32 = 19456; // 19 MiB
pub const ARGON2_TIME_COST: u32 = 2;
pub const ARGON2_PARALLELISM: u32 = 1;
pub const ARGON2_HASH_LENGTH: usize = 32;
pub const ARGON2_SALT_LENGTH: usize = 16;

/// Password hashing errors
#[derive(Debug, Error)]
pub enum PasswordError {
    #[error("Password hashing failed: {0}")]
    Hashing(String),
    #[error("Password verification failed: {0}")]
    Verification(String),
    #[error("Invalid password hash format")]
    InvalidHash,
    #[error("Password too weak: {0}")]
    TooWeak(String),
}

/// Validate password strength
pub fn validate_password_strength(password: &str) -> Result<(), PasswordError> {
    if password.len() < 8 {
        return Err(PasswordError::TooWeak(
            "Password must be at least 8 characters".to_string(),
        ));
    }
    if password.len() > 128 {
        return Err(PasswordError::TooWeak(
            "Password must not exceed 128 characters".to_string(),
        ));
    }

    // Check for at least one letter and one number
    let has_letter = password.chars().any(|c| c.is_alphabetic());
    let has_digit = password.chars().any(|c| c.is_ascii_digit());

    if !has_letter || !has_digit {
        return Err(PasswordError::TooWeak(
            "Password must contain at least one letter and one number".to_string(),
        ));
    }

    Ok(())
}

/// Hash a password using Argon2id
pub fn hash_password(password: &str) -> Result<String, PasswordError> {
    // Validate password strength
    validate_password_strength(password)?;

    // Generate salt
    let salt = SaltString::generate(&mut SaltRng);

    // Configure Argon2id
    let params = Params::new(
        ARGON2_MEMORY_COST,
        ARGON2_TIME_COST,
        ARGON2_PARALLELISM,
        Some(ARGON2_HASH_LENGTH),
    )
    .map_err(|e| PasswordError::Hashing(e.to_string()))?;

    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    // Hash password
    let password_hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| PasswordError::Hashing(e.to_string()))?
        .to_string();

    Ok(password_hash)
}

/// Verify a password against an Argon2id hash
pub fn verify_password(password: &str, hash: &str) -> Result<bool, PasswordError> {
    // Parse the hash
    let parsed_hash = PasswordHash::new(hash).map_err(|_| PasswordError::InvalidHash)?;

    // Configure Argon2id with same parameters
    let params = Params::new(
        ARGON2_MEMORY_COST,
        ARGON2_TIME_COST,
        ARGON2_PARALLELISM,
        Some(ARGON2_HASH_LENGTH),
    )
    .map_err(|e| PasswordError::Verification(e.to_string()))?;

    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    // Verify password
    match argon2.verify_password(password.as_bytes(), &parsed_hash) {
        Ok(()) => Ok(true),
        Err(argon2::password_hash::Error::Password) => Ok(false),
        Err(e) => Err(PasswordError::Verification(e.to_string())),
    }
}

/// Check if a hash needs rehashing (parameters changed)
pub fn needs_rehash(hash: &str) -> bool {
    // Try to parse and check parameters
    if let Ok(parsed) = PasswordHash::new(hash) {
        // Check if it's Argon2id with correct parameters
        if parsed.algorithm.as_str() == "argon2id" {
            {
                let params = &parsed.params;
                // Check memory cost
                if params.get("m").map(|v| v.to_string()) != Some(ARGON2_MEMORY_COST.to_string()) {
                    return true;
                }
                if params.get("t").map(|v| v.to_string()) != Some(ARGON2_TIME_COST.to_string()) {
                    return true;
                }
                if params.get("p").map(|v| v.to_string()) != Some(ARGON2_PARALLELISM.to_string()) {
                    return true;
                }
                return false;
            }
        }
    }
    true // Unknown format or wrong algorithm -> rehash
}

/// Rehash a password if needed (call after successful verification)
pub fn rehash_if_needed(password: &str, hash: &str) -> Result<Option<String>, PasswordError> {
    if needs_rehash(hash) {
        Ok(Some(hash_password(password)?))
    } else {
        Ok(None)
    }
}

/// Generate a secure random password (for service accounts, temp passwords)
pub fn generate_secure_password(length: usize) -> String {
    const CHARSET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnpqrstuvwxyz23456789!@#$%^&*";
    let mut password = String::with_capacity(length);
    let mut bytes = vec![0u8; length];
    OsRng.try_fill_bytes(&mut bytes).expect("OS random source");

    for &b in &bytes {
        password.push(CHARSET[(b as usize) % CHARSET.len()] as char);
    }

    password
}

/// Password reset token (hashed with SHA256 for storage)
pub fn hash_reset_token(token: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Verify a reset token
pub fn verify_reset_token(token: &str, hash: &str) -> bool {
    hash_reset_token(token) == hash
}

/// Generate a password reset token (URL-safe, 32 bytes)
pub fn generate_reset_token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.try_fill_bytes(&mut bytes).expect("OS random source");
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}
