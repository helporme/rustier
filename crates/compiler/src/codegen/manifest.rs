use std::collections::BTreeMap;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum Dependency {
    Path { path: String },
    Registry { version: String },
}

pub fn render_manifest(crates: &BTreeMap<String, Dependency>) -> String {
    let aux = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../runtime"); // FIXME: env
    let mut manifest = toml::Table::from_iter([
        (
            "package".to_owned(),
            toml::Value::Table(toml::Table::from_iter([
                ("name".to_owned(), "rustier-macros".into()),
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
                "rustier".to_owned(),
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
    for (name, dependency) in crates {
        let (key, value) = match dependency {
            Dependency::Path { path } => ("path", path),
            Dependency::Registry { version } => ("version", version),
        };
        dependencies.insert(
            name.clone(),
            toml::Value::Table(toml::Table::from_iter([(key.into(), value.clone().into())])),
        );
    }
    manifest.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest() {
        let mut manifest: toml::Table = render_manifest(&BTreeMap::from([(
            "example".into(),
            Dependency::Path {
                path: "<example-crate>".into(),
            },
        )]))
        .parse()
        .unwrap();
        manifest["dependencies"]["rustier"]["path"] = "<rustier>".into();
        insta::with_settings!({
            snapshot_path => "_snapshots/manifest",
            prepend_module_to_snapshot => false,
        }, {
            insta::assert_snapshot!("manifest", format!("```toml\n{manifest}```\n"));
        });
    }
}
