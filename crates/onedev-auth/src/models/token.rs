//! JWT Token handling

use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc, Duration};
use uuid::Uuid;
use secrecy::{SecretString, ExposeSecret};
use thiserror::Error;

/// JWT Claims
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Subject (principal string: u:<id> or a:<id>)
    pub sub: String,
    /// JWT ID
    pub jti: String,
    /// Issued at
    pub iat: i64,
    /// Expiration time
    pub exp: i64,
    /// Issuer
    pub iss: String,
    /// Audience
    pub aud: String,
    /// Token type
    pub typ: TokenType,
    /// Custom claims
    pub scopes: Vec<String>,
    /// User ID (for user tokens)
    pub user_id: Option<i64>,
    /// Token ID (for access tokens)
    pub token_id: Option<i64>,
}

/// Token type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TokenType {
    /// Access token (short-lived)
    Access,
    /// Refresh token (long-lived)
    Refresh,
    /// Temporary token (very short-lived)
    Temporary,
}

/// Token pair (access + refresh)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
    pub access_expires_at: DateTime<Utc>,
    pub refresh_expires_at: DateTime<Utc>,
    pub token_type: String,
}

/// Token configuration
#[derive(Debug, Clone)]
pub struct TokenConfig {
    /// Issuer
    pub issuer: String,
    /// Audience
    pub audience: String,
    /// Access token TTL (seconds)
    pub access_ttl_seconds: i64,
    /// Refresh token TTL (seconds)
    pub refresh_ttl_seconds: i64,
    /// Temporary token TTL (seconds)
    pub temporary_ttl_seconds: i64,
    /// RS256 private key (PEM format)
    pub private_key: SecretString,
    /// RS256 public key (PEM format)
    pub public_key: SecretString,
    /// Algorithm
    pub algorithm: Algorithm,
}

impl Default for TokenConfig {
    fn default() -> Self {
        Self {
            issuer: "onedev".to_string(),
            audience: "onedev-api".to_string(),
            access_ttl_seconds: 15 * 60, // 15 minutes
            refresh_ttl_seconds: 7 * 24 * 60 * 60, // 7 days
            temporary_ttl_seconds: 5 * 60, // 5 minutes
            private_key: SecretString::from(String::new()),
            public_key: SecretString::from(String::new()),
            algorithm: Algorithm::RS256,
        }
    }
}

/// JWT errors
#[derive(Debug, Error)]
pub enum JwtError {
    #[error("Token expired")]
    Expired,
    #[error("Invalid token: {0}")]
    Invalid(String),
    #[error("Token validation failed: {0}")]
    Validation(String),
    #[error("Encoding failed: {0}")]
    Encoding(String),
    #[error("Decoding failed: {0}")]
    Decoding(String),
    #[error("Key error: {0}")]
    Key(String),
    #[error("Missing configuration")]
    MissingConfig,
}

impl From<jsonwebtoken::errors::Error> for JwtError {
    fn from(err: jsonwebtoken::errors::Error) -> Self {
        match err.kind() {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => JwtError::Expired,
            jsonwebtoken::errors::ErrorKind::InvalidSignature => JwtError::Invalid("Invalid signature".to_string()),
            jsonwebtoken::errors::ErrorKind::InvalidToken => JwtError::Invalid("Invalid token format".to_string()),
            _ => JwtError::Validation(err.to_string()),
        }
    }
}

/// JWT Service for token creation and validation
pub struct JwtService {
    config: TokenConfig,
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    validation: Validation,
}

