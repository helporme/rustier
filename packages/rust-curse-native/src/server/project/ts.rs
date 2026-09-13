use std::path::{Path, PathBuf};

use crate::{
    compiler::{db::Db, ir::decals::Decals},
    server::{
        project::{Project, ProjectError},
        write_file::write_file,
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
        let out_dir = self.project.out_dir();

        ProjectTsSync {
            decals,
            out_types_dir: out_dir.join("types"),
            out_ambient_file: out_dir.join("ambient.d.ts"),
            root_dir: self.project.root_dir(),
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
    pub fn exec(&self, db: &'db dyn Db) -> Result<(), ProjectError> {
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
                    .ok_or_else(|| ProjectError::ModuleOutsideRoot {
                        path: path.as_path().to_owned(),
                        root: self.root_dir.to_owned(),
                    })?;
                Ok((module, relative))
            })
            .collect::<Result<Vec<_>, ProjectError>>()?;

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
        }

        write_file(
            &self.out_ambient_file,
            self.decals.render_ambient_ts_mods(db),
        )?;

        Ok(())
    }
}
