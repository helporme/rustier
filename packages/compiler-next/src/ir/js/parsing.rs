//! Parsing of JavaScript source files, with help of oxc.

use std::collections::HashSet;

use oxc_allocator::Allocator;
use oxc_ast::{
    AstKind,
    ast::{Expression, ImportDeclarationSpecifier, ImportOrExportKind, ModuleExportName},
};
use oxc_diagnostics::OxcDiagnostic;
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::SourceType;

use crate::{
    db::Db,
    diag::{Diag, DiagAccumulateExt as _},
    ir::js::{JsFile, JsFileRange, JsModule, JsPart, RsMacroSrc},
};

/// A parser diagnostic.
#[derive(Clone, Debug, thiserror::Error, miette::Diagnostic)]
#[error("{message}")]
#[diagnostic(code(rustier::javascript))]
struct JsDiagnostic {
    /// The diagnostic's human-readable description.
    message: String,
    /// The named source snapshot used to render labeled excerpts.
    #[source_code]
    source_code: miette::NamedSource<String>,
    /// Labeled UTF-8 byte spans within the source snapshot.
    #[label(collection)]
    labels: Vec<miette::LabeledSpan>,
}

impl JsDiagnostic {
    /// Copies an Oxc diagnostic and its source into an owned IR diagnostic.
    fn from_oxc(db: &dyn Db, file: JsFile, error: OxcDiagnostic) -> Self {
        Self {
            message: error.message.to_string(),
            source_code: miette::NamedSource::new(
                file.module_id(db).clone(),
                file.contents(db).clone(),
            )
            .with_language("JavaScript"),
            labels: error
                .labels
                .iter()
                .map(|label| {
                    miette::LabeledSpan::new(
                        label.label().map(str::to_owned),
                        label.offset() as usize,
                        label.len() as usize,
                    )
                })
                .collect(),
        }
    }
}

impl JsFile {
    /// Parses this snapshot and resolves imported `rs` tags, respecting shadowing.
    ///
    /// Parsing or semantic errors are accumulated as diagnostics and yield a
    /// module with no snippet occurrences.
    pub fn module<'db>(self, db: &'db dyn Db) -> JsModule<'db> {
        module(db, self)
    }

    /// Diagnostics accumulated by this snapshot's parsing query.
    pub fn diagnostics(self, db: &dyn Db) -> Vec<&Diag> {
        module::accumulated::<Diag>(db, self)
    }
}

/// Parses one immutable file snapshot and records raw template and expression ranges.
#[salsa::tracked]
fn module<'db>(db: &'db dyn Db, file: JsFile) -> JsModule<'db> {
    let source = file.contents(db);
    let module_id = file.module_id(db);
    let path = module_id.split(['?', '#']).next().unwrap();
    let source_type = SourceType::from_path(path).unwrap_or_else(|_| SourceType::mjs());
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, source_type).parse();
    if !parsed.diagnostics.is_empty() {
        for error in parsed.diagnostics {
            JsDiagnostic::from_oxc(db, file, error).accumulate(db);
        }

        return JsModule::new(db, file, Vec::new());
    }

    let analyzed = SemanticBuilder::new()
        .with_build_nodes(true)
        .with_check_syntax_error(true)
        .build(&parsed.program);
    if !analyzed.diagnostics.is_empty() {
        for error in analyzed.diagnostics {
            JsDiagnostic::from_oxc(db, file, error).accumulate(db);
        }

        return JsModule::new(db, file, Vec::new());
    }

    let semantic = analyzed.semantic;
    let mut bindings = HashSet::new();
    for node in semantic.nodes().iter() {
        let AstKind::ImportDeclaration(import) = node.kind() else {
            continue;
        };
        if !matches!(import.source.value.as_str(), "rustier" | "rustier-next")
            || import.import_kind == ImportOrExportKind::Type
        {
            continue;
        }
        for specifier in import.specifiers.iter().flatten() {
            let ImportDeclarationSpecifier::ImportSpecifier(specifier) = specifier else {
                continue;
            };
            let imported = match &specifier.imported {
                ModuleExportName::IdentifierName(name) => name.name.as_str(),
                ModuleExportName::StringLiteral(name) => name.value.as_str(),
                ModuleExportName::IdentifierReference(_) => unreachable!(),
            };
            if imported == "rs" && specifier.import_kind != ImportOrExportKind::Type {
                bindings.insert(specifier.local.symbol_id.get().unwrap());
            }
        }
    }

    let mut templates = Vec::new();
    for node in semantic.nodes().iter() {
        let AstKind::TaggedTemplateExpression(template) = node.kind() else {
            continue;
        };
        let Expression::Identifier(tag) = template.tag.get_inner_expression() else {
            continue;
        };
        let reference = semantic
            .scoping()
            .get_reference(tag.reference_id.get().unwrap());
        if reference
            .symbol_id()
            .is_some_and(|id| bindings.contains(&id))
        {
            templates.push(template);
        }
    }
    templates.sort_by_key(|template| template.span.start);

    let macros: Vec<_> = templates
        .into_iter()
        .map(|template| {
            let mut body = Vec::new();
            for (index, quasi) in template.quasi.quasis.iter().enumerate() {
                body.push(JsPart::Text(JsFileRange::new(
                    db,
                    file,
                    quasi.span.start,
                    quasi.span.end,
                )));
                if let Some(next) = template.quasi.quasis.get(index + 1) {
                    body.push(JsPart::Interpolation {
                        range: JsFileRange::new(db, file, quasi.span.end, next.span.start),
                        expression: JsFileRange::new(
                            db,
                            file,
                            quasi.span.end + 2,
                            next.span.start - 1,
                        ),
                    });
                }
            }

            RsMacroSrc::new(
                db,
                JsFileRange::new(db, file, template.span.start, template.span.end),
                body,
            )
        })
        .collect();

    JsModule::new(db, file, macros)
}
