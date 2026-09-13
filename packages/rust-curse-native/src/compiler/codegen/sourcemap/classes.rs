//! Generates source map mappings for TypeScript class declarations.

use crate::compiler::{
    codegen::sourcemap::mappings::{Mapping, OriginalLocation, find_name_line, original_location},
    db::Db,
    ir::{
        decals::{
            DecalClass, DecalDiscriminator, DecalField, DecalMethod, DecalModule, DecalVariant,
        },
        rust::FieldName,
    },
};

#[salsa::tracked]
impl<'db> DecalClass<'db> {
    /// Renders mappings for this class at its first declaration line.
    #[salsa::tracked(returns(ref))]
    pub(super) fn render_sourcemap_mappings(
        self,
        db: &'db dyn Db,
        module: DecalModule<'db>,
        generated_line: usize,
    ) -> Vec<Mapping> {
        match self {
            DecalClass::Plain(plain) => self.render_plain_sourcemap_mappings(
                db,
                module,
                plain.fields(db).collect(),
                generated_line,
            ),
            DecalClass::Discriminated(discriminated) => self
                .render_discriminated_sourcemap_mappings(
                    db,
                    module,
                    discriminated.variants(db),
                    discriminated.discriminator(db),
                    generated_line,
                ),
        }
    }

    fn render_plain_sourcemap_mappings(
        self,
        db: &'db dyn Db,
        module: DecalModule<'db>,
        fields: Vec<DecalField<'db>>,
        generated_line: usize,
    ) -> Vec<Mapping> {
        let generated = module.render_ts_mod(db);
        let name = self.name(db);
        let class_line = find_name_line(generated, generated_line, "export class ", name, " {")
            .expect("rendered TypeScript class declaration should exist");
        let Some(span) = self.span(db) else {
            return Vec::new();
        };
        let original = original_location(db, span);
        let mut mappings = declaration_mappings(
            class_line,
            "export class ".len(),
            &original,
            original.column + "pub struct ".len(),
            name,
        )
        .to_vec();
        let mut field_start_line = class_line + 1;

        for field in &fields {
            let prefix = if field.is_readonly(db) {
                "\tpublic readonly "
            } else {
                "\tpublic "
            };
            let field_name = field.name(db).to_string();
            let field_line = find_name_line(generated, field_start_line, prefix, &field_name, ":")
                .expect("rendered TypeScript class field declaration should exist");
            field_start_line = field_line + 1;

            let Some(span) = field.span(db) else {
                continue;
            };
            let original = original_location(db, span);
            let original_name_column = match field.name(db) {
                FieldName::Name(name) if name.parse::<usize>().is_err() => {
                    original.column + "pub ".len()
                }
                FieldName::Name(_) | FieldName::Idx(_) => original.column,
            };
            mappings.extend(declaration_mappings(
                field_line,
                prefix.len(),
                &original,
                original_name_column,
                &field_name,
            ));
        }

        let constructor = self.constructor(db);
        for method in self
            .methods(db)
            .filter(|method| Some(*method) != constructor)
        {
            let (prefix, suffix) = class_method_search(&method, db, "\t");
            let method_line = find_name_line(
                generated,
                field_start_line,
                &prefix,
                method.name(db),
                suffix,
            )
            .expect("rendered TypeScript class method declaration should exist");
            field_start_line = method_line + 1;
            mappings.extend(method_mappings(&method, db, method_line, prefix.len()));
        }

        if let Some(constructor) = constructor {
            let prefix = "\t";
            let constructor_line =
                find_name_line(generated, field_start_line, prefix, "constructor", "(")
                    .expect("rendered TypeScript class constructor declaration should exist");
            mappings.extend(method_mappings(
                &constructor,
                db,
                constructor_line,
                prefix.len(),
            ));
        }

        let namespace_line = find_name_line(
            generated,
            field_start_line,
            "export declare namespace ",
            name,
            " {",
        )
        .expect("rendered TypeScript struct namespace should exist");
        let plain_line = find_name_line(
            generated,
            namespace_line + 1,
            "\texport type ",
            "Plain",
            " =",
        )
        .expect("rendered TypeScript plain struct should exist");
        for (line, column) in [
            (namespace_line, "export declare namespace ".len()),
            (plain_line, "\texport type ".len()),
        ] {
            mappings.extend(declaration_mappings(
                line,
                column,
                &original,
                original.column + "pub struct ".len(),
                name,
            ));
        }

        let mut field_start_line = plain_line + 1;
        for field in fields {
            let field_name = field.name(db).to_string();
            let wrapped = match field.name(db) {
                FieldName::Idx(_) => true,
                FieldName::Name(name) => name.parse::<usize>().is_ok(),
            };
            let prefix = format!(
                "\t\t{}{}",
                if field.is_readonly(db) {
                    "readonly "
                } else {
                    ""
                },
                if wrapped { "[" } else { "" },
            );
            let suffix = if wrapped { "]:" } else { ":" };
            let field_line =
                find_name_line(generated, field_start_line, &prefix, &field_name, suffix)
                    .expect("rendered TypeScript plain struct field should exist");
            field_start_line = field_line + 1;

            let Some(span) = field.span(db) else {
                continue;
            };
            let original = original_location(db, span);
            let original_name_column = if wrapped {
                original.column
            } else {
                original.column + "pub ".len()
            };
            mappings.extend(declaration_mappings(
                field_line,
                prefix.len(),
                &original,
                original_name_column,
                &field_name,
            ));
        }

        mappings
    }

