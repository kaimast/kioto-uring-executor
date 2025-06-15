#[derive(Clone, PartialEq, Eq, Debug, thiserror::Error)]
pub enum Error {
    #[error("No kioto runtime is active")]
    NoRuntime,
    #[error("kioto runtime is shutting down")]
    ShuttingDown,
    #[error("kioto task failed {message}")]
    TaskFailed { message: String },
}

pub type Result<O> = std::result::Result<O, Error>;
