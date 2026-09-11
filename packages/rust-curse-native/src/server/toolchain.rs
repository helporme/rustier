//! Resolve, install, and invoke the build tools used by rust-curse.

use std::{
    env, fs,
    io::Cursor,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};

use cargo_metadata::Message;

/// A failed tool invocation, with compiler diagnostics and human-readable
/// output.
#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("{message}")]
#[diagnostic(code(rust_curse::build_failed))]
pub struct BuildFailure {
    pub message: String,
    #[help]
    pub rendered: String,
}

#[derive(Debug, Clone, Copy)]
pub struct ToolchainOps<'a> {
    pub napi: ToolInstallationOpts<'a>,
    pub wasm_bindgen: ToolInstallationOpts<'a>,
    pub cargo: ToolInstallationOpts<'a>,
}

/// Where to obtain a tool. Relative paths are resolved at installation time.
#[derive(Debug, Clone, Copy)]
pub enum ToolInstallationOpts<'a> {
    /// Use the executable at this path, returning error if not found.
    Locate(&'a Path),
    /// Use installation at this path, or install it if not found.
    Install(&'a Path),
    /// Search PATH for a working executable.
    Discover,
}

/// Build tools resolved together by [`Toolchain::install`].
#[derive(Debug)]
pub struct Toolchain {
    napi: PathBuf,
    wasm_bindgen: PathBuf,
    cargo: RustTool,
}

#[derive(Debug)]
struct RustTool {
    cargo: PathBuf,
    homes: Option<(PathBuf, PathBuf)>,
}

impl RustTool {
    fn configure(&self, command: &mut Command) -> Result<(), ToolchainError> {
        let bin = self
            .cargo
            .parent()
            .ok_or_else(|| ToolchainError::MissingParent {
                path: self.cargo.clone(),
            })?;
        let mut paths = vec![bin.to_owned()];
        paths.extend(env::split_paths(&env::var_os("PATH").unwrap_or_default()));
        command.env(
            "PATH",
            env::join_paths(paths).map_err(ToolchainError::InvalidSearchPath)?,
        );
        command.env("CARGO", &self.cargo);
        if let Some((rustup_home, cargo_home)) = &self.homes {
            command.env("RUSTUP_HOME", rustup_home);
            command.env("CARGO_HOME", cargo_home);
            // Use the installed compiler even in a project with a rustup override.
            command.env("RUSTC", bin.join(executable("rustc")));
            command.env("RUSTDOC", bin.join(executable("rustdoc")));
        }
        Ok(())
    }

    fn command(&self) -> Result<Command, ToolchainError> {
        let mut command = Command::new(&self.cargo);
        self.configure(&mut command)?;
        Ok(command)
    }
}

impl Toolchain {
    /// Installs or locates all tools.
    pub fn install(ops: ToolchainOps<'_>) -> Result<Self, ToolchainError> {
        let cargo = install_rust(&ops.cargo)?;
        let napi = resolve(&ops.napi, "napi", |root| {
            let bin = root.join("node_modules/.bin").join(script("napi"));
            if !working(&bin) {
                fs::create_dir_all(root).map_err(|source| ToolchainError::Io {
                    operation: "create tool installation directory",
                    path: root.to_owned(),
                    source,
                })?;
                let mut command = Command::new(discover("npm")?);
                command
                    .args(["install", "--ignore-scripts", "--no-audit", "--no-fund"])
                    .arg("--prefix")
                    .arg(root)
                    .arg("@napi-rs/cli@3.7.2");
                run(&mut command)?;
            }
            Ok(bin)
        })?;
        let wasm_bindgen = resolve(&ops.wasm_bindgen, "wasm-bindgen", |root| {
            let bin = root.join("bin").join(executable("wasm-bindgen"));
            if !working(&bin) {
                let mut command = cargo.command()?;
                command
                    .args(["install", "wasm-bindgen-cli", "--locked", "--force"])
                    .arg("--root")
                    .arg(root);
                run(&mut command)?;
            }
            Ok(bin)
        })?;
        Ok(Self {
            napi,
            wasm_bindgen,
            cargo,
        })
    }
}

/// Build a native Node.js addon and an ESM JavaScript loader with NAPI-RS.
#[derive(Debug, Clone, Copy)]
pub struct NapiBuildCommand<'a> {
    /// Cargo manifest of the crate to build.
    pub manifest_path: &'a Path,
    /// Directory receiving `index.js` and the platform-specific `.node` file.
    /// Use a separate output directory for each build target.
    pub output_dir: &'a Path,
    /// Build with Cargo's release profile.
    pub release: bool,
}

