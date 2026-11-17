//! Custom Validators

use validator::ValidationError;

/// Validate that a string is a valid Git reference name
pub fn validate_git_ref(value: &str) -> Result<(), ValidationError> {
    if value.is_empty() {
        return Err(ValidationError::new("git_ref_empty"));
    }
    
    // Git refs must not contain certain characters
    let invalid_chars = [' ', '~', '^', ':', '?', '*', '[', '\\'];
    if value.chars().any(|c| invalid_chars.contains(&c)) {
        return Err(ValidationError::new("git_ref_invalid_chars"));
    }
    
    // Must not start or end with /
    if value.starts_with('/') || value.ends_with('/') {
        return Err(ValidationError::new("git_ref_slash"));
    }
    
    // Must not contain //
    if value.contains("//") {
        return Err(ValidationError::new("git_ref_double_slash"));
    }
    
    // Must not end with .lock
    if value.ends_with(".lock") {
        return Err(ValidationError::new("git_ref_lock"));
    }
    
    Ok(())
}

/// Validate project path (namespace/project)
pub fn validate_project_path(value: &str) -> Result<(), ValidationError> {
    let parts: Vec<&str> = value.split('/').collect();
    if parts.len() != 2 {
        return Err(ValidationError::new("project_path_format"));
    }
    
    for part in parts {
        if part.is_empty() {
            return Err(ValidationError::new("project_path_empty_part"));
        }
        // Each part must be valid Git ref
        validate_git_ref(part)?;
    }
    
    Ok(())
}

/// Validate slug (URL-friendly identifier)
pub fn validate_slug(value: &str) -> Result<(), ValidationError> {
    if value.is_empty() {
        return Err(ValidationError::new("slug_empty"));
    }
    
    if value.len() > 255 {
        return Err(ValidationError::new("slug_too_long"));
    }
    
    // Only lowercase letters, numbers, hyphens, underscores
    if !value.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_') {
        return Err(ValidationError::new("slug_invalid_chars"));
    }
    
    // Must not start or end with hyphen/underscore
    if value.starts_with('-') || value.starts_with('_') || value.ends_with('-') || value.ends_with('_') {
        return Err(ValidationError::new("slug_boundary"));
    }
    
    // Must not contain consecutive hyphens/underscores
    if value.contains("--") || value.contains("__") {
        return Err(ValidationError::new("slug_consecutive"));
    }
    
    Ok(())
}

/// Validate email format (additional to validator::ValidateEmail)
pub fn validate_email_format(value: &str) -> Result<(), ValidationError> {
    if value.is_empty() {
        return Err(ValidationError::new("email_empty"));
    }
    
    if value.len() > 254 {
        return Err(ValidationError::new("email_too_long"));
    }
    
    // Basic email validation
    let parts: Vec<&str> = value.split('@').collect();
    if parts.len() != 2 {
        return Err(ValidationError::new("email_format"));
    }
    
    let local = parts[0];
    let domain = parts[1];
    
    if local.is_empty() || local.len() > 64 {
        return Err(ValidationError::new("email_local_part"));
    }
    
    if domain.is_empty() || domain.len() > 255 {
        return Err(ValidationError::new("email_domain"));
    }
    
    // Domain must have at least one dot
    if !domain.contains('.') {
        return Err(ValidationError::new("email_domain_format"));
    }
    
    Ok(())
}