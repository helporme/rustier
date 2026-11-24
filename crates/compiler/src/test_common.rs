use ra_ap_base_db::{
    CrateGraphBuilder, CrateOrigin, CrateWorkspaceData, DependencyBuilder, FileChange, SourceRoot,
};
use ra_ap_syntax::Edition;
use ra_ap_vfs::{AbsPathBuf, FileId, VfsPath, file_set::FileSet};
use triomphe::Arc;

use crate::{db::RootDatabase, ir::rs::RsProgram};

const RUNTIME: &str = r#"
    pub use js::{JsEnv, JsValue};
    pub struct JsCallError;
    pub trait JsMacro {}
    pub enum JsValueType {}
    #[macro_export]
    macro_rules! __js_in_rs { () => { () }; }
    pub mod js {
        pub struct JsValue;
        pub struct JsEnv;
        impl JsValue {
            pub fn clone(&self) -> Self { loop {} }
            pub fn discard(self) {}
        }
    }
"#;

pub(super) fn database() -> (RootDatabase, RsProgram) {
    let (db, [rs]) = programs([""]);
    (db, rs)
}

pub(super) fn programs<const N: usize>(dependencies: [&str; N]) -> (RootDatabase, [RsProgram; N]) {
    let mut db = RootDatabase::default();
    let aux_file = FileId::from_raw(1);
    let std_file = FileId::from_raw(3);
    let mut files = FileSet::default();
    files.insert(aux_file, VfsPath::new_virtual_path("/aux/lib.rs".into()));
    files.insert(std_file, VfsPath::new_virtual_path("/std/lib.rs".into()));

    let mut graph = CrateGraphBuilder::default();
    let mut add_crate = |file, origin| {
        graph.add_crate_root(
            file,
            Edition::Edition2024,
            None,
            None,
            Default::default(),
            None,
            Default::default(),
            origin,
            vec![],
            false,
            Arc::new(AbsPathBuf::assert_utf8(if cfg!(windows) {
                "C:\\".into()
            } else {
                "/".into()
            })),
            Arc::new(CrateWorkspaceData {
                target: Err("test target".into()),
                toolchain: None,
            }),
        )
    };
    let local = CrateOrigin::Local {
        repo: None,
        name: None,
    };
    let aux = add_crate(aux_file, local.clone());
    let std = add_crate(
        std_file,
        CrateOrigin::Lang(ra_ap_base_db::LangCrateOrigin::Std),
    );

    let mut change = FileChange::default();
    let pairs: [_; N] = std::array::from_fn(|i| {
        let root_file = FileId::from_raw(4 + i as u32 * 2);
        let dep_file = FileId::from_raw(5 + i as u32 * 2);
        files.insert(
            root_file,
            VfsPath::new_virtual_path(format!("/analysis{i}/lib.rs")),
        );
        files.insert(
            dep_file,
            VfsPath::new_virtual_path(format!("/dep{i}/lib.rs")),
        );
        change.change_file(root_file, Some(String::new()));
        change.change_file(dep_file, Some(dependencies[i].into()));
        (
            add_crate(root_file, local.clone()),
            add_crate(dep_file, local.clone()),
        )
    });
    for (root, dep) in pairs {
        for crate_ in [root, dep] {
            graph
                .add_dep(
                    crate_,
                    DependencyBuilder::new(ra_ap_base_db::CrateName::new("std").unwrap(), std),
                )
                .unwrap();
        }

        for name in ["rustier", "rustier_runtime", "rustier_js_proc"] {
            graph
                .add_dep(
                    root,
                    DependencyBuilder::new(ra_ap_base_db::CrateName::new(name).unwrap(), aux),
                )
                .unwrap();
        }
        graph
            .add_dep(
                root,
                DependencyBuilder::new(ra_ap_base_db::CrateName::new("dep").unwrap(), dep),
            )
            .unwrap();
    }
    change.set_roots(vec![SourceRoot::new_local(files)]);
    change.set_crate_graph(graph);
    change.change_file(aux_file, Some(RUNTIME.into()));
    change.change_file(
        std_file,
        Some(
            "pub mod string { pub struct String; impl String { pub fn new() -> Self { loop {} } } }
             pub mod result { pub enum Result<T, E> { Ok(T), Err(E) } }"
                .into(),
        ),
    );

    let crates = change.apply(&mut db).unwrap();
    let programs = pairs.map(|(root, _)| RsProgram::new(&db, crates[&root]));
    (db, programs)
}