impl NapiBuildCommand<'_> {
    pub fn exec(&self, toolchain: &Toolchain) -> Result<NapiBuildOutput, ToolchainError> {
        let manifest =
            fs::canonicalize(self.manifest_path).map_err(|source| ToolchainError::Io {
                operation: "resolve manifest",
                path: self.manifest_path.to_owned(),
                source,
            })?;
        let root = manifest
            .parent()
            .ok_or_else(|| ToolchainError::MissingParent {
                path: manifest.clone(),
            })?;
        let output_dir = prepare_output(self.output_dir)?;
        // The NAPI CLI requires a Node package root; preserve any existing file.
        let package = root.join("package.json");
        if !package.exists() {
            fs::write(&package, "{\"private\":true}\n").map_err(|source| ToolchainError::Io {
                operation: "create package manifest",
                path: package.clone(),
                source,
            })?;
        }
        let mut command = Command::new(&toolchain.napi);
        toolchain.cargo.configure(&mut command)?;
        command
            .arg("build")
            .arg("--manifest-path")
            .arg(&manifest)
            .arg("--output-dir")
            .arg(&output_dir)
            .args(["--platform", "--esm", "--js", "index.js"])
            .current_dir(root);
        if self.release {
            command.arg("--release");
        }
        command.args(["--", "--message-format=json-diagnostic-rendered-ansi"]);
        run(&mut command)?;
        let js_path = require_file(output_dir.join("index.js"))?;
        let mut addons = Vec::new();
        for entry in fs::read_dir(&output_dir).map_err(|source| ToolchainError::Io {
            operation: "read output directory",
            path: output_dir.clone(),
            source,
        })? {
            let path = entry
                .map_err(|source| ToolchainError::Io {
                    operation: "read output directory entry in",
                    path: output_dir.clone(),
                    source,
                })?
                .path();
            if path.is_file() && path.extension().is_some_and(|ext| ext == "node") {
                addons.push(path);
            }
        }
        if addons.len() != 1 {
            return Err(ToolchainError::InvalidAddonOutput {
                directory: output_dir,
                addons,
            });
        }
        Ok(NapiBuildOutput {
            node_path: addons.remove(0),
            js_path,
        })
    }
}

/// Files generated by a successful NAPI build.
#[derive(Debug, Clone)]
pub struct NapiBuildOutput {
    /// Absolute path to the generated native Node.js addon.
    pub node_path: PathBuf,
    /// Absolute path to the generated JavaScript binding file.
    pub js_path: PathBuf,
}

/// JavaScript environment used by wasm-bindgen.
#[derive(Debug, Clone, Copy)]
pub enum WasmBindgenTarget {
    /// Browser ES modules initialized by the caller.
    Web,
    /// Node.js CommonJS bindings.
    NodeJs,
}

/// Generate JavaScript bindings from an already compiled WebAssembly artifact.
#[derive(Debug, Clone, Copy)]
pub struct WasmBindgenCommand<'a> {
    /// Input `.wasm` artifact, normally from [`CargoBuildCommand`].
    pub wasm_path: &'a Path,
    /// Directory receiving the JavaScript and processed WebAssembly files.
    pub output_dir: &'a Path,
    /// Runtime for the generated bindings.
    pub target: WasmBindgenTarget,
}

impl WasmBindgenCommand<'_> {
    /// Runs wasm-bindgen and verifies both generated artifacts.
    pub fn exec(&self, toolchain: &Toolchain) -> Result<WasmBindgenOutput, ToolchainError> {
        let input = fs::canonicalize(self.wasm_path).map_err(|source| ToolchainError::Io {
            operation: "resolve WASM input",
            path: self.wasm_path.to_owned(),
            source,
        })?;
        let name = input
            .file_stem()
            .and_then(|name| name.to_str())
            .ok_or_else(|| ToolchainError::InvalidWasmName {
                path: input.clone(),
            })?;
        let output_dir = prepare_output(self.output_dir)?;
        let mut command = Command::new(&toolchain.wasm_bindgen);
        command
            .arg(&input)
            .arg("--out-dir")
            .arg(&output_dir)
            .arg("--out-name")
            .arg(name)
            .arg("--target")
            .arg(match self.target {
                WasmBindgenTarget::Web => "web",
                WasmBindgenTarget::NodeJs => "nodejs",
            });
        run(&mut command)?;
        Ok(WasmBindgenOutput {
            js_path: require_file(output_dir.join(format!("{name}.js")))?,
            wasm_path: require_file(output_dir.join(format!("{name}_bg.wasm")))?,
        })
    }
}

