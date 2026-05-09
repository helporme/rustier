use std::path::Path;

use napi::{bindgen_prelude::Promise, threadsafe_function::ThreadsafeFunction};
use napi_derive::napi;

use crate::sync::write_napi::NAPI_CARGO_FEATURE;

pub struct NapiBuildOpts<'a> {
    pub release: bool,
    pub output_abs_dir: &'a Path,
    pub project_root_abs_path: &'a Path,
    pub profile: &'a String,
}

#[derive(Debug, Clone)]
pub struct NapiBuildOutput {
    pub js_output_abs_path: Option<String>,
    pub ts_output_abs_path: Option<String>,
    pub node_output_abs_paths: Vec<String>,
}

#[cfg_attr(feature = "napi-binding", napi(object))]
#[derive(Debug)]
pub struct NapiBuildArgs {
    pub features: Vec<String>,
    pub release: bool,
    pub output_abs_dir: String,
    pub target_abs_dir: String,
    pub manifest_abs_path: String,
}

#[cfg_attr(feature = "napi-binding", napi(string_enum = "lowercase"))]
#[derive(Debug, Clone)]
pub enum NapiBuildArtifactKind {
    Js,
    Dts,
    Node,
    Exe,
    Wasm,
}

#[cfg_attr(feature = "napi-binding", napi(object))]
#[derive(Debug, Clone)]
pub struct NapiBuildAritfact {
    pub kind: NapiBuildArtifactKind,
    pub path: String,
}

/// Napi build function passed in from JS.
//
// Passing a function in from JS is a compromise that lets us run the original
// @napi/cli JS library without doing any of the following:
//  - Reimplementing @napi/cli in Rust
//  - Running @napi/cli as a subprocess by resolving the binary path,
//    implementing cross-platform bin location (e.g. handling .bat/.ps1 files on
//    Windows), finding the correct node_modules, etc.
//  - Bundling @napi/cli and a JS runtime into a single binary
//  - Splitting the build step across JS and Rust code
//
// By passing the JS function into Rust, we sidestep all of the problems above
// and can run the original @napi/cli library as-is by just depending on it.
pub type NapiBuildFn = ThreadsafeFunction<
    NapiBuildArgs,
    Promise<Vec<NapiBuildAritfact>>,
    NapiBuildArgs,
    napi::Status,
    true, // Pass error as first arg to js fn
    true, // Weak reference to the JS function, do not keep the js alive just because of it
>;

/// Napi builder utility
pub struct NapiBuilder(NapiBuildFn);

impl From<NapiBuildFn> for NapiBuilder {
    fn from(_fn: NapiBuildFn) -> Self {
        Self(_fn)
    }
}

impl NapiBuilder {
    pub async fn build(&self, opts: NapiBuildOpts<'_>) -> Result<NapiBuildOutput, napi::Error> {
        let profile_output_abs_dir = opts.output_abs_dir.join(opts.profile);
        let output_abs_dir = profile_output_abs_dir
            .join("package")
            .to_string_lossy()
            .to_string();

        let target_abs_dir = profile_output_abs_dir
            .join("target")
            .to_string_lossy()
            .to_string();

        let manifest_abs_path = opts
            .project_root_abs_path
            .join("Cargo.toml")
            .to_string_lossy()
            .to_string();

        let features = vec![NAPI_CARGO_FEATURE.to_string(), opts.profile.to_string()];

        let args = NapiBuildArgs {
            release: opts.release,
            output_abs_dir,
            target_abs_dir,
            manifest_abs_path,
            features,
        };

        let promise = self.0.call_async(Ok(args)).await?;
        let artifacts = promise.await?;

        let js_output_abs_path: Option<String> = artifacts
            .iter()
            .find(|a| matches!(a.kind, NapiBuildArtifactKind::Js))
            .map(|a| a.path.clone());

        let ts_output_abs_path: Option<String> = artifacts
            .iter()
            .find(|a| matches!(a.kind, NapiBuildArtifactKind::Dts))
            .map(|a| a.path.clone());

        let node_output_abs_paths: Vec<String> = artifacts
            .iter()
            .filter_map(|a| {
                if matches!(a.kind, NapiBuildArtifactKind::Node) {
                    Some(a.path.clone())
                } else {
                    None
                }
            })
            .collect();

        Ok(NapiBuildOutput {
            js_output_abs_path,
            ts_output_abs_path,
            node_output_abs_paths,
        })
    }
}
