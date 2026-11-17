use rustier_compiler::codegen::TransportBackend;
use target_lexicon::{Architecture, Environment, OperatingSystem, Triple};

use super::BuildError;

pub(super) struct Target {
    pub extension: &'static str,
    pub filename: &'static str,
    pub rustflags: Option<&'static str>,
    pub build_std: bool,
}

impl Target {
    pub fn parse(name: &str, backend: &TransportBackend) -> Result<Self, BuildError> {
        let triple: Triple =
            name.parse()
                .map_err(|error: target_lexicon::ParseError| BuildError::Target {
                    target: name.into(),
                    reason: error.to_string(),
                })?;
        let unsupported = |reason: &str| BuildError::Target {
            target: name.into(),
            reason: reason.into(),
        };

        match backend {
            TransportBackend::Wasm(wasm) => {
                let expected = if wasm.mem64 {
                    "wasm64-unknown-unknown"
                } else {
                    "wasm32-unknown-unknown"
                };
                if name != expected {
                    return Err(unsupported("target does not match the prepared Wasm glue"));
                }
                Ok(Self {
                    extension: "wasm",
                    filename: "bridge.wasm",
                    rustflags: None,
                    build_std: wasm.mem64,
                })
            }
            TransportBackend::Node(_) => {
                if !matches!(
                    triple.architecture,
                    Architecture::X86_64 | Architecture::Aarch64(_)
                ) {
                    return Err(unsupported(
                        "Node builds currently support x86_64 and aarch64",
                    ));
                }
                let (extension, rustflags) = match (triple.operating_system, triple.environment) {
                    (
                        OperatingSystem::Darwin(_) | OperatingSystem::MacOSX(_),
                        Environment::Unknown,
                    ) => (
                        "dylib",
                        Some(
                            "[\"-C\", \"link-arg=-undefined\", \"-C\", \"link-arg=dynamic_lookup\"]",
                        ),
                    ),
                    (OperatingSystem::Linux, Environment::Gnu) => ("so", None),
                    (OperatingSystem::Linux, Environment::Musl) => {
                        ("so", Some("[\"-C\", \"target-feature=-crt-static\"]"))
                    }
                    (OperatingSystem::Windows, Environment::Msvc) => ("dll", None),
                    _ => return Err(unsupported("unsupported Node target OS or ABI")),
                };
                Ok(Self {
                    extension,
                    filename: "bridge.node",
                    rustflags,
                    build_std: false,
                })
            }
        }
    }
}
