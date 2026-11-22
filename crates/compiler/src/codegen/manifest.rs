use std::collections::BTreeMap;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum Dependency {
    Path {
        path: String,
        #[serde(default)]
        features: Vec<String>,
    },
    Registry {
        version: String,
        #[serde(default)]
        features: Vec<String>,
    },
}

pub fn render_manifest(crates: &BTreeMap<String, Dependency>) -> String {
    let crates_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let builtin = [
        ("rustier", "rustier"),
        ("rustier_runtime", "runtime"),
        ("rustier_js_proc", "js-proc"),
    ];
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
            toml::Value::Table(
                builtin
                    .into_iter()
                    .map(|(name, directory)| {
                        let path = crates_dir.join(directory);
                        (
                            name.into(),
                            toml::Value::Table(toml::Table::from_iter([(
                                "path".into(),
                                toml::Value::String(path.to_string_lossy().into_owned()),
                            )])),
                        )
                    })
                    .collect(),
            ),
        ),
        (
            "workspace".to_owned(),
            toml::Value::Table(toml::Table::new()),
        ),
    ]);

    manifest.insert(
        "patch".into(),
        toml::Value::Table(toml::Table::from_iter([(
            "crates-io".into(),
            manifest["dependencies"].clone(),
        )])),
    );

    let dependencies = manifest["dependencies"].as_table_mut().unwrap();
    for (name, dependency) in crates {
        let (key, value, features) = match dependency {
            Dependency::Path { path, features } => ("path", path, features),
            Dependency::Registry { version, features } => ("version", version, features),
        };
        let mut entry = toml::Table::from_iter([(key.into(), value.clone().into())]);
        if !features.is_empty() {
            entry.insert(
                "features".into(),
                toml::Value::Array(features.iter().cloned().map(toml::Value::String).collect()),
            );
        }
        dependencies.insert(name.clone(), toml::Value::Table(entry));
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
                features: vec![],
            },
        )]))
        .parse()
        .unwrap();
        manifest["dependencies"]["rustier"]["path"] = "<rustier>".into();
        manifest["dependencies"]["rustier_runtime"]["path"] = "<runtime>".into();
        manifest["dependencies"]["rustier_js_proc"]["path"] = "<js-proc>".into();
        for name in ["rustier", "rustier_runtime", "rustier_js_proc"] {
            manifest["patch"]["crates-io"][name]["path"] =
                manifest["dependencies"][name]["path"].clone();
        }
        insta::with_settings!({
            snapshot_path => "_snapshots/manifest",
            prepend_module_to_snapshot => false,
        }, {
            insta::assert_snapshot!("manifest", format!("```toml\n{manifest}```\n"));
        });
    }
}
