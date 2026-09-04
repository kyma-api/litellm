use crate::Error;
mod client;
mod handler;
mod prepare;
pub mod transformation;
pub mod types;

use serde_json::Value;

pub use handler::execute_audio_transcription_provider_call;
pub use prepare::prepare_audio_transcription_provider_call;
pub use types::{AudioTranscriptionRequest, ProviderAudioTranscriptionRequest};

pub type PreparedAudioTranscription = ProviderAudioTranscriptionRequest;

pub fn prepare(
    request: AudioTranscriptionRequest<'_>,
) -> Result<PreparedAudioTranscription, Error> {
    prepare_audio_transcription_provider_call(request)
}

pub async fn execute(prepared: PreparedAudioTranscription) -> Result<Value, Error> {
    execute_audio_transcription_provider_call(prepared)
        .await
        .map_err(Error::after_ownership_transfer)
}

#[tracing::instrument(target = "litellm::function_trace", level = "trace", skip_all)]
pub async fn audio_transcription(request: AudioTranscriptionRequest<'_>) -> Result<Value, Error> {
    execute(prepare(request)?).await
}

#[cfg(test)]
mod tests;
