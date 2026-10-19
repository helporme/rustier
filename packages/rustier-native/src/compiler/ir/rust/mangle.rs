use crate::compiler::{
    db::Db,
    ir::rust::{GenArg, ImplTrait, PathSegment, Type, TypeKind},
};

/// An argument whose stable semantic identity is not available to the mangler.
///
/// Display text, parameter names, and database-local intern IDs are
/// deliberately not used as substitutes for declaration identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, salsa::Update, thiserror::Error)]
pub enum MangleError {
    #[error("cannot mangle an unresolved type parameter")]
    TypeParameter,
}

fn length_prefixed(value: &str) -> String {
    format!("{}_{value}", value.len())
}

/// Keeps qualified names readable, using byte lengths to delimit identifiers.
///
/// Path segments are separated by `__`. Nonempty generic arguments follow their
/// segment inside `G...E`.
/// Names are consumed by length, so their contents cannot act as
/// separators or generic markers.
pub(super) fn mangle_segments<'db>(
    db: &'db dyn Db,
    segments: &[PathSegment<'db>],
) -> Result<String, MangleError> {
    let mut path = String::new();
    for (index, segment) in segments.iter().enumerate() {
        if index > 0 {
            path.push_str("__");
        }
        // `r#` is Rust spelling, not part of the identifier. Preserve the actual
        // name, including underscores and Unicode identifier characters.
        let name = segment.name(db);
        let name = name.strip_prefix("r#").unwrap_or(name);
        path.push_str(&length_prefixed(name));
        if !segment.gen_args(db).is_empty() {
            path.push('G');
            for arg in segment.gen_args(db) {
                let GenArg::Type(type_) = arg;
                path.push_str(&encode_type(db, *type_)?);
            }
            path.push('E');
        }
    }
    Ok(path)
}

pub(super) fn mangle_qualified<'db>(
    db: &'db dyn Db,
    impl_: ImplTrait<'db>,
    segments: &[PathSegment<'db>],
) -> Result<String, MangleError> {
    let target = encode_type(db, impl_.target(db))?;
    let trait_ = mangle_segments(db, impl_.trait_path(db).segments(db))?;
    let method = mangle_segments(db, segments)?;

    Ok(format!(
        "Q{}__{}__{method}",
        length_prefixed(&target),
        length_prefixed(&trait_),
    ))
}

fn encode_type<'db>(db: &'db dyn Db, type_: Type<'db>) -> Result<String, MangleError> {
    // These literal tags describe constructors, never source/display spelling.
    let primitive = match type_.kind(db) {
        TypeKind::Unit => "unit",
        TypeKind::bool => "bool",
        TypeKind::char => "char",
        TypeKind::i8 => "i8",
        TypeKind::u8 => "u8",
        TypeKind::i16 => "i16",
        TypeKind::u16 => "u16",
        TypeKind::i32 => "i32",
        TypeKind::u32 => "u32",
        TypeKind::f32 => "f32",
        TypeKind::f64 => "f64",
        TypeKind::Str => "str",
        TypeKind::String => "String",
        TypeKind::JsValue => "JsValue",
        TypeKind::Generic(_) => return Err(MangleError::TypeParameter),
        TypeKind::Struct(type_) => {
            return Ok(format!(
                "S{}",
                mangle_segments(db, type_.path(db).segments(db))?
            ));
        }
        TypeKind::Enum(type_) => {
            // `E` closes generic arguments, so enums use a separate `V` tag.
            return Ok(format!(
                "V{}",
                mangle_segments(db, type_.path(db).segments(db))?
            ));
        }
        TypeKind::Borrowed(reference) => {
            return Ok(format!(
                "R{}{}",
                if reference.is_mutable(db) { "M" } else { "S" },
                encode_type(db, reference.inner(db))?,
            ));
        }
    };
    Ok(format!("P{}", length_prefixed(primitive)))
}

#[cfg(test)]
mod tests {
    use super::{PathSegment, mangle_segments};
    use crate::compiler::db::RootDatabase;

    fn mangle<'a>(path: impl IntoIterator<Item = &'a str>) -> String {
        let db = RootDatabase::default();
        let segments = path
            .into_iter()
            .map(|name| PathSegment::plain(&db, name.to_owned()))
            .collect::<Vec<_>>();
        mangle_segments(&db, &segments).unwrap()
    }

    #[test]
    fn mangles_empty_path_to_empty_string() {
        assert_eq!(mangle([]), "");
    }

    #[test]
    fn length_prefixes_path_segments() {
        assert_eq!(
            mangle(["test_crate", "Test", "new"]),
            "10_test_crate__4_Test__3_new"
        );
    }

    #[test]
    fn encodes_zeroes_inside_segments() {
        assert_eq!(mangle(["item0"]), "5_item0");
    }

    #[test]
    fn encodes_underscores_inside_segments() {
        assert_eq!(mangle(["my_mod"]), "6_my_mod");
    }

    #[test]
    fn encoded_segment_chars_do_not_collide_with_separators() {
        assert_ne!(mangle(["a__b", "c"]), mangle(["a", "b__c"]));
        assert_eq!(mangle(["a__b", "c"]), "4_a__b__1_c");
        assert_eq!(mangle(["a", "b__c"]), "1_a__4_b__c");
    }
}
