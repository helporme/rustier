//! Build generated glue crate.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    process::ExitStatus,
    sync::atomic::{AtomicUsize, Ordering},
    thread,
};

use cargo_metadata::{CrateType, Message, diagnostic::DiagnosticLevel};
use cargo_options::{Build, CommonOptions};
use rustier_compiler::{
    codegen::{
        TransportBackend,
        manifest::{Dependency, render_manifest},
    },
    db::Db,
    ir::js::JsProgram,
};

mod target;

/// Settings for preparing source shared by builds of the same transport.
pub struct GlueOpts {
    pub out_dir: PathBuf,
    pub backend: TransportBackend,
    pub dependency_crates: BTreeMap<String, Dependency>,
}

/// Prepared source. Do not rewrite it until all builds using it have finished.
pub struct GlueCrate {
    out_dir: PathBuf,
    directory: PathBuf,
    backend: TransportBackend,
}

/// Resolved settings for one target; environment overrides apply only to its Cargo process.
pub struct TargetBuild {
    pub target: String,
    pub features: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub release: bool,
}

pub struct BuildRequest<'a> {
    pub glue: &'a GlueCrate,
    pub target: TargetBuild,
}

pub struct BatchOpts {
    pub iteration: u32,
    pub concurrency: NonZeroUsize,
}

#[derive(Debug)]
pub struct BuildOutcome {
    pub target: String,
    pub result: Result<Artifact, BuildError>,
}

pub struct BuildOpts {
    /// Directory for the generated crate, Cargo cache, and artifact iterations.
    pub out_dir: PathBuf,
    pub backend: TransportBackend,
    /// Dependencies from local paths or registry versions.
    pub dependency_crates: BTreeMap<String, Dependency>,
    /// Cargo features to enable, such as `my_crate/feature`. Defaults remain enabled.
    pub features: Vec<String>,
    pub release: bool,
    /// A fresh artifact directory number. Existing iterations are never overwritten.
    pub iteration: u32,
}

#[derive(Debug)]
pub struct Artifact {
    pub target: String,
    pub binary_path: PathBuf,
    pub warnings: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("Cargo build failed ({status}):\n{stderr}")]
    Cargo { status: ExitStatus, stderr: String },
    #[error("Cargo succeeded without reporting the glue binary")]
    MissingArtifact,
    #[error("unsupported target {target}: {reason}")]
    Target { target: String, reason: String },
    #[error("cannot determine the host Rust target: {0}")]
    Host(#[from] rustc_version::Error),
    #[error("duplicate target {0} in the same output directory")]
    DuplicateTarget(String),
}

/// Writes and builds a glue crate.
pub fn build(db: &dyn Db, program: JsProgram, opts: BuildOpts) -> Result<Artifact, BuildError> {
    let target = match &opts.backend {
        TransportBackend::Node(_) => rustc_version::version_meta()?.host,
        TransportBackend::Wasm(wasm) => if wasm.mem64 {
            "wasm64-unknown-unknown"
        } else {
            "wasm32-unknown-unknown"
        }
        .into(),
    };
    let glue = write_glue_crate(
        db,
        program,
        GlueOpts {
            out_dir: opts.out_dir,
            backend: opts.backend,
            dependency_crates: opts.dependency_crates,
        },
    )?;
    build_glue_crate(
        &glue,
        &TargetBuild {
            target,
            features: opts.features,
            env: BTreeMap::new(),
            release: opts.release,
        },
        opts.iteration,
    )
}

/// Removes the specified artifact iteration.
pub fn remove_build_iteration(out_dir: &Path, iteration: u32) -> io::Result<()> {
    match fs::remove_dir_all(out_dir.join("artifacts").join(iteration.to_string())) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}

/// Generates source once. Node targets share `node/`; Wasm pointer widths have separate crates.
pub fn write_glue_crate(db: &dyn Db, program: JsProgram, opts: GlueOpts) -> io::Result<GlueCrate> {
    let mut dependencies = BTreeMap::new();
    for (name, dependency) in &opts.dependency_crates {
        let mut dependency = dependency.clone();
        if let Dependency::Path { path } = &mut dependency {
            *path = fs::canonicalize(&*path)?
                .into_os_string()
                .into_string()
                .map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidInput, "crate paths must be UTF-8")
                })?;
        }
        dependencies.insert(name.clone(), dependency);
    }

    let manifest = render_manifest(&dependencies);
    let source = program.runtime(db).render_rs_ffi(db, &opts.backend);
    fs::create_dir_all(&opts.out_dir)?;
    let out_dir = fs::canonicalize(&opts.out_dir)?;
    let directory = out_dir.join(match &opts.backend {
        TransportBackend::Node(_) => "node",
        TransportBackend::Wasm(wasm) => {
            if wasm.mem64 {
                "wasm64"
            } else {
                "wasm32"
            }
        }
    });
    fs::create_dir_all(&directory)?;
    write_if_changed(&directory.join("lib.rs"), source.as_bytes())?;
    write_if_changed(&directory.join("Cargo.toml"), manifest.as_bytes())?;

    Ok(GlueCrate {
        out_dir,
        directory,
        backend: opts.backend,
    })
}

