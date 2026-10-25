use crate::{db::Db, ir::js::JsProgram};

impl JsProgram {
    pub fn render_cargo_toml(
        &self,
        _db: &dyn Db,
        crates: &std::collections::BTreeMap<String, String>,
    ) -> String {
        let aux = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../aux-next");
        let mut manifest = toml::Table::from_iter([
            (
                "package".to_owned(),
                toml::Value::Table(toml::Table::from_iter([
                    ("name".to_owned(), "rustier-snippets".into()),
                    ("version".to_owned(), "0.0.0".into()),
                    ("edition".to_owned(), "2024".into()),
                ])),
            ),
            (
                "lib".to_owned(),
                toml::Value::Table(toml::Table::from_iter([
                    ("path".to_owned(), "lib.rs".into()),
                    (
                        "crate-type".to_owned(),
                        toml::Value::Array(vec!["cdylib".into()]),
                    ),
                ])),
            ),
            (
                "dependencies".to_owned(),
                toml::Value::Table(toml::Table::from_iter([(
                    "aux-next".to_owned(),
                    toml::Value::Table(toml::Table::from_iter([(
                        "path".to_owned(),
                        toml::Value::String(aux.to_string_lossy().into_owned()),
                    )])),
                )])),
            ),
            (
                "workspace".to_owned(),
                toml::Value::Table(toml::Table::new()),
            ),
        ]);

        let dependencies = manifest["dependencies"].as_table_mut().unwrap();
        for (name, path) in crates {
            dependencies.insert(
                name.clone(),
                toml::Value::Table(toml::Table::from_iter([(
                    "path".into(),
                    path.clone().into(),
                )])),
            );
        }
        manifest.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::RootDatabase;

    #[test]
    fn manifest() {
        let db = RootDatabase::default();
        let program = JsProgram::new(&db, vec![]);
        let mut manifest: toml::Table = program
            .render_cargo_toml(
                &db,
                &std::collections::BTreeMap::from([("example".into(), "<example-crate>".into())]),
            )
            .parse()
            .unwrap();
        manifest["dependencies"]["aux-next"]["path"] = "<aux-next>".into();
        insta::with_settings!({
            snapshot_path => "_snapshots/manifest",
            prepend_module_to_snapshot => false,
        }, {
            insta::assert_snapshot!("manifest", format!("```toml\n{manifest}```\n"));
        });
    }
}