impl JwtService {
    /// Create a new JWT service
    pub fn new(config: TokenConfig) -> Result<Self, JwtError> {
        let encoding_key = EncodingKey::from_rsa_pem(config.private_key.expose_secret().as_bytes())
            .map_err(|e| JwtError::Key(format!("Invalid private key: {}", e)))?;

        let decoding_key = DecodingKey::from_rsa_pem(config.public_key.expose_secret().as_bytes())
            .map_err(|e| JwtError::Key(format!("Invalid public key: {}", e)))?;

        let mut validation = Validation::new(config.algorithm);
        validation.set_issuer(&[config.issuer.clone()]);
        validation.set_audience(&[config.audience.clone()]);
        validation.validate_exp = true;
        validation.validate_nbf = true;
        validation.leeway = 30; // 30 seconds clock skew tolerance

        Ok(Self {
            config,
            encoding_key,
            decoding_key,
            validation,
        })
    }

    /// Create an access token for a user
    pub fn create_access_token(&self, user_id: i64, username: &str, scopes: Vec<String>) -> Result<TokenPair, JwtError> {
        let now = Utc::now();
        let access_exp = now + Duration::seconds(self.config.access_ttl_seconds);
        let refresh_exp = now + Duration::seconds(self.config.refresh_ttl_seconds);
        let jti = Uuid::new_v4().to_string();

        // Access token claims
        let access_claims = Claims {
            sub: format!("u:{}", user_id),
            jti: jti.clone(),
            iat: now.timestamp(),
            exp: access_exp.timestamp(),
            iss: self.config.issuer.clone(),
            aud: self.config.audience.clone(),
            typ: TokenType::Access,
            scopes: scopes.clone(),
            user_id: Some(user_id),
            token_id: None,
        };

        // Refresh token claims
        let refresh_claims = Claims {
            sub: format!("u:{}", user_id),
            jti: format!("{}_refresh", jti),
            iat: now.timestamp(),
            exp: refresh_exp.timestamp(),
            iss: self.config.issuer.clone(),
            aud: self.config.audience.clone(),
            typ: TokenType::Refresh,
            scopes: vec!["refresh".to_string()],
            user_id: Some(user_id),
            token_id: None,
        };

        let access_token = encode(&Header::new(self.config.algorithm), &access_claims, &self.encoding_key)
            .map_err(|e| JwtError::Encoding(e.to_string()))?;

        let refresh_token = encode(&Header::new(self.config.algorithm), &refresh_claims, &self.encoding_key)
            .map_err(|e| JwtError::Encoding(e.to_string()))?;

        Ok(TokenPair {
            access_token,
            refresh_token,
            access_expires_at: access_exp,
            refresh_expires_at: refresh_exp,
            token_type: "Bearer".to_string(),
        })
    }

    /// Create an access token for an access token (API token)
    pub fn create_token_access_token(&self, token_id: i64, token_name: &str, owner_id: i64, scopes: Vec<String>) -> Result<TokenPair, JwtError> {
        let now = Utc::now();
        let access_exp = now + Duration::seconds(self.config.access_ttl_seconds);
        let refresh_exp = now + Duration::seconds(self.config.refresh_ttl_seconds);
        let jti = Uuid::new_v4().to_string();

        let access_claims = Claims {
            sub: format!("a:{}", token_id),
            jti: jti.clone(),
            iat: now.timestamp(),
            exp: access_exp.timestamp(),
            iss: self.config.issuer.clone(),
            aud: self.config.audience.clone(),
            typ: TokenType::Access,
            scopes: scopes.clone(),
            user_id: Some(owner_id),
            token_id: Some(token_id),
        };

        let refresh_claims = Claims {
            sub: format!("a:{}", token_id),
            jti: format!("{}_refresh", jti),
            iat: now.timestamp(),
            exp: refresh_exp.timestamp(),
            iss: self.config.issuer.clone(),
            aud: self.config.audience.clone(),
            typ: TokenType::Refresh,
            scopes: vec!["refresh".to_string()],
            user_id: Some(owner_id),
            token_id: Some(token_id),
        };

        let access_token = encode(&Header::new(self.config.algorithm), &access_claims, &self.encoding_key)
            .map_err(|e| JwtError::Encoding(e.to_string()))?;

        let refresh_token = encode(&Header::new(self.config.algorithm), &refresh_claims, &self.encoding_key)
            .map_err(|e| JwtError::Encoding(e.to_string()))?;

        Ok(TokenPair {
            access_token,
            refresh_token,
            access_expires_at: access_exp,
            refresh_expires_at: refresh_exp,
            token_type: "Bearer".to_string(),
        })
    }

