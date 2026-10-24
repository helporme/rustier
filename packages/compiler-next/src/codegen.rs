use crate::{
    db::Db,
    fragment::{FragmentOnce, FragmentPrint, frag, fragment},
    ir::{Part, RustFunction},
};

#[salsa::tracked]
impl<'db> RustFunction<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn render(self, db: &'db dyn Db) -> String {
        let name = self.name(db);
        let return_type = self.return_type(db).source(db);
        let params = fragment(|printer| {
            for (index, rust_type) in self
                .body(db)
                .iter()
                .filter_map(|part| match part {
                    Part::Text(_) => None,
                    Part::Interpolation { rust_type } => Some(rust_type.source(db)),
                })
                .enumerate()
            {
                if index != 0 {
                    ", ".apply(printer);
                }

                frag!(__rustier_arg_{index}: ~{rust_type}).apply(printer);
            }
        });
        let body = fragment(|printer| {
            let mut index = 0;
            for part in self.body(db) {
                match part {
                    Part::Text(text) => text.apply(printer),
                    Part::Interpolation { .. } => {
                        frag!(__rustier_arg_ { index }).apply(printer);
                        index += 1;
                    }
                }
            }
        });

        let mut output = frag!(
            fn ~{name}{"("}{params}{")"} ~-> ~{return_type} ~{"{\n"}{body}{"\n}"}
        )
        .print();
        output.push('\n');
        output
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        db::RootDatabase,
        ir::{Part, RustFunction, RustType},
    };

    #[test]
    fn renders_a_body_without_interpolations() {
        let db = RootDatabase::default();
        let function = RustFunction::new(
            &db,
            "answer".to_owned(),
            RustType::new(&db, "u32".to_owned()),
            vec![Part::Text("40 + 2".to_owned())],
        );

        assert_eq!(function.render(&db), "fn answer() -> u32 {\n40 + 2\n}\n");
    }

    #[test]
    fn renders_typed_arguments_in_interpolation_order() {
        let db = RootDatabase::default();
        let function = RustFunction::new(
            &db,
            "calculate".to_owned(),
            RustType::new(&db, "u32".to_owned()),
            vec![
                Part::Text("let n: u32 = ".to_owned()),
                Part::Interpolation {
                    rust_type: RustType::new(&db, "u32".to_owned()),
                },
                Part::Text(";\nn + ".to_owned()),
                Part::Interpolation {
                    rust_type: RustType::new(&db, "u32".to_owned()),
                },
            ],
        );

        assert_eq!(
            function.render(&db),
            "fn calculate(__rustier_arg_0: u32, __rustier_arg_1: u32) -> u32 {\nlet n: u32 = __rustier_arg_0;\nn + __rustier_arg_1\n}\n"
        );
    }

    #[test]
    fn preserves_rust_text_verbatim() {
        let db = RootDatabase::default();
        let body = "/* comment */\nlet text = r#\"{ hello }\\n\"#;\ntext";
        let function = RustFunction::new(
            &db,
            "text".to_owned(),
            RustType::new(&db, "&'static str".to_owned()),
            vec![Part::Text(body.to_owned())],
        );

        assert_eq!(
            function.render(&db),
            &format!("fn text() -> &'static str {{\n{body}\n}}\n")
        );
    }
}
