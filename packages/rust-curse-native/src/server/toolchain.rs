//! Resolve, install, and invoke the build tools used by rust-curse.

use std::{
    env, fs,
    io::Cursor,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};

use anyhow::{Context, Result, bail, ensure};
use cargo_metadata::Message;

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
    fn configure(&self, command: &mut Command) -> Result<()> {
        let bin = self
            .cargo
            .parent()
            .context("Cargo executable has no parent")?;
        let mut paths = vec![bin.to_owned()];
        paths.extend(env::split_paths(&env::var_os("PATH").unwrap_or_default()));
        command.env("PATH", env::join_paths(paths)?);
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

    fn command(&self) -> Result<Command> {
        let mut command = Command::new(&self.cargo);
        self.configure(&mut command)?;
        Ok(command)
    }
}

impl Toolchain {
    /// Installs or locates all tools.
    pub fn install(ops: ToolchainOps<'_>) -> Result<Self> {
        let cargo = install_rust(&ops.cargo)?;
        let napi = resolve(&ops.napi, "napi", |root| {
            let bin = root.join("node_modules/.bin").join(script("napi"));
            if !working(&bin) {
                fs::create_dir_all(root)?;
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
    pub fn exec(&self, toolchain: &Toolchain) -> Result<NapiBuildOutput> {
        let manifest = fs::canonicalize(self.manifest_path)
            .with_context(|| format!("resolve manifest {}", self.manifest_path.display()))?;
        let root = manifest.parent().context("manifest has no parent")?;
        let output_dir = prepare_output(self.output_dir)?;
        // The NAPI CLI requires a Node package root; preserve any existing file.
        let package = root.join("package.json");
        if !package.exists() {
            fs::write(&package, "{\"private\":true}\n")
                .with_context(|| format!("create {}", package.display()))?;
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
        run(&mut command)?;
        let js_path = require_file(output_dir.join("index.js"))?;
        let mut addons = Vec::new();
        for entry in fs::read_dir(&output_dir)? {
            let path = entry?.path();
            if path.is_file() && path.extension().is_some_and(|ext| ext == "node") {
                addons.push(path);
            }
        }
        ensure!(
            addons.len() == 1,
            "expected one .node file in {}, found {:?}",
            output_dir.display(),
            addons
        );
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
    pub fn exec(&self, toolchain: &Toolchain) -> Result<WasmBindgenOutput> {
        let input = fs::canonicalize(self.wasm_path)
            .with_context(|| format!("resolve WASM input {}", self.wasm_path.display()))?;
        let name = input
            .file_stem()
            .and_then(|name| name.to_str())
            .context("WASM input must have a UTF-8 file stem")?;
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
    pub fn exec(&self, toolchain: &Toolchain) -> Result<CargoBuildOutput> {
        let manifest = fs::canonicalize(self.manifest_path)
            .with_context(|| format!("resolve manifest {}", self.manifest_path.display()))?;
        let mut command = toolchain.cargo.command()?;
        command
            .args(["build", "--message-format=json-render-diagnostics"])
            .arg("--manifest-path")
            .arg(&manifest)
            .arg("--target-dir")
            .arg(absolute(self.target_dir)?)
            .current_dir(manifest.parent().context("manifest has no parent")?);
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
                message.context("parse Cargo build output")?
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

fn install_rust(opts: &ToolInstallationOpts<'_>) -> Result<RustTool> {
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
                .context("rustup returned a non-UTF-8 path")?
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
    install: impl FnOnce(&Path) -> Result<PathBuf>,
) -> Result<PathBuf> {
    let path = match opts {
        ToolInstallationOpts::Locate(path) => absolute(path)?,
        ToolInstallationOpts::Discover => discover(name)?,
        ToolInstallationOpts::Install(root) => install(&absolute(root)?)?,
    };
    run(Command::new(&path).arg("--version"))
        .with_context(|| format!("validate {name} at {}", path.display()))?;
    Ok(path)
}

fn discover(name: &str) -> Result<PathBuf> {
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
    bail!("could not find a working {name} on PATH; install it or use Locate/Install")
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

fn absolute(path: &Path) -> Result<PathBuf> {
    std::path::absolute(path).with_context(|| format!("resolve {}", path.display()))
}

fn prepare_output(path: &Path) -> Result<PathBuf> {
    let path = absolute(path)?;
    fs::create_dir_all(&path)
        .with_context(|| format!("create output directory {}", path.display()))?;
    Ok(path)
}

fn require_file(path: PathBuf) -> Result<PathBuf> {
    ensure!(path.is_file(), "tool did not generate {}", path.display());
    Ok(path)
}

fn run(command: &mut Command) -> Result<Output> {
    let description = format!("{command:?}");
    tracing::debug!(command = %description, "running build tool");
    let output = command
        .stdin(Stdio::null())
        .output()
        .with_context(|| format!("failed to execute {description}"))?;
    ensure!(
        output.status.success(),
        "{description} failed ({})\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    Ok(output)
}
