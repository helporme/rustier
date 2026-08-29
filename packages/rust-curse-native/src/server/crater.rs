//! Generated crate synchronization and builds for server profiles.

mod build_versioning;
mod manifest;
mod napi_tool;
mod wasm_tool;

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::atomic::{AtomicU32, Ordering},
};

use miette::Diagnostic;
use thiserror::Error;

use crate::compiler::{CompileShimOpts, Compiler, Platform};

use self::manifest::ManifestOpts;
use super::write_file::write_file;

#[derive(Debug, Clone)]
pub struct BuildProfile {
    pub platform: Platform,
    pub out_dir: PathBuf,
    pub features: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub enum CraterToolOpts {
    Local { path: PathBuf, auto_install: bool },
}

#[derive(Debug, Clone)]
pub struct CraterToolsOps {
    pub napi: CraterToolOpts,
    pub wasm_bindgen: CraterToolOpts,
}

#[derive(Debug, Clone)]
pub struct SrcCrate {
    pub path: PathBuf,
    pub crate_name: String,
    pub package_name: String,
}

pub struct ServerCraterOpts {
    pub profiles: HashMap<String, BuildProfile>,
    pub tools: CraterToolsOps,
    pub src_crate: SrcCrate,
}

pub struct SyncOpts<'a> {
    pub profile: &'a str,
}

pub struct BuildOpts<'a> {
    pub release: bool,
    pub profile: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildArtifact {
    Wasm {
        wasm_abs_path: PathBuf,
        js_abs_path: PathBuf,
    },
    Node {
        js_abs_path: PathBuf,
        node_abs_path: PathBuf,
    },
}

pub struct ServerCrater {
    opts: ServerCraterOpts,
    next_version: AtomicU32,
}

impl ServerCrater {
    pub fn new(opts: ServerCraterOpts) -> Self {
        Self {
            opts,
            next_version: AtomicU32::new(0),
        }
    }

    pub fn sync_tools(&self) -> Result<(), CraterSyncToolsError> {
        self.needs_napi()
            .then(|| napi_tool::sync(&self.opts.tools.napi))
            .transpose()
            .map_err(|source| CraterSyncToolsError::Napi { source })?;
        self.needs_wasm()
            .then(|| wasm_tool::sync(&self.opts.tools.wasm_bindgen))
            .transpose()
            .map_err(|source| CraterSyncToolsError::Wasm { source })?;
        Ok(())
    }

    pub fn sync_crates(
        &self,
        compiler: &mut Compiler,
        opts: SyncOpts<'_>,
    ) -> Result<(), CraterSyncError> {
        let profile = self.profile(opts.profile)?;
        let compilation = compiler.compile_shim(CompileShimOpts {
            platform: profile.platform,
        });
        let rendered_manifest = manifest::render_manifest(
            profile.platform,
            ManifestOpts {
                name: opts.profile,
                source_crate_name: &self.opts.src_crate.crate_name,
                source_package_name: &self.opts.src_crate.package_name,
                source_package_path: &self.opts.src_crate.path,
                features: profile.features.as_deref(),
            },
        );

        let source_dir = profile.out_dir.join("src");
        write_generated_file(profile.out_dir.join("Cargo.toml"), rendered_manifest)?;
        write_generated_file(source_dir.join("lib.rs"), compilation.shim)?;

        if matches!(profile.platform, Platform::Node) {
            write_generated_file(
                profile.out_dir.join("build.rs"),
                "fn main() { napi_build::setup(); }\n",
            )?;
            napi_tool::ensure_project(&profile.out_dir)
                .map_err(|source| CraterSyncError::EnsureNapiProject { source })?;
        }

        Ok(())
    }

