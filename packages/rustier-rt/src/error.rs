//! Rustier runtime errors.

use std::{
    cell::{BorrowError, BorrowMutError},
    error::Error,
    fmt,
};

#[derive(Debug)]
pub struct CastError {
    pub value: f64,
    pub target: &'static str,
}

impl fmt::Display for CastError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cannot cast {} to {}", self.value, self.target)
    }
}

impl Error for CastError {}

#[derive(Debug)]
pub enum EnumError {
    InactiveField {
        type_name: &'static str,
        variant: &'static str,
        field: &'static str,
    },
    UnknownVariant,
}

impl fmt::Display for EnumError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InactiveField {
                type_name,
                variant,
                field,
            } => write!(
                f,
                "cannot access inactive enum variant field `{type_name}::{variant}::{field}`"
            ),
            Self::UnknownVariant => {
                f.write_str("cannot discriminate an unknown non-exhaustive enum variant")
            }
        }
    }
}

impl Error for EnumError {}

#[derive(Debug)]
pub enum CallError {
    Borrow(BorrowError),
    BorrowMut(BorrowMutError),
    Cast(CastError),
    Enum(EnumError),
}

impl fmt::Display for CallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Borrow(_) => {
                f.write_str("cannot access a tethered value while it is mutably borrowed")
            }
            Self::BorrowMut(_) => {
                f.write_str("cannot mutably access a tethered value while it is borrowed")
            }
            Self::Cast(error) => error.fmt(f),
            Self::Enum(error) => error.fmt(f),
        }
    }
}

impl Error for CallError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Borrow(error) => Some(error),
            Self::BorrowMut(error) => Some(error),
            Self::Cast(error) => Some(error),
            Self::Enum(error) => Some(error),
        }
    }
}

impl From<BorrowError> for CallError {
    fn from(error: BorrowError) -> Self {
        Self::Borrow(error)
    }
}

impl From<BorrowMutError> for CallError {
    fn from(error: BorrowMutError) -> Self {
        Self::BorrowMut(error)
    }
}

impl From<CastError> for CallError {
    fn from(error: CastError) -> Self {
        Self::Cast(error)
    }
}

impl From<EnumError> for CallError {
    fn from(error: EnumError) -> Self {
        Self::Enum(error)
    }
}
