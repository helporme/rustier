use std::{collections::HashSet, fmt::Write};

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
    ir::{JsDiagnostic, JsFile, Macro, Part, SourceRange, Type},
};

impl From<OxcDiagnostic> for JsDiagnostic {
    fn from(error: OxcDiagnostic) -> Self {
        let range = error.labels.iter().next().map(|label| SourceRange {
            start: label.offset(),
            end: (label.offset() + label.len()),
        });

        Self {
            message: error.message.to_string(),
            range,
        }
    }
}

impl JsFile {
    pub fn macros<'db>(self, db: &'db dyn Db) -> &'db Result<Vec<Macro<'db>>, Vec<JsDiagnostic>> {
        macros(db, self)
    }
}

#[salsa::tracked(returns(ref))]
fn macros<'db>(db: &'db dyn Db, file: JsFile) -> Result<Vec<Macro<'db>>, Vec<JsDiagnostic>> {
    let source = file.contents(db);
    let module_id = file.module_id(db);
    let path = module_id.split(['?', '#']).next().unwrap();
    let source_type = SourceType::from_path(path).unwrap_or_else(|_| SourceType::mjs());
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, source_type).parse();
    if !parsed.diagnostics.is_empty() {
        return Err(parsed.diagnostics.into_iter().map(Into::into).collect());
    }

    let analyzed = SemanticBuilder::new()
        .with_build_nodes(true)
        .with_check_syntax_error(true)
        .build(&parsed.program);
    if !analyzed.diagnostics.is_empty() {
        return Err(analyzed.diagnostics.into_iter().map(Into::into).collect());
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

    let mut symbol = String::from("__rustier_");
    for byte in module_id.bytes() {
        write!(symbol, "{byte:02x}").unwrap();
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

    let macros = templates
        .into_iter()
        .enumerate()
        .map(|(index, template)| {
            let mut body = Vec::new();
            for (index, quasi) in template.quasi.quasis.iter().enumerate() {
                body.push(Part::Text(
                    source[quasi.span.start as usize..quasi.span.end as usize].to_owned(),
                ));
                if let Some(next) = template.quasi.quasis.get(index + 1) {
                    body.push(Part::Interpolation {
                        type_: Type::JsValue,
                        range: SourceRange {
                            start: quasi.span.end,
                            end: next.span.start,
                        },
                        expression: SourceRange {
                            start: quasi.span.end + 2,
                            end: next.span.start - 1,
                        },
                    });
                }
            }

            Macro::new(
                db,
                file,
                SourceRange {
                    start: template.span.start,
                    end: template.span.end,
                },
                format!("{symbol}_{index}"),
                Type::JsValue,
                body,
            )
        })
        .collect();

    Ok(macros)
}
