use std::error::Error as StdError;

use strum::AsRefStr;
use thiserror::Error as ThisError;

use crate::constants::ERROR_MESSAGE_MAX_CHARS;

pub type BoxError = Box<dyn StdError + Send + Sync + 'static>;

#[derive(AsRefStr, Clone, Copy, Debug, Eq, PartialEq)]
#[strum(serialize_all = "snake_case")]
pub enum ErrorCode {
    Unsupported,
    InvalidRequest,
    Authentication,
    Routing,
    Policy,
    Transport,
    Upstream,
    InvalidResponse,
    Internal,
}

#[derive(AsRefStr, Clone, Copy, Debug, Eq, PartialEq)]
#[strum(serialize_all = "snake_case")]
pub enum ProviderState {
    NotStarted,
    MayHaveStarted,
    ResponseReceived,
}

#[derive(Debug, ThisError)]
pub enum Error {
    #[error("{message}")]
    Prepare {
        code: ErrorCode,
        message: String,
        #[source]
        source: Option<BoxError>,
    },
    #[error("{message}")]
    Execute {
        code: ErrorCode,
        message: String,
        status_code: Option<u16>,
        provider_state: ProviderState,
        #[source]
        source: Option<BoxError>,
    },
}

#[cfg(test)]
impl PartialEq for Error {
    fn eq(&self, other: &Self) -> bool {
        self.is_prepare() == other.is_prepare()
            && self.code() == other.code()
            && self.message() == other.message()
            && self.status_code() == other.status_code()
            && self.provider_state() == other.provider_state()
    }
}

#[cfg(test)]
impl Eq for Error {}

impl Error {
    pub fn prepare(code: ErrorCode, message: impl Into<String>) -> Self {
        Self::Prepare {
            code,
            message: bounded_message(message),
            source: None,
        }
    }

    pub fn prepare_with_source<E>(code: ErrorCode, message: impl Into<String>, source: E) -> Self
    where
        E: StdError + Send + Sync + 'static,
    {
        Self::Prepare {
            code,
            message: bounded_message(message),
            source: Some(Box::new(source)),
        }
    }

    pub fn execute(
        code: ErrorCode,
        message: impl Into<String>,
        status_code: Option<u16>,
        provider_state: ProviderState,
    ) -> Self {
        Self::Execute {
            code,
            message: bounded_message(message),
            status_code,
            provider_state,
            source: None,
        }
    }

    pub fn execute_with_source<E>(
        code: ErrorCode,
        message: impl Into<String>,
        status_code: Option<u16>,
        provider_state: ProviderState,
        source: E,
    ) -> Self
    where
        E: StdError + Send + Sync + 'static,
    {
        Self::Execute {
            code,
            message: bounded_message(message),
            status_code,
            provider_state,
            source: Some(Box::new(source)),
        }
    }

    pub const fn code(&self) -> ErrorCode {
        match self {
            Self::Prepare { code, .. } | Self::Execute { code, .. } => *code,
        }
    }

    pub fn message(&self) -> &str {
        match self {
            Self::Prepare { message, .. } | Self::Execute { message, .. } => message,
        }
    }

    pub const fn status_code(&self) -> Option<u16> {
        match self {
            Self::Prepare { .. } => None,
            Self::Execute { status_code, .. } => *status_code,
        }
    }

    pub const fn provider_state(&self) -> ProviderState {
        match self {
            Self::Prepare { .. } => ProviderState::NotStarted,
            Self::Execute { provider_state, .. } => *provider_state,
        }
    }

    pub const fn is_prepare(&self) -> bool {
        matches!(self, Self::Prepare { .. })
    }

