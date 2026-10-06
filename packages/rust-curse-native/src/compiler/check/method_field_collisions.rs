#[cfg(test)]
mod tests {
    use crate::{compiler::check::tests::diagnostic_codes, diagnostic_test_db as test_db};

    #[test]
    fn warns_when_struct_field_shadows_instance_method() {
        let db = test_db! {
            #[derive(Clone)]
            pub struct Example {
                pub value: i32,
            }

            impl Example {
                pub fn value(&self) -> i32 {
                    self.value
                }
            }
        };

        assert_eq!(
            diagnostic_codes(&db),
            ["rust_curse::method_shadowed_by_field"]
        );
    }

    #[test]
    fn warns_once_when_enum_fields_shadow_a_method() {
        let db = test_db! {
            #[derive(Clone)]
            pub enum Example {
                First { value: i32 },
                Second { value: i32 },
            }

            impl Example {
                pub fn value(&self) -> i32 {
                    0
                }
            }
        };

        assert_eq!(
            diagnostic_codes(&db),
            ["rust_curse::method_shadowed_by_field"]
        );
    }

    #[test]
    fn static_method_does_not_shadow_instance_field() {
        let db = test_db! {
            #[derive(Clone)]
            pub struct Example {
                pub value: i32,
            }

            impl Example {
                pub fn value() -> i32 {
                    0
                }
            }
        };

        assert!(diagnostic_codes(&db).is_empty());
    }
}
