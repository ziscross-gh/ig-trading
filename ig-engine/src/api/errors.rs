use reqwest::StatusCode;
use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum IGError {
    #[error("Rate limit exceeded: {0}")]
    RateLimitExceeded(String),

    #[error("Authentication failed or token invalid: {0}")]
    InvalidToken(String),

    #[error("Market is closed or offline: {0}")]
    MarketClosed(String),

    #[error("Insufficient funds or margin for trade: {0}")]
    InsufficientFunds(String),

    #[error("API request failed with status {status}: {details}")]
    ApiError {
        status: StatusCode,
        code: String,
        details: String,
    },

    #[error("Network or connection error: {0}")]
    ConnectionError(String),

    #[error("JSON decoding error: {0}")]
    DecodeError(String),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

impl IGError {
    pub fn from_ig_code(status: StatusCode, code: &str, details: &str) -> Self {
        match code {
            "error.public-api.exceeded-account-trading-limit"
            | "error.public-api.exceeded-account-allowance"
            | "error.public-api.exceeded-api-key-allowance" => {
                IGError::RateLimitExceeded(code.to_string())
            }
            "error.security.client-token-invalid" | "error.security.cst-token-invalid" => {
                IGError::InvalidToken(code.to_string())
            }
            "error.market.closed" | "error.market.offline" => {
                IGError::MarketClosed(code.to_string())
            }
            "error.trade.not-enough-funds" => IGError::InsufficientFunds(code.to_string()),
            _ => {
                if status == StatusCode::UNAUTHORIZED {
                    IGError::InvalidToken("Unauthorized".to_string())
                } else if status == StatusCode::FORBIDDEN {
                    IGError::RateLimitExceeded("Forbidden / Burst limit hit".to_string())
                } else {
                    IGError::ApiError {
                        status,
                        code: code.to_string(),
                        details: details.to_string(),
                    }
                }
            }
        }
    }

    /// Returns true if the error is likely transient and should be retried
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            IGError::RateLimitExceeded(_) | IGError::ConnectionError(_)
        )
    }

    /// Returns true if the error requires a full session re-authentication
    pub fn requires_reauth(&self) -> bool {
        matches!(self, IGError::InvalidToken(_))
    }
}
