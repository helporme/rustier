use ra_ap_base_db::{
    Crate, CrateGraphBuilder, CrateOrigin, CrateWorkspaceData, DependencyBuilder, FileChange,
    SourceRoot,
};
use ra_ap_syntax::Edition;
use ra_ap_vfs::{AbsPathBuf, FileId, VfsPath, file_set::FileSet};
use triomphe::Arc;

use crate::{db::RootDatabase, ir::runtime::RuntimeAnalysis};

const RUNTIME: &str = r#"
    pub mod runtime {
        pub mod js {
            pub struct JsValue;
            pub struct JsEnv;
            impl JsValue {
                pub fn clone(&self) -> Self { loop {} }
                pub fn strict_eq(&self, other: &Self) -> bool { loop {} }
                pub fn discard(self) {}
            }
            impl JsEnv {
                pub fn new_number(&self, value: f64) -> JsValue { loop {} }
                pub fn new_string(&self, value: &str) -> JsValue { loop {} }
                pub fn null(&self) -> JsValue { loop {} }
                pub fn undefined(&self) -> JsValue { loop {} }
            }
        }
    }
"#;

pub(super) fn database() -> RootDatabase {
    let mut db = RootDatabase::default();
    let template = template(&mut db, "");
    RuntimeAnalysis::set(&mut db, template);

    db
}

pub(super) fn template(db: &mut RootDatabase, dependency: &str) -> Crate {
    let root_file = FileId::from_raw(0);
    let aux_file = FileId::from_raw(1);
    let dep_file = FileId::from_raw(2);
    let std_file = FileId::from_raw(3);
    let mut files = FileSet::default();
    files.insert(
        root_file,
        VfsPath::new_virtual_path("/analysis/lib.rs".into()),
    );
    files.insert(aux_file, VfsPath::new_virtual_path("/aux/lib.rs".into()));
    files.insert(dep_file, VfsPath::new_virtual_path("/dep/lib.rs".into()));
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
    let root = add_crate(root_file, local.clone());
    let aux = add_crate(aux_file, local.clone());
    let dep = add_crate(dep_file, local);
    let std = add_crate(
        std_file,
        CrateOrigin::Lang(ra_ap_base_db::LangCrateOrigin::Std),
    );

    for crate_ in [root, dep] {
        graph
            .add_dep(
                crate_,
                DependencyBuilder::new(ra_ap_base_db::CrateName::new("std").unwrap(), std),
            )
            .unwrap();
    }

    graph
        .add_dep(
            root,
            DependencyBuilder::new(ra_ap_base_db::CrateName::new("aux_next").unwrap(), aux),
        )
        .unwrap();
    graph
        .add_dep(
            root,
            DependencyBuilder::new(ra_ap_base_db::CrateName::new("dep").unwrap(), dep),
        )
        .unwrap();

    let mut change = FileChange::default();
    change.set_roots(vec![SourceRoot::new_local(files)]);
    change.set_crate_graph(graph);
    change.change_file(root_file, Some(String::new()));
    change.change_file(aux_file, Some(RUNTIME.into()));
    change.change_file(dep_file, Some(dependency.into()));
    change.change_file(std_file, Some(
        "pub mod string { pub struct String; impl String { pub fn new() -> Self { loop {} } } }".into()
    ));

    change.apply(db).unwrap()[&root]
}
