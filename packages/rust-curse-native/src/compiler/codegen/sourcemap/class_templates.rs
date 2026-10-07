use crate::compiler::{
    codegen::{
        fragment::{FragmentPrint, frag},
        sourcemap::{
            classes::declaration_mappings,
            mappings::{Mapping, find_name_line, original_location},
        },
    },
    db::Db,
    ir::decals::{DecalClass, DecalClassTemplate, DecalModule},
};

#[salsa::tracked]
impl<'db> DecalClassTemplate<'db> {
    #[salsa::tracked(returns(ref))]
    pub(super) fn render_sourcemap_mappings(
        self,
        db: &'db dyn Db,
        module: DecalModule<'db>,
        generated_line: usize,
    ) -> Vec<Mapping> {
        let generated = module.render_ts_mod(db);
        let mut mappings = Vec::new();
        let mut start_line = generated_line;

        for (_, class) in self.mappings(db) {
            let (prefix, suffix) = match class {
                DecalClass::Plain(_) => ("declare class ", " {"),
                DecalClass::Discriminated(_) => ("type ", " ="),
            };
            let line = find_name_line(generated, start_line, prefix, class.name(db), suffix)
                .expect("rendered TypeScript template class should exist");
            start_line = line + 1;

            mappings.extend(
                class
                    .render_sourcemap_mappings(db, module, line)
                    .iter()
                    .cloned(),
            );
        }

        let Some(span) = self.span(db) else {
            return mappings;
        };
        let original = original_location(db, span);
        let original_name_column = original.column
            + if self.is_discriminated(db) {
                "pub enum ".len()
            } else {
                "pub struct ".len()
            };
        let name = self.name(db);
        let private_name = frag!(_{name}).print();
        let type_line = find_name_line(generated, start_line, "export type ", name, "<")
            .expect("rendered TypeScript template alias should exist");
        let private_line = find_name_line(generated, type_line + 1, "type ", &private_name, "<")
            .expect("rendered TypeScript private template alias should exist");
        let namespace_line = find_name_line(
            generated,
            private_line + 1,
            "export declare namespace ",
            name,
            " {",
        )
        .expect("rendered TypeScript template namespace should exist");

        for (line, column) in [
            (type_line, "export type ".len()),
            (private_line, "type ".len()),
            (namespace_line, "export declare namespace ".len()),
        ] {
            mappings.extend(declaration_mappings(
                line,
                column,
                &original,
                original_name_column,
                name,
            ));
        }

        start_line = namespace_line + 1;

        for (alias, prefix) in [
            ("Class", "\texport type "),
            ("_Class", "\ttype "),
            ("Plain", "\texport type "),
            ("Coerce", "\texport type "),
        ] {
            let line = find_name_line(generated, start_line, prefix, alias, "<")
                .expect("rendered TypeScript template projection should exist");
            start_line = line + 1;

            mappings.extend(declaration_mappings(
                line,
                prefix.len(),
                &original,
                original_name_column,
                name,
            ));

            if alias != "Plain" && alias != "Coerce" {
                continue;
            }

            for (generics, class) in self.mappings(db) {
                let condition = self.render_condition_ts(db, generics).print();
                let line = find_name_line(generated, start_line, "\t\t", &condition, "")
                    .expect("rendered TypeScript template branch should exist");
                start_line = line + 1;

                mappings.extend(class.render_projection_sourcemap_mappings(
                    db,
                    generated,
                    start_line,
                    "\t\t\t\t",
                    alias == "Coerce",
                ));
            }
        }

        let function_line = find_name_line(generated, start_line, "export function ", name, "<")
            .expect("rendered TypeScript template function should exist");

        mappings.extend(declaration_mappings(
            function_line,
            "export function ".len(),
            &original,
            original_name_column,
            name,
        ));

        mappings
    }
}
