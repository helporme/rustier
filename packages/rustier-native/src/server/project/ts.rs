use std::{
    collections::HashSet,
    fs, io,
    path::{Path, PathBuf},
};

use crate::{
    compiler::{db::Db, ir::decals::Decals},
    server::{
        project::Project,
        write_file::{WriteFileError, write_file},
    },
};

/// A project's TypeScript definition.
#[derive(Debug, Clone, Copy)]
pub struct ProjectTs<'a> {
    project: Project<'a>,
}

impl<'a> ProjectTs<'a> {
    pub(super) fn new(project: Project<'a>) -> Self {
        Self { project }
    }

    /// Synchronization the project's TypeScript declarations to the file
    /// system.
    ///
    /// Doesn't do anything until executed.
    pub fn sync(self, db: &'a dyn Db) -> ProjectTsSync<'a> {
        let decals = self.project.reachable_shim(db).decals(db);
        let out_dir = self.project.out_dir(db);

        ProjectTsSync {
            decals,
            out_types_dir: out_dir.join("types"),
            out_ambient_file: out_dir.join("ambient.d.ts"),
            root_dir: self.project.root_dir(db),
        }
    }
}

/// Synchronization of the project's TypeScript declarations.
pub struct ProjectTsSync<'a> {
    decals: Decals<'a>,
    out_types_dir: PathBuf,
    out_ambient_file: PathBuf,
    root_dir: &'a Path,
}

impl<'db> ProjectTsSync<'db> {
    /// Syncs the decals IR to the file system.
    pub fn exec(&self, db: &'db dyn Db) -> Result<(), ProjectTsError> {
        // Validate every local module before writing any output.
        let modules = self
            .decals
            .local_modules(db)
            .iter()
            .map(|module| {
                let path = module.file_path(db);
                let relative = path
                    .as_path()
                    .strip_prefix(self.root_dir)
                    .ok()
                    .filter(|_| path.is_real())
                    .ok_or_else(|| ProjectTsError::ModuleOutsideRoot {
                        path: path.as_path().to_owned(),
                        root: self.root_dir.to_owned(),
                    })?;
                Ok((module, relative))
            })
            .collect::<Result<Vec<_>, ProjectTsError>>()?;

        let mut generated = HashSet::new();
        for (module, relative) in modules {
            let Some(file_name) = relative.file_name() else {
                continue;
            };

            let declaration_name = format!("{}.d.ts", file_name.to_string_lossy());
            let declaration_path = self
                .out_types_dir
                .join(relative)
                .with_file_name(&declaration_name);

            let sourcemap_name = format!("{declaration_name}.map");
            let sourcemap_path = declaration_path.with_file_name(&sourcemap_name);

            // FIXME: move to the codegen?
            let declaration = format!(
                "{}\n//# sourceMappingURL={sourcemap_name}\n",
                module.render_ts_mod(db)
            );

            write_file(&sourcemap_path, module.render_sourcemap(db))?;

            write_file(&declaration_path, declaration)?;
            generated.extend([declaration_path, sourcemap_path]);
        }

        prune_declarations(&self.out_types_dir, &generated).map_err(|source| {
            ProjectTsError::PruneDeclarations {
                path: self.out_types_dir.clone(),
                source,
            }
        })?;
        write_file(
            &self.out_ambient_file,
            self.decals.render_ambient_ts_mods(db),
        )?;

        Ok(())
    }
}

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum ProjectTsError {
    #[error(transparent)]
    #[diagnostic(transparent)]
    WriteFile(#[from] WriteFileError),
    #[error("failed to remove stale declarations in {}", path.display())]
    #[diagnostic(code(rustier::artifact_io))]
    PruneDeclarations {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("local module {} is outside project root {}", path.display(), root.display())]
    #[diagnostic(code(rustier::module_outside_project))]
    ModuleOutsideRoot { path: PathBuf, root: PathBuf },
}

/// A different declaration target may no longer expose previously emitted
/// modules.
fn prune_declarations(root: &Path, generated: &HashSet<PathBuf>) -> io::Result<()> {
    if fs::symlink_metadata(root).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return Ok(());
    }
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let kind = entry.file_type()?;
        if kind.is_dir() {
            prune_declarations(&path, generated)?;
        } else if kind.is_file()
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".rs.d.ts") || name.ends_with(".rs.d.ts.map"))
            && !generated.contains(&path)
        {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}