    fn render_discriminated_sourcemap_mappings(
        self,
        db: &'db dyn Db,
        module: DecalModule<'db>,
        variants: &'db [DecalVariant<'db>],
        discriminator: DecalDiscriminator<'db>,
        generated_line: usize,
    ) -> Vec<Mapping> {
        let generated = module.render_ts_mod(db);
        let name = self.name(db);
        let type_line = find_name_line(generated, generated_line, "export type ", name, " =")
            .expect("rendered TypeScript union declaration should exist");
        let value_line = find_name_line(
            generated,
            type_line + 1,
            "export declare const ",
            name,
            ": {",
        )
        .expect("rendered TypeScript enum static value declaration should exist");
        let namespace_line = find_name_line(
            generated,
            value_line + 1,
            "export declare namespace ",
            name,
            " {",
        )
        .expect("rendered TypeScript union namespace should exist");
        let plain_line = find_name_line(
            generated,
            namespace_line + 1,
            "\texport type ",
            "Plain",
            " =",
        )
        .expect("rendered TypeScript plain union should exist");
        let instance_line = find_name_line(
            generated,
            plain_line + 1,
            "\texport interface ",
            "Instance",
            " {",
        )
        .expect("rendered TypeScript union instance interface should exist");
        let Some(span) = self.span(db) else {
            return Vec::new();
        };
        let original = original_location(db, span);
        let original_name_column = original.column + "pub enum ".len();
        let mut mappings = Vec::new();

        for (line, generated_name_column) in [
            (type_line, "export type ".len()),
            (value_line, "export declare const ".len()),
            (namespace_line, "export declare namespace ".len()),
            (plain_line, "\texport type ".len()),
            (instance_line, "\texport interface ".len()),
        ] {
            mappings.extend(declaration_mappings(
                line,
                generated_name_column,
                &original,
                original_name_column,
                name,
            ));
        }

        let mut static_method_start_line = value_line + 1;
        for method in self.methods(db).filter(|method| method.is_static(db)) {
            let (prefix, suffix) = object_method_search(&method, db, "\t");
            let method_line = find_name_line(
                generated,
                static_method_start_line,
                &prefix,
                method.name(db),
                suffix,
            )
            .expect("rendered TypeScript enum static method declaration should exist");
            static_method_start_line = method_line + 1;
            mappings.extend(method_mappings(&method, db, method_line, prefix.len()));
        }

        let mut instance_method_start_line = instance_line + 1;
        for method in self.methods(db).filter(|method| !method.is_static(db)) {
            let (prefix, suffix) = object_method_search(&method, db, "\t\t");
            let method_line = find_name_line(
                generated,
                instance_method_start_line,
                &prefix,
                method.name(db),
                suffix,
            )
            .expect("rendered TypeScript enum instance method declaration should exist");
            instance_method_start_line = method_line + 1;
            mappings.extend(method_mappings(&method, db, method_line, prefix.len()));
        }

        let tag_prefix = format!("\t\t\treadonly {}: \"", discriminator.name(db));
        let mut variant_start_line = plain_line + 1;

        for variant in variants {
            let tag_line = find_name_line(
                generated,
                variant_start_line,
                &tag_prefix,
                variant.name(db),
                "\";",
            )
            .expect("rendered TypeScript union variant discriminant should exist");
            variant_start_line = tag_line + 1;

            let Some(span) = variant.span(db) else {
                continue;
            };
            let original = original_location(db, span);
            mappings.extend(declaration_mappings(
                tag_line,
                "\t\t\treadonly ".len(),
                &original,
                original.column,
                variant.source_name(db),
            ));

            for field in variant.fields(db) {
                let field_name = field.name(db).to_string();
                let wrapped = match field.name(db) {
                    FieldName::Idx(_) => true,
                    FieldName::Name(name) => name.parse::<usize>().is_ok(),
                };
                let prefix = format!(
                    "\t\t\t{}{}",
                    if field.is_readonly(db) {
                        "readonly "
                    } else {
                        ""
                    },
                    if wrapped { "[" } else { "" },
                );
                let suffix = if wrapped { "]:" } else { ":" };
                let field_line =
                    find_name_line(generated, variant_start_line, &prefix, &field_name, suffix)
                        .expect("rendered TypeScript union field should exist");
                variant_start_line = field_line + 1;

                let Some(span) = field.span(db) else {
                    continue;
                };
                let original = original_location(db, span);
                mappings.extend(declaration_mappings(
                    field_line,
                    prefix.len(),
                    &original,
                    original.column,
                    &field_name,
                ));
            }
        }

        mappings.sort_by_key(|mapping| (mapping.generated_line, mapping.generated_column));
        mappings
    }
}