/// Files generated by wasm-bindgen.
#[derive(Debug, Clone)]
pub struct WasmBindgenOutput {
    /// Absolute path to the JavaScript glue.
    pub js_path: PathBuf,
    /// Absolute path to the processed WebAssembly binary.
    pub wasm_path: PathBuf,
}

/// Build a Cargo package and collect its artifacts from Cargo's JSON messages.
#[derive(Debug, Clone, Copy)]
pub struct CargoBuildCommand<'a> {
    /// Manifest to build.
    pub manifest_path: &'a Path,
    /// Cargo target directory; independent of the manifest's location.
    pub target_dir: &'a Path,
    /// Target triple, or `None` for the host.
    pub target: Option<&'a str>,
    /// Build with Cargo's release profile.
    pub release: bool,
}

impl CargoBuildCommand<'_> {
    pub fn exec(&self, toolchain: &Toolchain) -> Result<CargoBuildOutput, ToolchainError> {
        let manifest =
            fs::canonicalize(self.manifest_path).map_err(|source| ToolchainError::Io {
                operation: "resolve manifest",
                path: self.manifest_path.to_owned(),
                source,
            })?;
        let mut command = toolchain.cargo.command()?;
        command
            .args(["build", "--message-format=json-diagnostic-rendered-ansi"])
            .arg("--manifest-path")
            .arg(&manifest)
            .arg("--target-dir")
            .arg(absolute(self.target_dir)?)
            .current_dir(
                manifest
                    .parent()
                    .ok_or_else(|| ToolchainError::MissingParent {
                        path: manifest.clone(),
                    })?,
            );
        if let Some(target) = self.target {
            command.arg("--target").arg(target);
        }
        if self.release {
            command.arg("--release");
        }
        let output = run(&mut command)?;
        let mut artifacts = Vec::new();
        for message in Message::parse_stream(Cursor::new(output.stdout)) {
            if let Message::CompilerArtifact(artifact) =
                message.map_err(ToolchainError::ParseCargoOutput)?
            {
                artifacts.extend(
                    artifact
                        .filenames
                        .into_iter()
                        .map(|path| path.into_std_path_buf()),
                );
            }
        }
        artifacts.sort();
        artifacts.dedup();
        Ok(CargoBuildOutput { artifacts })
    }
}

/// Artifacts reported by Cargo, sorted and deduplicated.
#[derive(Debug, Clone)]
pub struct CargoBuildOutput {
    /// Files emitted for the requested package and its dependencies.
    pub artifacts: Vec<PathBuf>,
}

fn install_rust(opts: &ToolInstallationOpts<'_>) -> Result<RustTool, ToolchainError> {
    if let ToolInstallationOpts::Install(root) = opts {
        let root = absolute(root)?;
        let rustup_home = root.join("rustup");
        let cargo_home = root.join("cargo");
        let rustup = discover("rustup")?;
        let mut install = Command::new(&rustup);
        install
            .env("RUSTUP_HOME", &rustup_home)
            .env("CARGO_HOME", &cargo_home)
            .args([
                "toolchain",
                "install",
                "stable",
                "--profile",
                "minimal",
                "--target",
                "wasm32-unknown-unknown",
                "--no-self-update",
            ]);
        run(&mut install)?;
        let mut which = Command::new(&rustup);
        which
            .env("RUSTUP_HOME", &rustup_home)
            .env("CARGO_HOME", &cargo_home)
            .args(["which", "--toolchain", "stable", "cargo"]);
        let output = run(&mut which)?;
        let cargo = PathBuf::from(
            String::from_utf8(output.stdout)
                .map_err(ToolchainError::InvalidRustupPath)?
                .trim(),
        );
        let tool = RustTool {
            cargo,
            homes: Some((rustup_home, cargo_home)),
        };
        run(tool.command()?.arg("--version"))?;
        Ok(tool)
    } else {
        let cargo = resolve(opts, "cargo", |_| unreachable!())?;
        Ok(RustTool { cargo, homes: None })
    }
}

fn resolve(
    opts: &ToolInstallationOpts<'_>,
    name: &str,
    install: impl FnOnce(&Path) -> Result<PathBuf, ToolchainError>,
) -> Result<PathBuf, ToolchainError> {
    let path = match opts {
        ToolInstallationOpts::Locate(path) => absolute(path)?,
        ToolInstallationOpts::Discover => discover(name)?,
        ToolInstallationOpts::Install(root) => install(&absolute(root)?)?,
    };
    run(Command::new(&path).arg("--version"))?;
    Ok(path)
}

