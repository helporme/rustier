use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use compiler::{
    db::RootDatabase,
    ir::{JsFile, Macro, Part, SourceRange, Transport},
};
use napi::{Error, Result, bindgen_prelude::Buffer};
use napi_derive::napi;
use salsa::Setter;

#[napi(object)]
pub struct ServerOptions {
    pub root_dir: String,
    pub release: Option<bool>,
    pub crates: Option<serde_json::Value>,
}

#[napi]
pub struct Server {
    db: RootDatabase,
    files: BTreeMap<String, JsFile>,
    directory: PathBuf,
    release: bool,
    dependencies: toml::Table,
}

#[napi]
impl Server {
    #[napi(constructor)]
    pub fn new(options: ServerOptions) -> Result<Self> {
        let root = fs::canonicalize(&options.root_dir)
            .map_err(|error| Error::from_reason(format!("invalid project root: {error}")))?;
        let mut dependencies = match options.crates {
            Some(value) => toml::Value::try_from(value)
                .map_err(|error| {
                    Error::from_reason(format!("invalid crates configuration: {error}"))
                })?
                .as_table()
                .cloned()
                .ok_or_else(|| Error::from_reason("crates must be an object"))?,
            None => toml::Table::new(),
        };
        if dependencies.contains_key("runtime-next") {
            return Err(Error::from_reason("runtime-next is reserved by Rustier"));
        }
        for (_, dependency) in dependencies.iter_mut() {
            if let Some(table) = dependency.as_table_mut() {
                if let Some(path) = table.get_mut("path") {
                    let value = path
                        .as_str()
                        .ok_or_else(|| Error::from_reason("dependency path must be a string"))?;
                    let absolute = fs::canonicalize(root.join(value)).map_err(|error| {
                        Error::from_reason(format!("invalid dependency path {value:?}: {error}"))
                    })?;
                    *path = toml::Value::String(absolute.to_string_lossy().into_owned());
                }
            }
        }
        let runtime = Path::new(env!("CARGO_MANIFEST_DIR")).join("../runtime-next");
        let runtime = fs::canonicalize(runtime)
            .map_err(|error| Error::from_reason(format!("cannot locate runtime-next: {error}")))?;
        dependencies.insert(
            "runtime-next".to_owned(),
            toml::Value::Table(toml::Table::from_iter([(
                "path".to_owned(),
                toml::Value::String(runtime.to_string_lossy().into_owned()),
            )])),
        );

        Ok(Self {
            db: RootDatabase::default(),
            files: BTreeMap::new(),
            directory: root.join(".rustier-next"),
            release: options.release.unwrap_or(false),
            dependencies,
        })
    }

    #[napi]
    pub fn transform(&mut self, module_id: String, contents: String) -> Result<Option<String>> {
        let file = match self.files.get(&module_id) {
            Some(file) => {
                let file = *file;
                file.set_contents(&mut self.db).to(contents);
                file
            }
            None => {
                let file = JsFile::new(&self.db, module_id.clone(), contents);
                self.files.insert(module_id.clone(), file);
                file
            }
        };
        let macros = file.macros(&self.db).as_ref().map_err(|diagnostics| {
            Error::from_reason(format!(
                "{module_id}:\n{}",
                diagnostics
                    .iter()
                    .map(|diagnostic| format!("{:?}: {}", diagnostic.range, diagnostic.message))
                    .collect::<Vec<_>>()
                    .join("\n")
            ))
        })?;
        if macros.is_empty() {
            return Ok(None);
        }

        let source = file.contents(&self.db);
        let mut invoke = format!("{}_invoke", macros[0].name(&self.db));
        while source.contains(&invoke) {
            invoke.push('_');
        }
        let start = if source.starts_with("#!") {
            source.find('\n').map_or(source.len(), |index| index + 1)
        } else {
            0
        };
        let mut output = source[..start].to_owned();
        output.push_str(&format!(
            "import {{ invoke as {invoke} }} from \"rustier-next:runtime\";\n"
        ));
        output.push_str(&self.render_range(
            source,
            macros,
            SourceRange {
                start: start as u32,
                end: source.len() as u32,
            },
            &invoke,
        ));

        Ok(Some(output))
    }