/// Compiles prepared source without accessing the database or rewriting the crate.
pub fn build_glue_crate(
    glue: &GlueCrate,
    opts: &TargetBuild,
    iteration: u32,
) -> Result<Artifact, BuildError> {
    let target = target::Target::parse(&opts.target, &glue.backend)?;
    let source_path = glue.directory.join("lib.rs");

    let mut cargo = Build {
        manifest_path: Some(glue.directory.join("Cargo.toml")),
        release: opts.release,
        common: CommonOptions {
            target: vec![opts.target.clone()],
            target_dir: Some(glue.out_dir.join("cargo").join(&opts.target)),
            features: opts.features.clone(),
            message_format: vec!["json".into()],
            color: Some("never".into()),
            ..Default::default()
        },
        ..Default::default()
    };

    if let Some(flags) = target.rustflags {
        cargo
            .config
            .push(format!("target.{}.rustflags={flags}", opts.target));
    }
    if target.build_std {
        cargo
            .unstable_flags
            .push("build-std=std,panic_abort".into());
    }

    let output = cargo.command().envs(&opts.env).output()?;
    let mut binary = None;
    let mut warnings = Vec::new();
    let mut diagnostics = String::new();
    for message in Message::parse_stream(output.stdout.as_slice()) {
        match message? {
            Message::CompilerArtifact(artifact)
                if artifact.target.src_path.as_std_path() == source_path
                    && artifact.target.crate_types.contains(&CrateType::CDyLib) =>
            {
                binary = artifact
                    .filenames
                    .into_iter()
                    .find(|path| path.extension() == Some(target.extension));
            }
            Message::CompilerMessage(message) => {
                let rendered = message.message.rendered.unwrap_or(message.message.message);
                diagnostics.push_str(&rendered);
                diagnostics.push('\n');
                if message.message.level == DiagnosticLevel::Warning {
                    warnings.push(rendered);
                }
            }
            _ => {}
        }
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        diagnostics.push_str(&stderr);
        return Err(BuildError::Cargo {
            status: output.status,
            stderr: diagnostics,
        });
    }
    if stderr.lines().any(|line| line.starts_with("warning:")) {
        warnings.push(stderr.trim().to_owned());
    }

    let binary = binary.ok_or(BuildError::MissingArtifact)?;
    let artifacts = glue.out_dir.join("artifacts").join(iteration.to_string());
    fs::create_dir_all(&artifacts)?;
    let target_directory = artifacts.join(&opts.target);
    fs::create_dir(&target_directory)?;
    let binary_path = target_directory.join(target.filename);
    fs::copy(binary, &binary_path)?;

    Ok(Artifact {
        target: opts.target.clone(),
        binary_path,
        warnings,
    })
}

/// Builds at most `concurrency` targets simultaneously and returns outcomes in input order.
/// A failed target does not cancel other targets or delete successful artifacts.
pub fn build_glue_crates(requests: &[BuildRequest<'_>], opts: BatchOpts) -> Vec<BuildOutcome> {
    let mut seen = BTreeSet::new();
    let unique: Vec<_> = requests
        .iter()
        .map(|request| seen.insert((&request.glue.out_dir, &request.target.target)))
        .collect();
    let next = AtomicUsize::new(0);
    let mut outcomes: Vec<_> = (0..requests.len()).map(|_| None).collect();

    thread::scope(|scope| {
        let workers: Vec<_> = (0..opts.concurrency.get().min(requests.len()))
            .map(|_| {
                let unique = &unique;
                let next = &next;
                scope.spawn(move || {
                    let mut results = Vec::new();
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        let Some(request) = requests.get(index) else {
                            break;
                        };
                        let result = if unique[index] {
                            build_glue_crate(request.glue, &request.target, opts.iteration)
                        } else {
                            Err(BuildError::DuplicateTarget(request.target.target.clone()))
                        };
                        results.push((
                            index,
                            BuildOutcome {
                                target: request.target.target.clone(),
                                result,
                            },
                        ));
                    }
                    results
                })
            })
            .collect();

        for worker in workers {
            let results = worker
                .join()
                .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
            for (index, outcome) in results {
                outcomes[index] = Some(outcome);
            }
        }
    });

    outcomes
        .into_iter()
        .map(|outcome| outcome.expect("every request has an outcome"))
        .collect()
}

fn write_if_changed(path: &Path, contents: &[u8]) -> io::Result<()> {
    match fs::read(path) {
        Ok(previous) if previous == contents => Ok(()),
        Ok(_) => fs::write(path, contents),
        Err(error) if error.kind() == io::ErrorKind::NotFound => fs::write(path, contents),
        Err(error) => Err(error),
    }
}
