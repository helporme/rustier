use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::PathBuf,
    process::Command,
};

use compiler::{
    codegen::{TransportBackend, TransportBackendWasm, TransportCodegenOpts, WasmImport},
    db::RootDatabase,
    diag::Theme,
    ir::js::{JsFile, JsProgram},
};
use napi::{Error, Result};
use napi_derive::napi;
use salsa::Setter;

#[napi(object)]
pub struct ServerOptions {
    pub root_dir: String,
    pub release: Option<bool>,
    pub mem64: Option<bool>,
    pub wasm_import: Option<String>,
    pub crates: Option<HashMap<String, String>>,
}

#[napi(object)]
pub struct BuildResult {
    pub diagnostics: Vec<String>,
    pub bin_ffi_path: Option<String>,
}

#[napi]
pub struct Server {
    db: RootDatabase,
    program: JsProgram,
    directory: PathBuf,
    release: bool,
    backend: TransportBackend,
    js_ffi_mod_id: String,
    bin_ffi_mod_id: String,
    js_modules: HashMap<String, String>,
    crates: BTreeMap<String, String>,
    next_artifact: u64,
}

#[napi]
impl Server {
    #[napi(constructor)]
    pub fn new(options: ServerOptions) -> Result<Self> {
        let root = fs::canonicalize(&options.root_dir)
            .map_err(|error| Error::from_reason(format!("invalid project root: {error}")))?;
        let db = RootDatabase::default();
        let program = JsProgram::new(&db, Vec::new());
        let import = match options.wasm_import.as_deref().unwrap_or("fetch") {
            "fetch" => WasmImport::Fetch,
            "node-js" => WasmImport::NodeJs,
            "module" => WasmImport::Module,
            mode => {
                return Err(Error::from_reason(format!(
                    "unsupported Wasm import mode: {mode}"
                )));
            }
        };
        let backend = TransportBackend::Wasm(TransportBackendWasm {
            mem64: options.mem64.unwrap_or(false),
            import,
        });
        let directory = root.join(".rustier/bridge");
        let js_ffi_mod_id = directory.join("ffi.mjs").to_string_lossy().into_owned();
        let bin_ffi_mod_id = directory.join("binary.mjs").to_string_lossy().into_owned();
        let opts = TransportCodegenOpts {
            ffi_module_id: js_ffi_mod_id.clone(),
            ffi_bin_id: bin_ffi_mod_id.clone(),
        };
        let js_modules = HashMap::from([(js_ffi_mod_id.clone(), backend.render_js_backend(&opts))]);
        let crates = options
            .crates
            .unwrap_or_default()
            .into_iter()
            .map(|(name, path)| {
                let path = fs::canonicalize(root.join(path)).map_err(|error| {
                    Error::from_reason(format!("invalid crate path for {name}: {error}"))
                })?;
                Ok((name, path.to_string_lossy().into_owned()))
            })
            .collect::<Result<BTreeMap<_, _>>>()?;

        Ok(Self {
            db,
            program,
            directory,
            release: options.release.unwrap_or(false),
            backend,
            js_ffi_mod_id,
            bin_ffi_mod_id,
            js_modules,
            crates,
            next_artifact: 0,
        })
    }

    #[napi(getter)]
    pub fn js_ffi_mod_id(&self) -> String {
        self.js_ffi_mod_id.clone()
    }

    #[napi(getter)]
    pub fn bin_ffi_mod_id(&self) -> String {
        self.bin_ffi_mod_id.clone()
    }

    /// Compiler-generated modules available before the Rust build.
    #[napi]
    pub fn js_ffi_module(&self, module_id: String) -> Option<String> {
        self.js_modules.get(&module_id).cloned()
    }

    #[napi]
    pub fn transform(&mut self, module_id: String, contents: String) -> Result<String> {
        let file = JsFile::new(&self.db, module_id.clone(), contents);
        let mut files = self.program.files(&self.db).clone();
        match files
            .iter_mut()
            .find(|entry| entry.module_id(&self.db) == &module_id)
        {
            Some(entry) => *entry = file,
            None => files.push(file),
        }
        self.program.set_files(&mut self.db).to(files);

        let diagnostics = file.diagnostics(&self.db);
        if !diagnostics.is_empty() {
            let messages = diagnostics
                .iter()
                .map(|diagnostic| diagnostic.render(Theme::UnicodeNoColor))
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|error| Error::from_reason(error.to_string()))?;
            return Err(Error::from_reason(messages.join("\n")));
        }

