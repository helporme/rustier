use super::errors::CastError;

macro_rules! integer_casts {
    ($($name:ident: $type:ty),* $(,)?) => {
        $(
            pub fn $name(value: f64) -> Result<$type, CastError> {
                if value.trunc() != value
                    || value < <$type>::MIN as f64
                    || value > <$type>::MAX as f64
                {
                    return Err(CastError {
                        value,
                        target: stringify!($type),
                    });
                }

                Ok(value as $type)
            }
        )*
    };
}

integer_casts!(
    f64_to_i8: i8,
    f64_to_i16: i16,
    f64_to_i32: i32,
    f64_to_u8: u8,
    f64_to_u16: u16,
    f64_to_u32: u32,
);

pub fn f64_to_char(value: f64) -> Result<char, CastError> {
    let value = f64_to_u32(value)?;

    char::from_u32(value).ok_or(CastError {
        value: value as f64,
        target: "char",
    })
}