    #[napi]
    pub fn retain_modules(&mut self, modules: Vec<String>) {
        self.files.retain(|id, _| modules.contains(id));
    }

    #[napi]
    pub fn build(&self) -> Result<Buffer> {
        let mut macros = Vec::new();
        for (module_id, file) in &self.files {
            let entries = file.macros(&self.db).as_ref().map_err(|diagnostics| {
                Error::from_reason(format!("{module_id}: {diagnostics:?}"))
            })?;
            macros.extend(entries.iter().copied());
        }
        let source = Transport::new(&self.db, macros).render(&self.db);
        let manifest = toml::Table::from_iter([
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
                toml::Value::Table(toml::Table::from_iter([(
                    "crate-type".to_owned(),
                    toml::Value::Array(vec!["cdylib".into()]),
                )])),
            ),
            (
                "dependencies".to_owned(),
                toml::Value::Table(self.dependencies.clone()),
            ),
            (
                "workspace".to_owned(),
                toml::Value::Table(toml::Table::new()),
            ),
        ]);
        fs::create_dir_all(self.directory.join("src"))
            .and_then(|_| fs::write(self.directory.join("src/lib.rs"), source))
            .map_err(|error| Error::from_reason(format!("cannot write generated Rust: {error}")))?;
        let manifest = toml::to_string(&manifest).map_err(|error| {
            Error::from_reason(format!("cannot generate Cargo manifest: {error}"))
        })?;
        fs::write(self.directory.join("Cargo.toml"), manifest)
            .map_err(|error| Error::from_reason(format!("cannot write Cargo manifest: {error}")))?;

        let mut command = Command::new("cargo");
        command
            .args([
                "build",
                "--target",
                "wasm32-unknown-unknown",
                "--manifest-path",
            ])
            .arg(self.directory.join("Cargo.toml"))
            .arg("--target-dir")
            .arg(self.directory.join("target"));
        if self.release {
            command.arg("--release");
        }
        let build = command
            .output()
            .map_err(|error| Error::from_reason(format!("cannot run Cargo: {error}")))?;
        if !build.status.success() {
            return Err(Error::from_reason(
                String::from_utf8_lossy(&build.stderr).into_owned(),
            ));
        }
        let profile = if self.release { "release" } else { "debug" };
        let artifact = self
            .directory
            .join("target/wasm32-unknown-unknown")
            .join(profile)
            .join("rustier_snippets.wasm");

        fs::read(artifact)
            .map(Buffer::from)
            .map_err(|error| Error::from_reason(format!("cannot read compiled Wasm: {error}")))
    }

    #[napi]
    pub fn js_runtime(&self) -> String {
        runtime_next::transport::WASM_RUNTIME.to_owned()
    }
}

impl Server {
    fn render_range(
        &self,
        source: &str,
        macros: &[Macro<'_>],
        range: SourceRange,
        invoke: &str,
    ) -> String {
        let mut output = String::new();
        let mut cursor = range.start;
        for macro_ in macros {
            let span = macro_.range(&self.db);
            if span.start < cursor || span.end > range.end {
                continue;
            }
            output.push_str(&source[cursor as usize..span.start as usize]);
            output.push_str(invoke);
            output.push('(');
            output.push_str(&serde_json::to_string(macro_.name(&self.db)).unwrap());
            output.push_str(", [");
            let mut first = true;
            for part in macro_.body(&self.db) {
                if let Part::Interpolation { expression, .. } = part {
                    if !first {
                        output.push_str(", ");
                    }
                    first = false;
                    output.push_str(&self.render_range(source, macros, *expression, invoke));
                }
            }
            output.push_str("])");
            cursor = span.end;
        }
        output.push_str(&source[cursor as usize..range.end as usize]);

        output
    }
}
