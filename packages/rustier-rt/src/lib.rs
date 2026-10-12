mod error;
mod handle;
pub mod tethering;

pub use error::{CallError, CastError, EnumError};
pub use handle::Handle;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub type CallResult<T> = Result<T, CallError>;
