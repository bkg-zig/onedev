//! Password and bearer-token primitives.
//!
//! Passwords are stored as Argon2id PHC strings. API tokens are random UUIDs;
//! only their SHA-256 digest is persisted, so a database disclosure does not
//! expose usable bearer credentials.

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::{Argon2, Algorithm, Version};
use rand_core::OsRng;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use crate::{CoreError, CoreResult};

pub fn hash_password(password: &str) -> CoreResult<String> {
    validate_password(password)?;
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, argon2::Params::default());
    argon2
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|error| CoreError::Failed(format!("password hashing failed: {error}")))
}

pub fn verify_password(password: &str, encoded: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(encoded) else {
        return false;
    };
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, argon2::Params::default());
    argon2
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

pub fn generate_access_token() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

pub fn token_digest(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn tokens_equal(left: &str, right: &str) -> bool {
    left.as_bytes().ct_eq(right.as_bytes()).into()
}

fn validate_password(password: &str) -> CoreResult<()> {
    if password.chars().count() < 8 {
        return Err(CoreError::validation(
            "password must contain at least 8 characters",
        ));
    }
    if password.chars().count() > 1024 {
        return Err(CoreError::validation("password is too long"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_round_trip() {
        let hash = hash_password("correct horse battery staple").unwrap();
        assert!(verify_password("correct horse battery staple", &hash));
        assert!(!verify_password("wrong", &hash));
        assert!(!hash.contains("correct horse"));
    }

    #[test]
    fn short_password_is_rejected() {
        assert!(matches!(
            hash_password("short"),
            Err(CoreError::Validation(_))
        ));
    }

    #[test]
    fn token_digest_is_stable_and_not_reversible() {
        let token = generate_access_token();
        let digest = token_digest(&token);
        assert_eq!(digest.len(), 64);
        assert_eq!(digest, token_digest(&token));
        assert!(!digest.contains(&token));
    }
}