        let ffi_module_id = format!("{}?source={module_id}", self.js_ffi_mod_id);
        if file.module(&self.db).macros(&self.db).is_empty() {
            self.js_modules.remove(&ffi_module_id);
        } else {
            let program = JsProgram::new(&self.db, vec![file]);
            let opts = TransportCodegenOpts {
                ffi_module_id: self.js_ffi_mod_id.clone(),
                ffi_bin_id: self.bin_ffi_mod_id.clone(),
            };
            self.js_modules.insert(
                ffi_module_id.clone(),
                program.render_js_ffi_exports(&self.db, &self.backend, &opts),
            );
        }
        Ok(file
            .module(&self.db)
            .render_transformed(&self.db, ffi_module_id)
            .clone())
    }

    #[napi]
    pub fn build(&mut self) -> Result<BuildResult> {
        let artifacts = self.directory.join("artifacts");
        fs::create_dir_all(&artifacts).map_err(|error| {
            Error::from_reason(format!("cannot create artifacts directory: {error}"))
        })?;
        for entry in fs::read_dir(&artifacts)
            .map_err(|error| Error::from_reason(format!("cannot list artifacts: {error}")))?
        {
            let entry = entry.map_err(|error| Error::from_reason(error.to_string()))?;
            let Some(number) = entry
                .file_name()
                .to_str()
                .and_then(|name| name.parse::<u64>().ok())
            else {
                continue;
            };
            if entry
                .file_type()
                .map_err(|error| Error::from_reason(error.to_string()))?
                .is_dir()
            {
                let next = number
                    .checked_add(1)
                    .ok_or_else(|| Error::from_reason("artifact counter exhausted"))?;
                self.next_artifact = self.next_artifact.max(next);
                fs::remove_dir_all(entry.path()).map_err(|error| {
                    Error::from_reason(format!("cannot remove previous artifact: {error}"))
                })?;
            }
        }
        let number = self.next_artifact;
        self.next_artifact = number
            .checked_add(1)
            .ok_or_else(|| Error::from_reason("artifact counter exhausted"))?;
        let artifact_directory = artifacts.join(number.to_string());
        fs::create_dir_all(&artifact_directory).map_err(|error| {
            Error::from_reason(format!("cannot create artifact directory: {error}"))
        })?;
        let mut result = BuildResult {
            diagnostics: Vec::new(),
            bin_ffi_path: None,
        };
        result.diagnostics = self
            .program
            .diagnostics(&self.db)
            .iter()
            .map(|diagnostic| diagnostic.render(Theme::UnicodeNoColor))
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|error| Error::from_reason(error.to_string()))?;
        if !result.diagnostics.is_empty() {
            return Ok(result);
        }

        let source = self.program.render_rs_ffi(&self.db, &self.backend);
        let manifest = self.program.render_cargo_toml(&self.db, &self.crates);
        fs::write(self.directory.join("lib.rs"), source)
            .and_then(|_| fs::write(self.directory.join("Cargo.toml"), manifest))
            .map_err(|error| Error::from_reason(format!("cannot write bridge crate: {error}")))?;

        let TransportBackend::Wasm(wasm) = &self.backend;
        let target = if wasm.mem64 {
            "wasm64-unknown-unknown"
        } else {
            "wasm32-unknown-unknown"
        };
        let mut command = Command::new("cargo");
        command
            .args(["build", "--quiet", "--target", target, "--manifest-path"])
            .arg(self.directory.join("Cargo.toml"))
            .arg("--target-dir")
            .arg(self.directory.join("target"));
        if self.release {
            command.arg("--release");
        }
        let build = command
            .output()
            .map_err(|error| Error::from_reason(format!("cannot run Cargo: {error}")))?;
        let diagnostics = String::from_utf8_lossy(&build.stderr).trim().to_owned();
        if !diagnostics.is_empty() {
            result.diagnostics.push(diagnostics);
        }
        if !build.status.success() {
            if result.diagnostics.is_empty() {
                result
                    .diagnostics
                    .push(format!("Cargo exited with {}", build.status));
            }
            return Ok(result);
        }

        let profile = if self.release { "release" } else { "debug" };
        let source = self
            .directory
            .join("target")
            .join(target)
            .join(profile)
            .join("rustier_snippets.wasm");
        let binary = artifact_directory.join("bridge.wasm");
        fs::copy(source, &binary)
            .map_err(|error| Error::from_reason(format!("cannot store compiled Wasm: {error}")))?;

        result.bin_ffi_path = Some(binary.to_string_lossy().into_owned());

        Ok(result)
    }
}
