use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use miette::Diagnostic;
use thiserror::Error;

use super::CraterToolOpts;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildOutput {
    pub node_path: PathBuf,
    pub js_path: PathBuf,
}

pub fn locate(opts: &CraterToolOpts) -> Option<PathBuf> {
    match opts {
        CraterToolOpts::Local { path, .. } => {
            let tool_path = tool_path(path);
            tool_path.is_file().then_some(tool_path)
        }
    }
}

pub fn sync(opts: &CraterToolOpts) -> Result<PathBuf, NapiToolError> {
    if let Some(tool_path) = locate(opts) {
        return Ok(tool_path);
    }

    match opts {
        CraterToolOpts::Local { path, auto_install } => {
            let tool_path = tool_path(path);
            if !auto_install {
                return Err(NapiToolError::NotInstalled { path: tool_path });
            }

            ensure_project(path)?;
            let output = Command::new("npm")
                .args([
                    "install",
                    "--ignore-scripts",
                    "--save-dev",
                    "@napi-rs/cli@3.7.2",
                ])
                .current_dir(path)
                .output()
                .map_err(|source| NapiToolError::RunNpm { source })?;
            ensure_success(&output, "failed to install the NAPI CLI")?;
            tool_path
                .is_file()
                .then_some(tool_path.clone())
                .ok_or(NapiToolError::NotInstalled { path: tool_path })
        }
    }
}

pub fn ensure_project(path: &Path) -> Result<(), NapiToolError> {
    fs::create_dir_all(path).map_err(|source| NapiToolError::CreateDirectory {
        path: path.to_path_buf(),
        source,
    })?;
    let package_json = path.join("package.json");
    if !package_json.is_file() {
        fs::write(&package_json, "{\n  \"private\": true\n}\n").map_err(|source| {
            NapiToolError::WritePackage {
                path: package_json,
                source,
            }
        })?;
    }
    Ok(())
}

pub fn build(
    crate_dir: &Path,
    output_dir: &Path,
    tool_path: &Path,
    release: bool,
) -> Result<BuildOutput, NapiToolError> {
    const GENERATED_JS_FILE: &str = "index.js";

    fs::create_dir_all(output_dir).map_err(|source| NapiToolError::CreateDirectory {
        path: output_dir.to_path_buf(),
        source,
    })?;

    let mut command = Command::new(tool_path);
    command
        .arg("build")
        .arg("--manifest-path")
        .arg(crate_dir.join("Cargo.toml"))
        .arg("--output-dir")
        .arg(output_dir)
        .arg("--platform")
        .arg("--esm")
        .arg("--js")
        .arg(GENERATED_JS_FILE)
        .current_dir(crate_dir);
    if release {
        command.arg("--release");
    }
    let output = command
        .output()
        .map_err(|source| NapiToolError::RunNapi { source })?;
    ensure_success(&output, "failed to build the Node crate")?;

    let js_path = output_dir.join(GENERATED_JS_FILE);
    if !js_path.is_file() {
        return Err(NapiToolError::MissingArtifact { path: js_path });
    }
    let mut node_paths = fs::read_dir(output_dir)
        .map_err(|source| NapiToolError::ReadOutput {
            path: output_dir.to_path_buf(),
            source,
        })?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "node")
        })
        .collect::<Vec<_>>();
    if node_paths.len() != 1 {
        return Err(NapiToolError::UnexpectedNodeArtifactCount {
            count: node_paths.len(),
        });
    }
    Ok(BuildOutput {
        node_path: node_paths.remove(0),
        js_path,
    })
}

fn tool_path(install_root: &Path) -> PathBuf {
    let name = if cfg!(windows) { "napi.cmd" } else { "napi" };
    install_root.join("node_modules").join(".bin").join(name)
}

fn ensure_success(output: &Output, context: &str) -> Result<(), NapiToolError> {
    output
        .status
        .success()
        .then_some(())
        .ok_or_else(|| NapiToolError::CommandFailed {
            context: context.to_owned(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
}

#[derive(Debug, Diagnostic, Error)]
pub enum NapiToolError {
    #[error("the NAPI CLI is not installed at {}", path.display())]
    NotInstalled { path: PathBuf },
    #[error("failed to create directory at {}", path.display())]
    CreateDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to write package metadata at {}", path.display())]
    WritePackage {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to run npm")]
    RunNpm {
        #[source]
        source: std::io::Error,
    },
    #[error("failed to run the NAPI CLI")]
    RunNapi {
        #[source]
        source: std::io::Error,
    },
    #[error("{context}\nstdout:\n{stdout}\nstderr:\n{stderr}")]
    CommandFailed {
        context: String,
        stdout: String,
        stderr: String,
    },
    #[error("failed to read output directory at {}", path.display())]
    ReadOutput {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("missing generated artifact at {}", path.display())]
    MissingArtifact { path: PathBuf },
    #[error("expected one generated .node artifact, found {count}")]
    UnexpectedNodeArtifactCount { count: usize },
}
