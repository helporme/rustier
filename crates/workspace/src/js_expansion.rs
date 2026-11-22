//! Preserve JavaScript return types when the proc-macro server has no line information.

use std::sync::Arc;

use ra_ap_base_db::{CrateGraphBuilder, Env, ProcMacroPaths, SourceDatabase};
use ra_ap_hir_expand::{
    change::ChangeWithProcMacros,
    proc_macro::{ProcMacroExpander, ProcMacroExpansionError},
    resolve_span,
    tt::{Leaf, LitKind, Literal, Span, TopSubtree, TopSubtreeBuilder, TtElement},
};
use ra_ap_proc_macro_api::ProcMacroClient;
use salsa::Setter;

/// Decorates Rustier's two real expanders, retaining the other loaded macros unchanged.
pub fn install(
    db: &mut impl SourceDatabase,
    server: Option<&ProcMacroClient>,
    graph: &CrateGraphBuilder,
    paths: ProcMacroPaths,
) {
    let Some(server) = server else { return };
    let mut replacements = Vec::new();
    for (id, entry) in paths {
        let Ok((name, path)) = entry else { continue };
        if name != "rustier_js_proc" {
            continue;
        }
        let Ok(mut macros) = ra_ap_load_cargo::load_proc_macro(server, &path, &[]) else {
            continue;
        };
        for mac in &mut macros {
            let deferred = match mac.name.as_str() {
                "__expand_js" => true,
                "__js_in_rs" => false,
                _ => continue,
            };
            mac.expander = Arc::new(JsExpander {
                inner: mac.expander.clone(),
                deferred,
            });
        }
        replacements.push((id, Ok(macros)));
    }
    if replacements.is_empty() {
        return;
    }

    let registry = ra_ap_hir::ProcMacros::get(db);
    let mut loaded = registry.by_crate(db).clone();
    let mut change = ChangeWithProcMacros::default();
    change.set_crate_graph(graph.clone());
    change.set_proc_macros(replacements.into_iter().collect());
    change.apply(db);
    loaded.extend(registry.by_crate(db).clone());
    registry.set_by_crate(db).to(loaded);
}

#[derive(Debug)]
struct JsExpander {
    inner: Arc<dyn ProcMacroExpander>,
    deferred: bool,
}

impl ProcMacroExpander for JsExpander {
    fn expand(
        &self,
        db: &dyn SourceDatabase,
        input: &TopSubtree,
        attrs: Option<&TopSubtree>,
        env: &Env,
        def_site: Span,
        call_site: Span,
        mixed_site: Span,
        current_dir: String,
    ) -> Result<TopSubtree, ProcMacroExpansionError> {
        let mut tokens = input.iter();
        let start = tokens.savepoint();
        if self.deferred {
            tokens.next();
        } // `$crate` precedes the JavaScript tokens.
        let prefix = tokens.from_savepoint(start);
        let body = tokens.remaining();
        let source = tokens.next().and_then(|first| {
            let start = resolve_span(db, first.first_span());
            let end = match tokens.last().unwrap_or(first) {
                TtElement::Leaf(leaf) => *leaf.span(),
                TtElement::Subtree(group, _) => group.delimiter.close,
            };
            let end = resolve_span(db, end);
            if start.file_id != end.file_id {
                return None;
            }
            let text = db.file_text(start.file_id.file_id(db)).text(db);
            text.get(usize::from(start.range.start())..usize::from(end.range.end()))
                .and_then(|s| rustier_js_syntax::ParsedJsMacro::parse(s.parse().ok()?).ok())
        });

        let annotated = source.map(|parsed| {
            let value = if rustier_js_syntax::returns_value(&parsed.body) {
                "1"
            } else {
                "0"
            };
            let mut builder = TopSubtreeBuilder::new(input.top_subtree().delimiter);
            builder.extend_with_tt(prefix);
            builder.push(Leaf::Literal(Literal::new(
                value,
                call_site,
                LitKind::Integer,
                "__rustier_output",
            )));
            builder.extend_with_tt(body);
            builder.build()
        });
        self.inner.expand(
            db,
            annotated.as_ref().unwrap_or(input),
            attrs,
            env,
            def_site,
            call_site,
            mixed_site,
            current_dir,
        )
    }

    fn eq_dyn(&self, other: &dyn ProcMacroExpander) -> bool {
        (other as &dyn std::any::Any)
            .downcast_ref::<Self>()
            .is_some_and(|other| {
                self.deferred == other.deferred && self.inner.eq_dyn(&*other.inner)
            })
    }
}