    pub fn after_ownership_transfer(self) -> Self {
        match self {
            Self::Prepare {
                code,
                message,
                source,
            } => Self::Execute {
                code,
                message,
                status_code: None,
                provider_state: ProviderState::NotStarted,
                source,
            },
            execution @ Self::Execute { .. } => execution,
        }
    }
    pub fn invalid_request(message: String) -> Self {
        Self::prepare(
            ErrorCode::InvalidRequest,
            format!("invalid request: {message}"),
        )
    }
    pub fn invalid_provider(provider: String) -> Self {
        Self::prepare(
            ErrorCode::Unsupported,
            format!("invalid provider: {provider}"),
        )
    }
    pub fn missing_field(field: &'static str) -> Self {
        Self::prepare(
            ErrorCode::InvalidRequest,
            format!("missing required field: {field}"),
        )
    }
    pub fn authentication(message: String) -> Self {
        Self::prepare(ErrorCode::Authentication, message)
    }
    pub fn routing(message: String) -> Self {
        Self::prepare(ErrorCode::Routing, format!("routing error: {message}"))
    }
    pub fn unsupported(message: &'static str) -> Self {
        Self::prepare(
            ErrorCode::Unsupported,
            format!("unsupported by the rust path: {message}"),
        )
    }
    pub fn connection_failed(_message: String) -> Self {
        Self::execute(
            ErrorCode::Transport,
            "could not reach the provider",
            None,
            ProviderState::NotStarted,
        )
    }
    pub fn transport(_message: String) -> Self {
        Self::execute(
            ErrorCode::Transport,
            "upstream network error",
            None,
            ProviderState::MayHaveStarted,
        )
    }
    pub fn invalid_response(message: String) -> Self {
        Self::execute(
            ErrorCode::InvalidResponse,
            format!("invalid response: {message}"),
            None,
            ProviderState::ResponseReceived,
        )
    }

    pub fn invalid_type(expected: &'static str, actual: &'static str) -> Self {
        Self::prepare(
            ErrorCode::InvalidRequest,
            format!("expected {expected}, got {actual}"),
        )
    }

    pub fn invalid_response_type(expected: &'static str, actual: &'static str) -> Self {
        Self::execute(
            ErrorCode::InvalidResponse,
            format!("expected {expected}, got {actual}"),
            None,
            ProviderState::ResponseReceived,
        )
    }

    pub fn missing_response_field(field: &'static str) -> Self {
        Self::execute(
            ErrorCode::InvalidResponse,
            format!("missing required field: {field}"),
            None,
            ProviderState::ResponseReceived,
        )
    }

    pub fn unsupported_response(message: &'static str) -> Self {
        Self::execute(
            ErrorCode::InvalidResponse,
            message,
            None,
            ProviderState::ResponseReceived,
        )
    }

    pub fn upstream(status: u16, _body: String) -> Self {
        Self::execute(
            ErrorCode::Upstream,
            format!("upstream request failed with status {status}"),
            Some(status),
            ProviderState::ResponseReceived,
        )
    }
}

fn bounded_message(message: impl Into<String>) -> String {
    let message = message.into();
    if message.chars().count() <= ERROR_MESSAGE_MAX_CHARS {
        return message;
    }
    let prefix: String = message.chars().take(ERROR_MESSAGE_MAX_CHARS).collect();
    format!("{prefix}... (truncated)")
}

pub fn json_type_name(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "bool",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phases_expose_ownership_metadata() {
        let prepare = Error::prepare(ErrorCode::InvalidRequest, "bad input");
        assert!(prepare.is_prepare());
        assert_eq!(prepare.code(), ErrorCode::InvalidRequest);
        assert_eq!(prepare.provider_state(), ProviderState::NotStarted);

        let execute = Error::execute(
            ErrorCode::Transport,
            "timed out",
            None,
            ProviderState::MayHaveStarted,
        );
        assert!(!execute.is_prepare());
        assert_eq!(execute.provider_state(), ProviderState::MayHaveStarted);
    }

    #[test]
    fn constructors_preserve_source_chain() {
        let error = Error::prepare_with_source(
            ErrorCode::InvalidRequest,
            "invalid JSON",
            serde_json::from_str::<serde_json::Value>("{").expect_err("must be invalid"),
        );
        assert!(error.source().is_some());
        assert_eq!(error.message(), "invalid JSON");
    }

    #[test]
    fn boundary_messages_are_bounded() {
        let error = Error::prepare(
            ErrorCode::InvalidRequest,
            "x".repeat(ERROR_MESSAGE_MAX_CHARS + 1),
        );
        assert!(error.message().ends_with("... (truncated)"));
        assert_eq!(
            error.message().chars().count(),
            ERROR_MESSAGE_MAX_CHARS + "... (truncated)".chars().count()
        );
    }
}
