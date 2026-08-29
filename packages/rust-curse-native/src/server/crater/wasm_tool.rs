use std::{
    fs,
    io::Cursor,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use cargo_metadata::Message;
use miette::Diagnostic;
use thiserror::Error;

use crate::compiler::WasmEnv;

use super::CraterToolOpts;

const WASM_TARGET: &str = "wasm32-unknown-unknown";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildOutput {
    pub wasm_abs_path: PathBuf,
    pub js_abs_path: PathBuf,
}

pub fn locate(opts: &CraterToolOpts) -> Option<PathBuf> {
    match opts {
        CraterToolOpts::Local { path, .. } => {
            let tool_path = tool_path(path);
            tool_path.is_file().then_some(tool_path)
        }
    }
}

pub fn sync(opts: &CraterToolOpts) -> Result<PathBuf, WasmToolError> {
    let CraterToolOpts::Local { auto_install, .. } = opts;
    let installed = Command::new("rustup")
        .args(["target", "list", "--installed"])
        .output()
        .map_err(|source| WasmToolError::RunCommand { source })?;
    ensure_success(&installed, "failed to list installed Rust targets")?;
    if !String::from_utf8_lossy(&installed.stdout)
        .lines()
        .any(|target| target.trim() == WASM_TARGET)
    {
        if !*auto_install {
            return Err(WasmToolError::MissingWasmTarget);
        }
        let output = Command::new("rustup")
            .args(["target", "add", WASM_TARGET])
            .output()
            .map_err(|source| WasmToolError::RunCommand { source })?;
        ensure_success(&output, "failed to install the Wasm Rust target")?;
    }

    if let Some(tool_path) = locate(opts) {
        return Ok(tool_path);
    }

    match opts {
        CraterToolOpts::Local { path, auto_install } => {
            let tool_path = tool_path(path);
            if !auto_install {
                return Err(WasmToolError::NotInstalled { path: tool_path });
            }

            fs::create_dir_all(path).map_err(|source| WasmToolError::CreateDirectory {
                path: path.clone(),
                source,
            })?;
            let output = Command::new("cargo")
                .args(["install", "wasm-bindgen-cli", "--root"])
                .arg(path)
                .output()
                .map_err(|source| WasmToolError::RunCommand { source })?;
            ensure_success(&output, "failed to install wasm-bindgen")?;
            tool_path
                .is_file()
                .then_some(tool_path.clone())
                .ok_or(WasmToolError::NotInstalled { path: tool_path })
        }
    }
}

pub fn build(
    crate_dir: &Path,
    output_dir: &Path,
    tool_path: &Path,
    release: bool,
    target: WasmEnv,
) -> Result<BuildOutput, WasmToolError> {
    let mut command = Command::new("cargo");
    command
        .arg("build")
        .arg("--target")
        .arg(WASM_TARGET)
        .arg("--message-format=json-render-diagnostics")
        .current_dir(crate_dir);
    if release {
        command.arg("--release");
    }
    let output = command
        .output()
        .map_err(|source| WasmToolError::RunCommand { source })?;
    ensure_success(&output, "failed to build the Wasm crate")?;
    let wasm_input = built_wasm_artifact(&output)?;
    let out_name = wasm_input
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| WasmToolError::MissingFileStem {
            path: wasm_input.clone(),
        })?;

    fs::create_dir_all(output_dir).map_err(|source| WasmToolError::CreateDirectory {
        path: output_dir.to_path_buf(),
        source,
    })?;
    let target = match target {
        WasmEnv::Web => "web",
        WasmEnv::Node => "nodejs",
    };
    let output = Command::new(tool_path)
        .arg(&wasm_input)
        .arg("--out-dir")
        .arg(output_dir)
        .arg("--target")
        .arg(target)
        .arg("--out-name")
        .arg(out_name)
        .output()
        .map_err(|source| WasmToolError::RunCommand { source })?;
    ensure_success(&output, "failed to generate wasm-bindgen bindings")?;

    let js_abs_path = output_dir.join(format!("{out_name}.js"));
    let wasm_abs_path = output_dir.join(format!("{out_name}_bg.wasm"));
    if !js_abs_path.is_file() {
        return Err(WasmToolError::MissingArtifact { path: js_abs_path });
    }
    if !wasm_abs_path.is_file() {
        return Err(WasmToolError::MissingArtifact {
            path: wasm_abs_path,
        });
    }
    Ok(BuildOutput {
        wasm_abs_path,
        js_abs_path,
    })
}

fn built_wasm_artifact(output: &Output) -> Result<PathBuf, WasmToolError> {
    let mut paths = Vec::new();
    for message in Message::parse_stream(Cursor::new(&output.stdout)) {
        let Message::CompilerArtifact(artifact) =
            message.map_err(|source| WasmToolError::ParseCargoMessage { source })?
        else {
            continue;
        };
        paths.extend(
            artifact
                .filenames
                .into_iter()
                .map(|path| path.into_std_path_buf())
                .filter(|path| {
                    path.extension()
                        .is_some_and(|extension| extension == "wasm")
                }),
        );
    }
    if paths.len() != 1 {
        return Err(WasmToolError::UnexpectedWasmArtifactCount { count: paths.len() });
    }
    Ok(paths.remove(0))
}

fn tool_path(install_root: &Path) -> PathBuf {
    let name = if std::env::consts::EXE_EXTENSION.is_empty() {
        "wasm-bindgen".to_owned()
    } else {
        format!("wasm-bindgen.{}", std::env::consts::EXE_EXTENSION)
    };
    install_root.join("bin").join(name)
}

fn ensure_success(output: &Output, context: &str) -> Result<(), WasmToolError> {
    output
        .status
        .success()
        .then_some(())
        .ok_or_else(|| WasmToolError::CommandFailed {
            context: context.to_owned(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
}

#[derive(Debug, Diagnostic, Error)]
pub enum WasmToolError {
    #[error("the {WASM_TARGET} Rust target is not installed")]
    MissingWasmTarget,
    #[error("wasm-bindgen is not installed at {}", path.display())]
    NotInstalled { path: PathBuf },
    #[error("failed to create directory at {}", path.display())]
    CreateDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to run external command")]
    RunCommand {
        #[source]
        source: std::io::Error,
    },
    #[error("{context}\nstdout:\n{stdout}\nstderr:\n{stderr}")]
    CommandFailed {
        context: String,
        stdout: String,
        stderr: String,
    },
    #[error("failed to parse Cargo build output")]
    ParseCargoMessage {
        #[source]
        source: std::io::Error,
    },
    #[error("expected one generated .wasm artifact, found {count}")]
    UnexpectedWasmArtifactCount { count: usize },
    #[error("Wasm artifact has no valid file stem: {}", path.display())]
    MissingFileStem { path: PathBuf },
    #[error("missing generated artifact at {}", path.display())]
    MissingArtifact { path: PathBuf },
}
