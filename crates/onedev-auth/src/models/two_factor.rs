//! Two-Factor Authentication (TOTP) support

use totp_lite::{totp, Sha1, Sha256, Sha512};
use base64::{Engine as _, engine::general_purpose};
use base64ct::Base32;
use qrcode::QrCode;
use qrcode::render::svg;
use serde::{Deserialize, Serialize};
use secrecy::{SecretString, ExposeSecret};
use thiserror::Error;
use rand::RngCore;
use crate::models::password::hash_password;

/// TOTP algorithm
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TotpAlgorithm {
    Sha1,
    Sha256,
    Sha512,
}

impl Default for TotpAlgorithm {
    fn default() -> Self {
        TotpAlgorithm::Sha1 // RFC 6238 default
    }
}

/// TOTP configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TotpConfig {
    /// Algorithm
    pub algorithm: TotpAlgorithm,
    /// Number of digits (6 or 8)
    pub digits: usize,
    /// Time step in seconds (usually 30)
    pub period: u64,
    /// Skew tolerance (number of periods to check before/after)
    pub skew: u64,
}

impl Default for TotpConfig {
    fn default() -> Self {
        Self {
            algorithm: TotpAlgorithm::Sha1,
            digits: 6,
            period: 30,
            skew: 1, // Allow 1 period before/after
        }
    }
}

/// Two-factor authentication setup data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TwoFactorSetup {
    /// Secret key (base32 encoded)
    pub secret: String,
    /// QR code as SVG string
    pub qr_code_svg: String,
    /// Backup codes (plaintext - only shown once!)
    pub backup_codes: Vec<String>,
    /// Issuer for QR code
    pub issuer: String,
    /// Account name for QR code
    pub account_name: String,
}

/// Two-factor verification input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyTwoFactor {
    /// 6-digit TOTP code
    pub code: String,
    /// Or backup code
    pub backup_code: Option<String>,
}

/// Two-factor authentication service
pub struct TwoFactorService {
    config: TotpConfig,
}

impl TwoFactorService {
    pub fn new(config: TotpConfig) -> Self {
        Self { config }
    }

    /// Generate a new TOTP secret (base32 encoded, 20 bytes = 160 bits)
    pub fn generate_secret() -> String {
        let mut bytes = [0u8; 20];
        rand::rng().fill_bytes(&mut bytes);
        Base32::encode_string(&bytes).to_lowercase()
    }

    /// Generate backup codes (10 codes, 8 chars each)
    pub fn generate_backup_codes(count: usize) -> Vec<String> {
        const CHARSET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789"; // No confusing chars
        let mut codes = Vec::with_capacity(count);
        
        for _ in 0..count {
            let mut code = String::with_capacity(8);
            let mut bytes = [0u8; 8];
            rand::rng().fill_bytes(&mut bytes);
            
            for &b in &bytes {
                code.push(CHARSET[(b as usize) % CHARSET.len()] as char);
            }
            
            // Format as XXXX-XXXX
            if code.len() == 8 {
                code.insert(4, '-');
            }
            codes.push(code);
        }
        
        codes
    }

    /// Create 2FA setup for a user
    pub fn create_setup(&self, username: &str, issuer: &str) -> TwoFactorSetup {
        let secret = Self::generate_secret();
        let backup_codes = Self::generate_backup_codes(10);
        
        // Build otpauth URI
        let encoded_issuer = urlencoding::encode(issuer);
        let encoded_account = urlencoding::encode(username);
        let otpauth_uri = format!(
            "otpauth://totp/{}:{}?secret={}&issuer={}&algorithm={}&digits={}&period={}",
            encoded_issuer,
            encoded_account,
            secret.to_uppercase(),
            encoded_issuer,
            match self.config.algorithm {
                TotpAlgorithm::Sha1 => "SHA1",
                TotpAlgorithm::Sha256 => "SHA256",
                TotpAlgorithm::Sha512 => "SHA512",
            },
            self.config.digits,
            self.config.period,
        );

        // Generate QR code as SVG
        let qr_code = QrCode::new(&otpauth_uri).expect("Valid otpauth URI");
        let qr_code_svg = qr_code
            .render::<svg::Color>()
            .min_dimensions(200, 200)
            .dark_color(svg::Color("#000000"))
            .light_color(svg::Color("#ffffff"))
            .build();

        TwoFactorSetup {
            secret,
            qr_code_svg,
            backup_codes,
            issuer: issuer.to_string(),
            account_name: username.to_string(),
        }
    }

