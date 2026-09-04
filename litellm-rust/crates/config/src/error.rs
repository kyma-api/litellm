use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum Error {
    #[cfg(feature = "python")]
    #[error("read_model_list failed")]
    PythonLoading(#[source] pyo3::PyErr),
    #[cfg(feature = "python")]
    #[error("serializing model_list failed")]
    Serialization(#[source] pyo3::PyErr),
    #[error("parsing model_list failed")]
    ModelListParsing(#[source] serde_json::Error),
}
