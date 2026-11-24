use std::collections::BTreeMap;

const BUILTIN_CRATES: [(&str, &str); 3] = [
    ("rustier", "rustier"),
    ("rustier_runtime", "runtime"),
    ("rustier_js_proc", "js-proc"),
];

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
    let builtin = BUILTIN_CRATES
        .into_iter()
        .map(|(name, _directory)| {
            #[cfg(feature = "local-runtime")]
            let (key, value) = {
                let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("..")
                    .join(_directory);
                (
                    "path",
                    toml::Value::String(path.to_string_lossy().into_owned()),
                )
            };
            #[cfg(not(feature = "local-runtime"))]
            let (key, value) = (
                "version",
                toml::Value::from(concat!("=", env!("CARGO_PKG_VERSION"))),
            );

            (
                name.into(),
                toml::Value::Table(toml::Table::from_iter([(key.into(), value)])),
            )
        })
        .collect();

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
        ("dependencies".to_owned(), toml::Value::Table(builtin)),
        (
            "workspace".to_owned(),
            toml::Value::Table(toml::Table::new()),
        ),
    ]);

    #[cfg(feature = "local-runtime")]
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
        let manifest: toml::Table = render_manifest(&BTreeMap::from([(
            "example".into(),
            Dependency::Path {
                path: "<example-crate>".into(),
                features: vec![],
            },
        )]))
        .parse()
        .unwrap();

        #[cfg(not(feature = "local-runtime"))]
        let manifest = {
            let mut manifest = manifest;
            for (name, _) in BUILTIN_CRATES {
                let version = &mut manifest["dependencies"][name]["version"];
                assert_eq!(
                    version.as_str(),
                    Some(concat!("=", env!("CARGO_PKG_VERSION")))
                );
                *version = "=<compiler-version>".into();
            }
            manifest
        };

        #[cfg(feature = "local-runtime")]
        let manifest = {
            let mut manifest = manifest;
            for (name, directory) in BUILTIN_CRATES {
                let path = manifest["dependencies"][name]["path"].as_str().unwrap();
                assert_eq!(
                    manifest["patch"]["crates-io"][name]["path"].as_str(),
                    Some(path)
                );
                let path = std::path::Path::new(path);
                assert!(path.is_absolute() && path.join("Cargo.toml").is_file());
                assert_eq!(path.file_name().unwrap(), directory);
                let label = format!("<{directory}>");
                manifest["dependencies"][name]["path"] = label.clone().into();
                manifest["patch"]["crates-io"][name]["path"] = label.into();
            }
            manifest
        };

        insta::with_settings!({
            snapshot_path => "_snapshots/manifest",
            prepend_module_to_snapshot => false,
        }, {
            let name = if cfg!(feature = "local-runtime") { "manifest" } else { "registry_manifest" };
            insta::assert_snapshot!(name, format!("```toml\n{manifest}```\n"));
        });
    }
}
