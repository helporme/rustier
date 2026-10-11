mod error;

pub use error::{CallError, CastError, EnumError};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub type CallResult<T> = Result<T, CallError>;
