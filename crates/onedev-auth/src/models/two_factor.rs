//! Two-Factor Authentication (TOTP) support

use crate::models::password::hash_password;
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use qrcode::QrCode;
use qrcode::render::svg;
use rand::RngCore;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use totp_lite::{Sha1, Sha256, Sha512, totp_custom};

const BASE32_ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

fn encode_base32(data: &[u8]) -> String {
    let mut out = String::new();
    let mut buffer: u64 = 0;
    let mut bits: u32 = 0;
    for &byte in data {
        buffer = (buffer << 8) | u64::from(byte);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            let idx = ((buffer >> bits) & 0x1f) as usize;
            out.push(BASE32_ALPHABET[idx] as char);
        }
    }
    if bits > 0 {
        let idx = ((buffer << (5 - bits)) & 0x1f) as usize;
        out.push(BASE32_ALPHABET[idx] as char);
    }
    out
}

fn decode_base32(input: &str) -> Vec<u8> {
    let mut buffer: u64 = 0;
    let mut bits: u32 = 0;
    let mut out = Vec::new();
    for c in input.chars() {
        if c == '=' || c.is_whitespace() {
            continue;
        }
        let value = match c {
            'A'..='Z' => u32::from(c) - u32::from('A'),
            'a'..='z' => u32::from(c) - u32::from('a'),
            '2'..='7' => u32::from(c) - u32::from('2') + 26,
            _ => return Vec::new(),
        };
        buffer = (buffer << 5) | u64::from(value);
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
        }
    }
    out
}

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

/// Two-factor authentication model (without database)
pub struct TwoFactorModel {
    config: TotpConfig,
}

impl TwoFactorModel {
    pub fn new(config: TotpConfig) -> Self {
        Self { config }
    }

    /// Generate a new TOTP secret (base32 encoded, 20 bytes = 160 bits)
    pub fn generate_secret() -> String {
        let mut bytes = [0u8; 20];
        rand::rng().fill_bytes(&mut bytes);
        encode_base32(&bytes).to_lowercase()
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
        let encoded_issuer = utf8_percent_encode(issuer, NON_ALPHANUMERIC);
        let encoded_account = utf8_percent_encode(username, NON_ALPHANUMERIC);
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

        let timestamp = timestamp.unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
                / self.config.period
        });

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
                TotpAlgorithm::Sha1 => totp_custom::<Sha1>(
                    self.config.period,
                    self.config.digits as u32,
                    &self.decode_secret(secret),
                    check_time,
                ),
                TotpAlgorithm::Sha256 => totp_custom::<Sha256>(
                    self.config.period,
                    self.config.digits as u32,
                    &self.decode_secret(secret),
                    check_time,
                ),
                TotpAlgorithm::Sha512 => totp_custom::<Sha512>(
                    self.config.period,
                    self.config.digits as u32,
                    &self.decode_secret(secret),
                    check_time,
                ),
            };

            if expected == code {
                return true;
            }

            // Also check future period
            if i > 0 {
                let future_time = timestamp + i;
                let expected = match self.config.algorithm {
                    TotpAlgorithm::Sha1 => totp_custom::<Sha1>(
                        self.config.period,
                        self.config.digits as u32,
                        &self.decode_secret(secret),
                        future_time,
                    ),
                    TotpAlgorithm::Sha256 => totp_custom::<Sha256>(
                        self.config.period,
                        self.config.digits as u32,
                        &self.decode_secret(secret),
                        future_time,
                    ),
                    TotpAlgorithm::Sha512 => totp_custom::<Sha512>(
                        self.config.period,
                        self.config.digits as u32,
                        &self.decode_secret(secret),
                        future_time,
                    ),
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

        if let Some(pos) = stored_codes
            .iter()
            .position(|c| c.replace('-', "").to_uppercase() == provided)
        {
            stored_codes.remove(pos);
            true
        } else {
            false
        }
    }

    /// Hash backup codes for storage (bcrypt)
    pub fn hash_backup_codes(codes: &[String]) -> Vec<String> {
        codes
            .iter()
            .map(|c| hash_password(c).expect("Failed to hash backup code"))
            .collect()
    }

    /// Verify a backup code against hashed codes
    pub fn verify_hashed_backup_code(
        &self,
        provided_code: &str,
        hashed_codes: &mut Vec<String>,
    ) -> bool {
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
            .as_secs()
            / self.config.period;

        match self.config.algorithm {
            TotpAlgorithm::Sha1 => totp_custom::<Sha1>(
                self.config.period,
                self.config.digits as u32,
                &self.decode_secret(secret),
                timestamp,
            ),
            TotpAlgorithm::Sha256 => totp_custom::<Sha256>(
                self.config.period,
                self.config.digits as u32,
                &self.decode_secret(secret),
                timestamp,
            ),
            TotpAlgorithm::Sha512 => totp_custom::<Sha512>(
                self.config.period,
                self.config.digits as u32,
                &self.decode_secret(secret),
                timestamp,
            ),
        }
    }

    /// Decode base32 secret
    fn decode_secret(&self, secret: &str) -> Vec<u8> {
        decode_base32(&secret.to_uppercase())
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
