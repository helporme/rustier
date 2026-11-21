use std::{collections::BTreeMap, ffi::OsString, io, process::Command};

use cargo_options::Build;

use super::{BuildError, TargetBuild};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BuildDriver {
    Cargo,
    #[default]
    Auto,
    Zigbuild,
    Xwin,
}

impl BuildDriver {
    fn resolve(self, target: &str, host: &str) -> Result<Self, BuildError> {
        let linux = target.contains("-linux-");
        let darwin = target.ends_with("-apple-darwin");
        let msvc = target.ends_with("-windows-msvc");
        match self {
            Self::Auto if linux && target != host => Ok(Self::Zigbuild),
            Self::Auto if msvc && !host.ends_with("-windows-msvc") => Ok(Self::Xwin),
            Self::Auto => Ok(Self::Cargo),
            Self::Zigbuild if !linux && !darwin => Err(BuildError::Target {
                target: target.into(),
                reason: "zigbuild requires a Linux or Darwin target".into(),
            }),
            Self::Xwin if !msvc => Err(BuildError::Target {
                target: target.into(),
                reason: "xwin requires a Windows MSVC target".into(),
            }),
            driver => Ok(driver),
        }
    }

    pub(super) fn command(self, opts: &TargetBuild, build: &Build) -> Result<Command, BuildError> {
        let cargo_build = build.command();
        let mut command = cargo(opts);
        match self.resolve(&opts.target, &target_lexicon::HOST.to_string())? {
            Self::Cargo => command.arg("build"),
            Self::Zigbuild => command.arg("zigbuild"),
            Self::Xwin => command.args(["xwin", "build"]),
            Self::Auto => unreachable!(),
        };
        command.args(cargo_build.get_args().skip(1));
        Ok(command)
    }
}

fn tool(opts: &TargetBuild, key: &str, default: &str) -> OsString {
    opts.env
        .get(key)
        .map(OsString::from)
        .or_else(|| std::env::var_os(key))
        .unwrap_or_else(|| default.into())
}

fn cargo(opts: &TargetBuild) -> Command {
    let mut command = Command::new(tool(opts, "CARGO", "cargo"));
    command.envs(&opts.env).env_remove("CARGO");
    command
}

/// Check the selected cross-build tools before loading analysis workspaces.
pub fn check_build_drivers(targets: &[TargetBuild]) -> Result<(), BuildError> {
    let host = target_lexicon::HOST.to_string();
    let mut failures = BTreeMap::<String, Vec<&str>>::new();
    let mut checked = BTreeMap::new();
    for opts in targets {
        let driver = opts.build_driver.resolve(&opts.target, &host)?;
        let (subcommand, hint) = match driver {
            BuildDriver::Cargo => continue,
            BuildDriver::Zigbuild => (
                "zigbuild",
                "install cargo-zigbuild with `cargo install --locked cargo-zigbuild`",
            ),
            BuildDriver::Xwin => (
                "xwin",
                "review cargo-xwin's SDK license and install cargo-xwin and LLVM",
            ),
            BuildDriver::Auto => unreachable!(),
        };
        let mut probe = |name, cmd: &mut Command, hint| {
            let available = checked
                .entry((name, &opts.env))
                .or_insert_with(|| cmd.output().is_ok_and(|output| output.status.success()));
            if !*available {
                failures
                    .entry(format!("{name} unavailable: {hint}"))
                    .or_default()
                    .push(&opts.target);
            }
        };
        probe(
            subcommand,
            cargo(opts).args([subcommand, "--help"]),
            hint,
        );
        if driver == BuildDriver::Zigbuild {
            probe(
                "zig",
                Command::new(tool(opts, "CARGO_ZIGBUILD_ZIG_PATH", "zig"))
                    .envs(&opts.env)
                    .arg("version"),
                "install Zig or set CARGO_ZIGBUILD_ZIG_PATH",
            );
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(io::Error::other(
            failures
                .into_iter()
                .map(|(why, targets)| format!("{}: {why}", targets.join(", ")))
                .collect::<Vec<_>>()
                .join("\n"),
        )
        .into())
    }
}