fn discover(name: &str) -> Result<PathBuf, ToolchainError> {
    let names = if cfg!(windows) {
        vec![executable(name), script(name), name.to_owned()]
    } else {
        vec![name.to_owned()]
    };
    for dir in env::split_paths(&env::var_os("PATH").unwrap_or_default()) {
        for name in &names {
            let path = absolute(&dir.join(name))?;
            if working(&path) {
                return Ok(path);
            }
        }
    }
    Err(ToolchainError::ToolNotFound {
        name: name.to_owned(),
    })
}

fn working(path: &Path) -> bool {
    path.is_file() && run(Command::new(path).arg("--version")).is_ok()
}

fn executable(name: &str) -> String {
    format!("{name}{}", env::consts::EXE_SUFFIX)
}

fn script(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.cmd")
    } else {
        name.to_owned()
    }
}

fn absolute(path: &Path) -> Result<PathBuf, ToolchainError> {
    std::path::absolute(path).map_err(|source| ToolchainError::Io {
        operation: "resolve path",
        path: path.to_owned(),
        source,
    })
}

fn prepare_output(path: &Path) -> Result<PathBuf, ToolchainError> {
    let path = absolute(path)?;
    fs::create_dir_all(&path).map_err(|source| ToolchainError::Io {
        operation: "create output directory",
        path: path.clone(),
        source,
    })?;
    Ok(path)
}

fn require_file(path: PathBuf) -> Result<PathBuf, ToolchainError> {
    if !path.is_file() {
        return Err(ToolchainError::MissingOutput { path });
    }
    Ok(path)
}

fn run(command: &mut Command) -> Result<Output, ToolchainError> {
    let program = Path::new(command.get_program())
        .file_name()
        .unwrap_or(command.get_program())
        .to_string_lossy()
        .into_owned();
    let cwd = absolute(command.get_current_dir().unwrap_or_else(|| Path::new(".")))?;
    tracing::debug!(%program, cwd = %cwd.display(), "running build tool");
    let output =
        command
            .stdin(Stdio::null())
            .output()
            .map_err(|source| ToolchainError::CommandStartup {
                program: program.clone(),
                cwd: cwd.clone(),
                source,
            })?;
    if !output.status.success() {
        let mut failure = BuildFailure {
            message: format!("{program} failed ({})", output.status),
            rendered: String::new(),
        };
        // Parse compiler diagnostics from both streams, but prefer stderr for
        // plain text: NAPI's stdout usually repeats the failure with a JS stack.
        let has_stderr = !String::from_utf8_lossy(&output.stderr).trim().is_empty();
        for (index, stream) in [&output.stderr, &output.stdout].into_iter().enumerate() {
            let text = String::from_utf8_lossy(stream);
            for message in Message::parse_stream(Cursor::new(text.as_bytes())) {
                match message {
                    Ok(Message::CompilerMessage(message)) => {
                        if let Some(rendered) = &message.message.rendered {
                            failure.rendered.push_str(rendered);
                            if !rendered.ends_with('\n') {
                                failure.rendered.push('\n');
                            }
                        }
                    }
                    Ok(Message::TextLine(line))
                        if (index == 0 || !has_stderr)
                            && serde_json::from_str::<serde_json::Value>(&line).is_err() =>
                    {
                        failure.rendered.push_str(&line);
                        failure.rendered.push('\n');
                    }
                    _ => {}
                }
            }
        }
        return Err(failure.into());
    }
    Ok(output)
}

/// Failures while resolving tools, preparing builds, or running a build tool.
#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum ToolchainError {
    #[error("could not {operation} {}", path.display())]
    Io {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{} has no parent directory", path.display())]
    MissingParent { path: PathBuf },
    #[error("could not configure the build tool's PATH")]
    InvalidSearchPath(#[source] env::JoinPathsError),
    #[error("could not start {program} in {}", cwd.display())]
    #[diagnostic(help("Check that the executable exists and can be run from this directory."))]
    CommandStartup {
        program: String,
        cwd: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(transparent)]
    #[diagnostic(transparent)]
    CommandFailed(#[from] BuildFailure),
    #[error("could not find a working {name} on PATH")]
    #[diagnostic(help("Install the tool or configure its location using Locate/Install."))]
    ToolNotFound { name: String },
    #[error("tool did not generate {}", path.display())]
    MissingOutput { path: PathBuf },
    #[error("expected one .node file in {}, found {addons:?}", directory.display())]
    InvalidAddonOutput {
        directory: PathBuf,
        addons: Vec<PathBuf>,
    },
    #[error("WASM input {} must have a UTF-8 file stem", path.display())]
    InvalidWasmName { path: PathBuf },
    #[error("could not parse Cargo build output")]
    ParseCargoOutput(#[source] std::io::Error),
    #[error("rustup returned a non-UTF-8 path")]
    InvalidRustupPath(#[source] std::string::FromUtf8Error),
}