    /// Verify a TOTP code
    pub fn verify_totp(&self, secret: &str, code: &str, timestamp: Option<u64>) -> bool {
        let code = code.replace(' ', "").replace('-', "");
        if code.len() != self.config.digits {
            return false;
        }

        let timestamp = timestamp.unwrap_or_else(|| std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() / self.config.period);

        // Check current period and skew periods before/after
        for i in 0..=self.config.skew {
            let check_time = if i == 0 {
                timestamp
            } else if timestamp >= i {
                timestamp - i
            } else {
                continue;
            };

            let expected = match self.config.algorithm {
                TotpAlgorithm::Sha1 => totp::<Sha1>(&self.decode_secret(secret), check_time, self.config.digits, self.config.period),
                TotpAlgorithm::Sha256 => totp::<Sha256>(&self.decode_secret(secret), check_time, self.config.digits, self.config.period),
                TotpAlgorithm::Sha512 => totp::<Sha512>(&self.decode_secret(secret), check_time, self.config.digits, self.config.period),
            };

            if expected == code {
                return true;
            }

            // Also check future period
            if i > 0 {
                let future_time = timestamp + i;
                let expected = match self.config.algorithm {
                    TotpAlgorithm::Sha1 => totp::<Sha1>(&self.decode_secret(secret), future_time, self.config.digits, self.config.period),
                    TotpAlgorithm::Sha256 => totp::<Sha256>(&self.decode_secret(secret), future_time, self.config.digits, self.config.period),
                    TotpAlgorithm::Sha512 => totp::<Sha512>(&self.decode_secret(secret), future_time, self.config.digits, self.config.period),
                };
                if expected == code {
                    return true;
                }
            }
        }

        false
    }

    /// Verify a backup code (and remove it from the list)
    pub fn verify_backup_code(&self, provided_code: &str, stored_codes: &mut Vec<String>) -> bool {
        let provided = provided_code.replace('-', "").to_uppercase();
        
        if let Some(pos) = stored_codes.iter().position(|c| c.replace('-', "").to_uppercase() == provided) {
            stored_codes.remove(pos);
            true
        } else {
            false
        }
    }

    /// Hash backup codes for storage (bcrypt)
    pub fn hash_backup_codes(codes: &[String]) -> Vec<String> {
        codes.iter()
            .map(|c| hash_password(c).expect("Failed to hash backup code"))
            .collect()
    }

    /// Verify a backup code against hashed codes
    pub fn verify_hashed_backup_code(&self, provided_code: &str, hashed_codes: &mut Vec<String>) -> bool {
        let provided = provided_code.replace('-', "").to_uppercase();
        
        for (i, hashed) in hashed_codes.iter().enumerate() {
            if crate::models::password::verify_password(&provided, hashed).unwrap_or(false) {
                hashed_codes.remove(i);
                return true;
            }
        }
        false
    }

    /// Get current TOTP code for a secret (for testing)
    pub fn current_code(&self, secret: &str) -> String {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() / self.config.period;

        match self.config.algorithm {
            TotpAlgorithm::Sha1 => totp::<Sha1>(&self.decode_secret(secret), timestamp, self.config.digits, self.config.period),
            TotpAlgorithm::Sha256 => totp::<Sha256>(&self.decode_secret(secret), timestamp, self.config.digits, self.config.period),
            TotpAlgorithm::Sha512 => totp::<Sha512>(&self.decode_secret(secret), timestamp, self.config.digits, self.config.period),
        }
    }

    /// Decode base32 secret
    fn decode_secret(&self, secret: &str) -> Vec<u8> {
        Base32::decode_vec(secret.to_uppercase().as_bytes()).unwrap_or_default()
    }
}

/// Two-factor errors
#[derive(Debug, Error)]
pub enum TwoFactorError {
    #[error("Invalid TOTP code")]
    InvalidCode,
    #[error("Invalid backup code")]
    InvalidBackupCode,
    #[error("No backup codes remaining")]
    NoBackupCodes,
    #[error("2FA not enabled")]
    NotEnabled,
    #[error("2FA already enabled")]
    AlreadyEnabled,
    #[error("Invalid secret")]
    InvalidSecret,
    #[error("QR code generation failed: {0}")]
    QrCodeFailed(String),
}