    pub fn build(&self, opts: BuildOpts<'_>) -> Result<BuildArtifact, CraterBuildError> {
        let profile = self.opts.profiles.get(opts.profile).ok_or_else(|| {
            CraterBuildError::UnknownProfile {
                profile: opts.profile.to_owned(),
            }
        })?;
        let version = self.next_version.fetch_add(1, Ordering::Relaxed);
        let output_root = profile.out_dir.join("output");
        let output_dir = build_versioning::versioned_dir(&output_root, version);

        let artifact = match profile.platform {
            Platform::Wasm { target } => {
                let tool = wasm_tool::locate(&self.opts.tools.wasm_bindgen).ok_or(
                    CraterBuildError::ToolNotLocated {
                        tool: "wasm-bindgen",
                    },
                )?;
                let output =
                    wasm_tool::build(&profile.out_dir, &output_dir, &tool, opts.release, target)
                        .map_err(|source| CraterBuildError::WasmTool { source })?;
                BuildArtifact::Wasm {
                    wasm_abs_path: output.wasm_abs_path,
                    js_abs_path: output.js_abs_path,
                }
            }
            Platform::Node => {
                let tool = napi_tool::locate(&self.opts.tools.napi)
                    .ok_or(CraterBuildError::ToolNotLocated { tool: "NAPI CLI" })?;
                let output = napi_tool::build(&profile.out_dir, &output_dir, &tool, opts.release)
                    .map_err(|source| CraterBuildError::NapiTool { source })?;
                BuildArtifact::Node {
                    js_abs_path: output.js_path,
                    node_abs_path: output.node_path,
                }
            }
        };

        build_versioning::cleanup(&output_root, version)
            .map_err(|source| CraterBuildError::BuildVersioning { source })?;
        Ok(artifact)
    }

    fn profile(&self, profile: &str) -> Result<&BuildProfile, CraterSyncError> {
        self.opts
            .profiles
            .get(profile)
            .ok_or_else(|| CraterSyncError::UnknownProfile {
                profile: profile.to_owned(),
            })
    }

    fn needs_napi(&self) -> bool {
        self.opts
            .profiles
            .values()
            .any(|profile| matches!(profile.platform, Platform::Node))
    }

    fn needs_wasm(&self) -> bool {
        self.opts
            .profiles
            .values()
            .any(|profile| matches!(profile.platform, Platform::Wasm { .. }))
    }
}

fn write_generated_file(path: PathBuf, contents: impl AsRef<[u8]>) -> Result<(), CraterSyncError> {
    write_file(&path, contents)
        .map(|_| ())
        .map_err(|source| CraterSyncError::WriteFile { path, source })
}

#[derive(Debug, Diagnostic, Error)]
pub enum CraterSyncError {
    #[error("unknown crater profile `{profile}`")]
    UnknownProfile { profile: String },
    #[error("failed to write generated file at {}", path.display())]
    WriteFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to prepare the generated NAPI project")]
    EnsureNapiProject {
        #[source]
        source: napi_tool::NapiToolError,
    },
}

#[derive(Debug, Diagnostic, Error)]
pub enum CraterSyncToolsError {
    #[error("failed to synchronize the NAPI tool")]
    Napi {
        #[source]
        source: napi_tool::NapiToolError,
    },
    #[error("failed to synchronize the Wasm tools")]
    Wasm {
        #[source]
        source: wasm_tool::WasmToolError,
    },
}

#[derive(Debug, Diagnostic, Error)]
pub enum CraterBuildError {
    #[error("unknown crater profile `{profile}`")]
    UnknownProfile { profile: String },
    #[error("{tool} could not be located")]
    ToolNotLocated { tool: &'static str },
    #[error("failed to use the NAPI tool")]
    NapiTool {
        #[source]
        source: napi_tool::NapiToolError,
    },
    #[error("failed to use the Wasm tool")]
    WasmTool {
        #[source]
        source: wasm_tool::WasmToolError,
    },
    #[error("failed to manage build output versions")]
    BuildVersioning {
        #[source]
        source: build_versioning::BuildVersioningError,
    },
}
