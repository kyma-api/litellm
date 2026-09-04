use litellm_core::error::{Error, ErrorCode, ProviderState};
use pyo3::prelude::*;
use pyo3::types::PyNone;

pyo3::create_exception!(
    _native,
    RustPreparationError,
    pyo3::exceptions::PyException,
    "Rust stopped before provider execution and the host may use another implementation."
);

pyo3::create_exception!(
    _native,
    RustExecutionError,
    pyo3::exceptions::PyException,
    "Rust owns provider execution and the host must not retry through another implementation."
);

pub(crate) fn core_error_to_pyerr(error: Error) -> PyErr {
    match error {
        Error::Prepare {
            code,
            message,
            source: _,
        } => {
            structured_error::<RustPreparationError>(code, message, None, ProviderState::NotStarted)
        }
        Error::Execute {
            code,
            message,
            status_code,
            provider_state,
            source: _,
        } => structured_error::<RustExecutionError>(code, message, status_code, provider_state),
    }
}

fn structured_error<E>(
    code: ErrorCode,
    message: String,
    status_code: Option<u16>,
    provider_state: ProviderState,
) -> PyErr
where
    E: pyo3::type_object::PyTypeInfo,
{
    let error = Python::attach(|py| PyErr::from_type(py.get_type::<E>(), (message.clone(),)));
    Python::attach(|py| {
        let value = error.value(py);
        value
            .setattr("code", code.as_ref())
            .expect("exception attributes are writable");
        value
            .setattr("message", message)
            .expect("exception attributes are writable");
        match status_code {
            Some(status) => value
                .setattr("status_code", status)
                .expect("exception attributes are writable"),
            None => value
                .setattr("status_code", PyNone::get(py))
                .expect("exception attributes are writable"),
        }
        value
            .setattr("provider_state", provider_state.as_ref())
            .expect("exception attributes are writable");
    });
    error
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = module.py();
    let preparation = py.get_type::<RustPreparationError>();
    let execution = py.get_type::<RustExecutionError>();
    module.add("RustPreparationError", &preparation)?;
    module.add("RustExecutionError", &execution)?;
    module.add("RustBridgeDeclined", preparation)?;
    module.add("RustUpstreamError", execution)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preparation_error_exposes_structured_fields() {
        Python::initialize();
        Python::attach(|py| {
            let mapped = core_error_to_pyerr(Error::prepare(
                ErrorCode::InvalidRequest,
                "invalid document",
            ));
            assert!(mapped.is_instance_of::<RustPreparationError>(py));
            let value = mapped.value(py);
            assert_eq!(
                value
                    .getattr("code")
                    .and_then(|item| item.extract::<String>())
                    .expect("code is a string"),
                "invalid_request"
            );
            assert_eq!(
                value
                    .getattr("provider_state")
                    .and_then(|item| item.extract::<String>())
                    .expect("provider state is a string"),
                "not_started"
            );
            assert!(
                value
                    .getattr("status_code")
                    .expect("field exists")
                    .is_none()
            );
        });
    }

    #[test]
    fn execution_error_preserves_ownership_fields() {
        Python::initialize();
        Python::attach(|py| {
            let mapped = core_error_to_pyerr(Error::execute(
                ErrorCode::Upstream,
                "rate limited",
                Some(429),
                ProviderState::ResponseReceived,
            ));
            assert!(mapped.is_instance_of::<RustExecutionError>(py));
            let value = mapped.value(py);
            assert_eq!(
                value
                    .getattr("status_code")
                    .and_then(|item| item.extract::<u16>())
                    .expect("status is an integer"),
                429
            );
            assert_eq!(
                value
                    .getattr("provider_state")
                    .and_then(|item| item.extract::<String>())
                    .expect("provider state is a string"),
                "response_received"
            );
        });
    }
}