fn object_method_search<'db>(
    method: &DecalMethod<'db>,
    db: &'db dyn Db,
    indent: &str,
) -> (String, &'static str) {
    if method.name(db) == "new" {
        (format!("{indent}\""), "\"(")
    } else {
        (indent.to_owned(), "(")
    }
}

fn class_method_search<'db>(
    method: &DecalMethod<'db>,
    db: &'db dyn Db,
    indent: &str,
) -> (String, &'static str) {
    let static_prefix = if method.is_static(db) { "static " } else { "" };
    if method.name(db) == "constructor" {
        (format!("{indent}{static_prefix}[\""), "\"](")
    } else {
        (format!("{indent}{static_prefix}"), "(")
    }
}

fn method_mappings<'db>(
    method: &DecalMethod<'db>,
    db: &'db dyn Db,
    generated_line: usize,
    generated_name_column: usize,
) -> Vec<Mapping> {
    let Some(span) = method.span(db) else {
        return Vec::new();
    };
    let original = original_location(db, span);

    declaration_mappings(
        generated_line,
        generated_name_column,
        &original,
        original.column + "pub fn ".len(),
        method.name(db),
    )
    .to_vec()
}

fn declaration_mappings(
    generated_line: usize,
    generated_name_column: usize,
    original: &OriginalLocation,
    original_name_column: usize,
    name: &str,
) -> [Mapping; 2] {
    [
        Mapping {
            generated_line,
            generated_column: 0,
            source: original.source.clone(),
            original_line: original.line,
            original_column: original.column,
            name: name.to_owned(),
        },
        Mapping {
            generated_line,
            generated_column: generated_name_column,
            source: original.source.clone(),
            original_line: original.line,
            original_column: original_name_column,
            name: name.to_owned(),
        },
    ]
}
