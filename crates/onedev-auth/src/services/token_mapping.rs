//! Map `access_tokens` rows onto the core access-token model.
//!
//! The table stores scopes as a JSON string array and has no token-type column.
//! The type is kept in that array as `type:<snake_case>`.

use onedev_core::models::access_token::{AccessToken, AccessTokenType, TokenScope};
use onedev_db::models::access_token::DbAccessToken;

const TYPE_PREFIX: &str = "type:";

pub fn scopes_to_db(token_type: AccessTokenType, scopes: TokenScope) -> Vec<String> {
    let mut names = vec![format!("{TYPE_PREFIX}{token_type}")];
    names.extend(scopes.iter_names().map(|(name, _)| name.to_string()));
    names
}

pub fn access_token_from_db(token: DbAccessToken) -> AccessToken {
    let (token_type, scopes) = decode_scopes(&token.scopes);
    AccessToken {
        id: token.id,
        uuid: token.uuid,
        name: token.name,
        token_hash: token.token_hash,
        token_prefix: String::new(),
        token_type,
        scopes,
        owner_id: token.user_id,
        impersonated_user_id: None,
        project_id: None,
        group_id: None,
        expires_at: token.expires_at,
        last_used_at: token.last_used_at,
        last_used_ip: None,
        revoked: token.revoked_at.is_some(),
        revoked_at: token.revoked_at,
        revoked_by_id: None,
        created_at: token.created_at,
        updated_at: token.updated_at,
    }
}

fn decode_scopes(scopes: &[String]) -> (AccessTokenType, TokenScope) {
    let mut token_type = AccessTokenType::Personal;
    let mut set = TokenScope::empty();
    for name in scopes {
        if let Some(rest) = name.strip_prefix(TYPE_PREFIX) {
            if let Ok(parsed) = rest.parse() {
                token_type = parsed;
            }
        } else if let Some(flag) = TokenScope::from_name(name) {
            set |= flag;
        }
    }
    (token_type, set)
}