    /// Create a temporary token (for email verification, password reset, etc.)
    pub fn create_temporary_token(&self, subject: &str, scopes: Vec<String>, ttl_seconds: Option<i64>) -> Result<String, JwtError> {
        let now = Utc::now();
        let exp = now + Duration::seconds(ttl_seconds.unwrap_or(self.config.temporary_ttl_seconds));
        let jti = Uuid::new_v4().to_string();

        let claims = Claims {
            sub: subject.to_string(),
            jti,
            iat: now.timestamp(),
            exp: exp.timestamp(),
            iss: self.config.issuer.clone(),
            aud: self.config.audience.clone(),
            typ: TokenType::Temporary,
            scopes,
            user_id: None,
            token_id: None,
        };

        encode(&Header::new(self.config.algorithm), &claims, &self.encoding_key)
            .map_err(|e| JwtError::Encoding(e.to_string()))
    }

    /// Validate and decode a token
    pub fn validate_token(&self, token: &str) -> Result<Claims, JwtError> {
        let token_data = decode::<Claims>(token, &self.decoding_key, &self.validation)
            .map_err(|e| JwtError::from(e))?;
        
        Ok(token_data.claims)
    }

    /// Validate an access token (not refresh)
    pub fn validate_access_token(&self, token: &str) -> Result<Claims, JwtError> {
        let claims = self.validate_token(token)?;
        
        if claims.typ != TokenType::Access {
            return Err(JwtError::Invalid("Not an access token".to_string()));
        }

        Ok(claims)
    }

    /// Validate a refresh token
    pub fn validate_refresh_token(&self, token: &str) -> Result<Claims, JwtError> {
        let claims = self.validate_token(token)?;
        
        if claims.typ != TokenType::Refresh {
            return Err(JwtError::Invalid("Not a refresh token".to_string()));
        }

        Ok(claims)
    }

    /// Extract subject from token without full validation (for logging/metrics)
    pub fn extract_subject(&self, token: &str) -> Option<String> {
        // Decode without verification for subject extraction
        let mut validation = Validation::new(self.config.algorithm);
        validation.validate_exp = false;
        validation.insecure_disable_signature_validation();
        
        decode::<Claims>(token, &DecodingKey::from_secret(b""), &validation)
            .ok()
            .map(|d| d.claims.sub)
    }

    /// Get token TTL for token type
    pub fn get_ttl(&self, token_type: TokenType) -> i64 {
        match token_type {
            TokenType::Access => self.config.access_ttl_seconds,
            TokenType::Refresh => self.config.refresh_ttl_seconds,
            TokenType::Temporary => self.config.temporary_ttl_seconds,
        }
    }
}

/// Generate RSA key pair for JWT signing
pub fn generate_rsa_keys() -> Result<(String, String), JwtError> {
    use rsa::{
        RsaPrivateKey, RsaPublicKey,
        pkcs8::{EncodePrivateKey, EncodePublicKey},
        rand_core::OsRng,
    };

    let mut rng = OsRng;
    let private_key = RsaPrivateKey::new(&mut rng, 2048)
        .map_err(|e| JwtError::Key(format!("Failed to generate private key: {}", e)))?;
    
    let public_key = RsaPublicKey::from(&private_key);

    let private_pem = private_key
        .to_pkcs8_pem(rsa::pkcs8::LineEnding::LF)
        .map_err(|e| JwtError::Key(format!("Failed to encode private key: {}", e)))?
        .to_string();

    let public_pem = public_key
        .to_public_key_pem(rsa::pkcs8::LineEnding::LF)
        .map_err(|e| JwtError::Key(format!("Failed to encode public key: {}", e)))?
        .to_string();

    Ok((private_pem, public_pem))
}