use thiserror::Error;

#[derive(Debug, Error)]
#[allow(dead_code)]
pub enum RevolverError {
    #[error("account '{0}' not found")]
    AccountNotFound(String),

    #[error("account '{0}' already exists")]
    AccountExists(String),

    #[error("invalid account name '{0}' — use alphanumeric, hyphens, underscores")]
    InvalidName(String),

    #[error("no active account")]
    NoActiveAccount,

    #[error("no credentials at {0} — run 'claude login' first")]
    NoCredentials(String),

    #[error("no accounts available for swap")]
    NoSwapTarget,

    #[error("usage API error: {0}")]
    UsageApi(String),

    #[error("token expired for account '{0}'")]
    TokenExpired(String),

    #[error("live token is the same as account '{0}' — login to a different account first")]
    DuplicateToken(String),
}
