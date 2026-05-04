//! `meta.json` schema and persistence.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{Activation, Dep, ScaffoldError, Section};

const META_FILE: &str = "meta.json";

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub(crate) struct Meta {
    pub version: u32,
    #[serde(default)]
    pub sections: BTreeMap<String, MetaSection>,
    #[serde(default)]
    pub cargo_toml: CargoState,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub(crate) struct MetaSection {
    #[serde(default)]
    pub activation: Option<MetaActivation>,
    #[serde(default)]
    pub has_content: bool,
    #[serde(default)]
    pub has_build_content: bool,
    #[serde(default)]
    pub deps: Vec<MetaDep>,
    #[serde(default)]
    pub build_deps: Vec<MetaDep>,
    #[serde(default)]
    pub external_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum MetaActivation {
    CargoFeature {
        name: String,
    },
    ExclusiveEnv {
        env_var: String,
        cfg_flag: String,
    },
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub(crate) struct MetaDep {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub features: Vec<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub(crate) struct CargoState {
    #[serde(default)]
    pub lib_path_recorded: bool,
    #[serde(default)]
    pub build_recorded: bool,
    #[serde(default)]
    pub original_lib_path: Option<String>,
    #[serde(default)]
    pub original_package_build: Option<String>,
}

impl Meta {
    pub fn load(abs_temp_path: &Path) -> Result<Self, ScaffoldError> {
        let path = abs_temp_path.join(META_FILE);
        if !path.exists() {
            return Ok(Self {
                version: 1,
                ..Self::default()
            });
        }
        let content = fs::read_to_string(&path).map_err(|e| ScaffoldError::Io {
            path: path.clone(),
            source: e,
        })?;
        let meta: Meta = serde_json::from_str(&content)?;
        Ok(meta)
    }

    pub fn save(&self, abs_temp_path: &Path) -> Result<(), ScaffoldError> {
        let path = abs_temp_path.join(META_FILE);
        let content = serde_json::to_string_pretty(self)?;
        fs::write(&path, content).map_err(|e| ScaffoldError::Io { path, source: e })?;
        Ok(())
    }

    pub fn from_state(sections: &BTreeMap<String, Section>, cargo_toml: CargoState) -> Self {
        let meta_sections = sections
            .iter()
            .map(|(name, s)| (name.clone(), section_to_meta(s)))
            .collect();
        Self {
            version: 1,
            sections: meta_sections,
            cargo_toml,
        }
    }
}

fn section_to_meta(s: &Section) -> MetaSection {
    MetaSection {
        activation: s.activation.as_ref().map(|a| match a {
            Activation::CargoFeature(name) => MetaActivation::CargoFeature { name: name.clone() },
            Activation::ExclusiveEnv { env_var, cfg_flag } => MetaActivation::ExclusiveEnv {
                env_var: env_var.clone(),
                cfg_flag: cfg_flag.clone(),
            },
        }),
        has_content: s.content.is_some(),
        has_build_content: s.build_content.is_some(),
        deps: s.deps.iter().map(dep_to_meta).collect(),
        build_deps: s.build_deps.iter().map(dep_to_meta).collect(),
        external_files: s.external_files.clone(),
    }
}

fn dep_to_meta(d: &Dep) -> MetaDep {
    MetaDep {
        name: d.name.clone(),
        version: d.version.clone(),
        features: d.features.clone(),
    }
}
