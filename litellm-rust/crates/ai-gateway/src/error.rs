use litellm_core::{ErrorCode, ProviderState};
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum Error {
    #[error(transparent)]
    Core(#[from] litellm_core::Error),
    #[error(transparent)]
    Config(#[from] litellm_config::Error),
    #[error("gateway authentication failed")]
    Authentication,
    #[error("gateway request extraction failed")]
    Extraction,
    #[error("gateway transport failed")]
    Transport,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Projection {
    pub status_code: u16,
    pub message: String,
    pub provider_state: ProviderState,
}

impl Error {
    pub fn project(&self) -> Projection {
        match self {
            Self::Core(error) => project_core(error),
            Self::Config(_) => projection(500, "gateway configuration failed"),
            Self::Authentication => projection(401, "gateway authentication failed"),
            Self::Extraction => projection(400, "invalid gateway request"),
            Self::Transport => projection(502, "gateway transport failed"),
        }
    }
}

fn project_core(error: &litellm_core::Error) -> Projection {
    let status_code = match error.code() {
        ErrorCode::InvalidRequest | ErrorCode::Unsupported | ErrorCode::Policy => 400,
        ErrorCode::Routing => 404,
        ErrorCode::Authentication | ErrorCode::Transport | ErrorCode::InvalidResponse => 502,
        ErrorCode::Upstream => error
            .status_code()
            .filter(|status| (400..=599).contains(status))
            .unwrap_or(502),
        ErrorCode::Internal => 500,
    };
    Projection {
        status_code,
        message: error.message().to_string(),
        provider_state: error.provider_state(),
    }
}

fn projection(status_code: u16, message: &str) -> Projection {
    Projection {
        status_code,
        message: message.to_string(),
        provider_state: ProviderState::NotStarted,
    }
}

#[cfg(feature = "server")]
impl axum::response::IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        use axum::Json;
        use axum::http::StatusCode;
        let projection = self.project();
        let status = StatusCode::from_u16(projection.status_code)
            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        (
            status,
            Json(serde_json::json!({"error": {"message": projection.message}})),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use litellm_core::{ErrorCode, ProviderState};

    use super::*;

    #[test]
    fn core_projection_is_shared_by_http_and_websocket_hosts() {
        let cases = [
            (ErrorCode::InvalidRequest, 400),
            (ErrorCode::Unsupported, 400),
            (ErrorCode::Policy, 400),
            (ErrorCode::Routing, 404),
            (ErrorCode::Authentication, 502),
            (ErrorCode::Transport, 502),
            (ErrorCode::InvalidResponse, 502),
            (ErrorCode::Internal, 500),
        ];
        for (code, expected_status) in cases {
            let projection = Error::from(litellm_core::Error::execute(
                code,
                "safe message",
                None,
                ProviderState::MayHaveStarted,
            ))
            .project();
            assert_eq!(projection.status_code, expected_status);
            assert_eq!(projection.provider_state, ProviderState::MayHaveStarted);
        }
    }

    #[test]
    fn upstream_projection_preserves_sanitized_status() {
        let projection = Error::from(litellm_core::Error::execute(
            ErrorCode::Upstream,
            "rate limited",
            Some(429),
            ProviderState::ResponseReceived,
        ))
        .project();
        assert_eq!(projection.status_code, 429);
        assert_eq!(projection.message, "rate limited");
    }
}
