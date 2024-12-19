use rdkafka::error::KafkaError;
use rdkafka::message::OwnedMessage;
use std::fmt;

#[derive(Debug)]
pub enum AppError {
    JsonError(serde_json::Error),
    KafkaError(rdkafka::error::KafkaError),
    HyperError(hyper::Error),
    KafkaErrorWithMessage((KafkaError, OwnedMessage)),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::JsonError(e) => write!(f, "JSON error: {}", e),
            AppError::KafkaError(e) => write!(f, "Kafka error: {}", e),
            AppError::HyperError(e) => write!(f, "Hyper error: {}", e),
            AppError::KafkaErrorWithMessage(e) => write!(f, "KafkaErrorWithMessage error: {:?}", e),
        }
    }
}

impl std::error::Error for AppError {}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        AppError::JsonError(err)
    }
}

impl From<rdkafka::error::KafkaError> for AppError {
    fn from(err: rdkafka::error::KafkaError) -> Self {
        AppError::KafkaError(err)
    }
}

impl From<hyper::Error> for AppError {
    fn from(err: hyper::Error) -> Self {
        AppError::HyperError(err)
    }
}

impl From<(KafkaError, OwnedMessage)> for AppError {
    fn from(err: (KafkaError, OwnedMessage)) -> Self {
        // Implement the conversion logic here
        AppError::KafkaErrorWithMessage(err)
    }
}
