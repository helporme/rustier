//! Generates source map mappings for TypeScript class declarations.

use crate::compiler::{
    codegen::{
        fragment::{FragmentPrint, frag},
        sourcemap::mappings::{Mapping, OriginalLocation, find_name_line, original_location},
    },
    db::Db,
    ir::{
        decals::{DecalClass, DecalField, DecalMethod, DecalModule},
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
            DecalClass::Discriminated(_) => {
                self.render_discriminated_sourcemap_mappings(db, module, generated_line)
            }
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
        let prefix = if self.is_template_instance(db) {
            "declare class "
        } else {
            "export class "
        };
        let class_line = find_name_line(generated, generated_line, prefix, name, " {")
            .expect("rendered TypeScript class declaration should exist");
        let Some(span) = self.span(db) else {
            return Vec::new();
        };
        let original = original_location(db, span);
        let mut mappings = declaration_mappings(
            class_line,
            prefix.len(),
            &original,
            original.column + "pub struct ".len(),
            name,
        )
        .to_vec();
        let mut field_start_line = class_line + 1;

        for field in &fields {
            let field_name = field.name(db).to_string();
            let locations = field_declaration_locations(
                field,
                db,
                generated,
                &mut field_start_line,
                "\tpublic ",
                true,
                false,
            );

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
            for (field_line, name_column) in locations {
                mappings.extend(declaration_mappings(
                    field_line,
                    name_column,
                    &original,
                    original_name_column,
                    &field_name,
                ));
            }
        }

        let constructor = self.constructor(db);

        for method in self.methods(db) {
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

        mappings.extend(self.coerce_mappings(db, generated, field_start_line, "\tstatic "));

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

        if self.is_template_instance(db) {
            return mappings;
        }

        let namespace_line = find_name_line(
            generated,
            field_start_line,
            "export declare namespace ",
            name,
            " {",
        )
        .expect("rendered TypeScript struct namespace should exist");
        mappings.extend(declaration_mappings(
            namespace_line,
            "export declare namespace ".len(),
            &original,
            original.column + "pub struct ".len(),
            name,
        ));
        let mut projection_start_line = namespace_line + 1;

        for (alias, coerce) in [("Plain", false), ("Coerce", true)] {
            if coerce && self.coerce(db).is_none() {
                continue;
            }

            let projection_line = find_name_line(
                generated,
                projection_start_line,
                "\texport type ",
                alias,
                " =",
            )
            .expect("rendered TypeScript struct projection should exist");
            mappings.extend(declaration_mappings(
                projection_line,
                "\texport type ".len(),
                &original,
                original.column + "pub struct ".len(),
                name,
            ));

            projection_start_line = projection_line + 1;

            mappings.extend(self.render_projection_sourcemap_mappings(
                db,
                generated,
                projection_start_line,
                "\t\t",
                coerce,
            ));
        }

        mappings
    }

    fn render_discriminated_sourcemap_mappings(
        self,
        db: &'db dyn Db,
        module: DecalModule<'db>,
        generated_line: usize,
    ) -> Vec<Mapping> {
        if self.is_template_instance(db) {
            return self.render_template_enum_sourcemap_mappings(db, module, generated_line);
        }

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

        mappings.extend(self.coerce_mappings(db, generated, static_method_start_line, "\t"));

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

        let mut projections = vec![(plain_line, false)];
        if self.coerce(db).is_some() {
            let coerce_line =
                find_name_line(generated, plain_line + 1, "\texport type ", "Coerce", " =")
                    .expect("rendered TypeScript coercion union should exist");
            mappings.extend(declaration_mappings(
                coerce_line,
                "\texport type ".len(),
                &original,
                original_name_column,
                name,
            ));
            projections.push((coerce_line, true));
        }
        for (projection_line, coerce) in projections {
            mappings.extend(self.render_projection_sourcemap_mappings(
                db,
                generated,
                projection_line + 1,
                "\t\t\t",
                coerce,
            ));
        }

        mappings.sort_by_key(|mapping| (mapping.generated_line, mapping.generated_column));
        mappings
    }

    fn render_template_enum_sourcemap_mappings(
        self,
        db: &'db dyn Db,
        module: DecalModule<'db>,
        generated_line: usize,
    ) -> Vec<Mapping> {
        let Some(span) = self.span(db) else {
            return Vec::new();
        };
        let generated = module.render_ts_mod(db);
        let name = self.name(db);
        let original = original_location(db, span);
        let type_line = find_name_line(generated, generated_line, "type ", name, " =")
            .expect("rendered TypeScript template enum type should exist");
        let value_line = find_name_line(generated, type_line + 1, "declare const ", name, ":")
            .expect("rendered TypeScript template enum value should exist");
        let instance_name = frag!(${name}Instance).print();
        let instance_line = find_name_line(
            generated,
            value_line + 1,
            "interface ",
            &instance_name,
            " {",
        )
        .expect("rendered TypeScript template enum instance should exist");
        let mut mappings = Vec::new();

        for (line, column) in [
            (type_line, "type ".len()),
            (value_line, "declare const ".len()),
            (instance_line, "interface ".len()),
        ] {
            mappings.extend(declaration_mappings(
                line,
                column,
                &original,
                original.column + "pub enum ".len(),
                name,
            ));
        }

        mappings.extend(self.render_projection_sourcemap_mappings(
            db,
            generated,
            type_line + 1,
            "\t\t",
            false,
        ));

        for (is_static, mut start_line) in [(true, value_line + 1), (false, instance_line + 1)] {
            for method in self
                .methods(db)
                .filter(|method| method.is_static(db) == is_static)
            {
                let (prefix, suffix) = object_method_search(&method, db, "\t");
                let line = find_name_line(generated, start_line, &prefix, method.name(db), suffix)
                    .expect("rendered TypeScript template enum method should exist");
                start_line = line + 1;

                mappings.extend(method_mappings(&method, db, line, prefix.len()));
            }

            if is_static {
                mappings.extend(self.coerce_mappings(db, generated, start_line, "\t"));
            }
        }

        mappings
    }

    pub(super) fn render_projection_sourcemap_mappings(
        self,
        db: &'db dyn Db,
        generated: &str,
        mut start_line: usize,
        indent: &str,
        coerce: bool,
    ) -> Vec<Mapping> {
        let mut mappings = Vec::new();

        match self {
            Self::Plain(class) => {
                let fields = if coerce {
                    let Some(coerce) = self.coerce(db) else {
                        return mappings;
                    };

                    coerce.input_fields(db)
                } else {
                    class.fields(db).collect()
                };

                for field in fields {
                    let locations = field_declaration_locations(
                        &field,
                        db,
                        generated,
                        &mut start_line,
                        indent,
                        false,
                        true,
                    );
                    let Some(span) = field.span(db) else {
                        continue;
                    };
                    let original = original_location(db, span);
                    let original_name_column = match field.name(db) {
                        FieldName::Name(name) if name.parse::<usize>().is_err() => {
                            original.column + "pub ".len()
                        }
                        _ => original.column,
                    };

                    for (line, column) in locations {
                        mappings.extend(declaration_mappings(
                            line,
                            column,
                            &original,
                            original_name_column,
                            &field.name(db).to_string(),
                        ));
                    }
                }
            }
            Self::Discriminated(class) => {
                if coerce && self.coerce(db).is_none() {
                    return mappings;
                }

                let tag_prefix =
                    frag!({indent}readonly ~{class.discriminator(db).name(db)}:~{"\""}).print();

                for variant in class
                    .variants(db)
                    .iter()
                    .filter(|variant| !coerce || variant.if_coercible(db))
                {
                    let line =
                        find_name_line(generated, start_line, &tag_prefix, variant.name(db), "\";")
                            .expect("rendered TypeScript template variant should exist");
                    start_line = line + 1;

                    if let Some(span) = variant.span(db) {
                        let original = original_location(db, span);

                        mappings.extend(declaration_mappings(
                            line,
                            indent.len() + "readonly ".len(),
                            &original,
                            original.column,
                            variant.source_name(db),
                        ));
                    }

                    for field in variant.fields(db) {
                        let locations = field_declaration_locations(
                            &field,
                            db,
                            generated,
                            &mut start_line,
                            indent,
                            !coerce,
                            true,
                        );
                        let Some(span) = field.span(db) else {
                            continue;
                        };
                        let original = original_location(db, span);

                        for (line, column) in locations {
                            mappings.extend(declaration_mappings(
                                line,
                                column,
                                &original,
                                original.column,
                                &field.name(db).to_string(),
                            ));
                        }
                    }
                }
            }
        }

        mappings
    }

    /// Synthetic coerce methods navigate to the class they construct, not a
    /// source method.
    pub(super) fn coerce_mappings(
        self,
        db: &'db dyn Db,
        generated: &str,
        start_line: usize,
        prefix: &str,
    ) -> Vec<Mapping> {
        let Some(coerce) = self.coerce(db) else {
            return Vec::new();
        };
        let class = coerce.class(db);
        let Some(span) = class.span(db) else {
            return Vec::new();
        };
        let line = find_name_line(generated, start_line, prefix, coerce.name(db), "(")
            .expect("rendered TypeScript coerce method should exist");
        let original = original_location(db, span);
        let declaration_prefix = match class {
            DecalClass::Plain(_) => "pub struct ",
            DecalClass::Discriminated(_) => "pub enum ",
        };
        declaration_mappings(
            line,
            prefix.len(),
            &original,
            original.column + declaration_prefix.len(),
            class.name(db),
        )
        .to_vec()
    }
}

/// Accessors share one Rust field origin, with setters only for writable
/// fields. Input projections remain ordinary properties regardless of
/// coercibility.
pub(super) fn field_declaration_locations<'db>(
    field: &DecalField<'db>,
    db: &'db dyn Db,
    generated: &str,
    start_line: &mut usize,
    indent: &str,
    accessors: bool,
    wrap_numeric_properties: bool,
) -> Vec<(usize, usize)> {
    let field_name = field.name(db).to_string();
    let wrapped = (accessors || wrap_numeric_properties)
        && match field.name(db) {
            FieldName::Idx(_) => true,
            FieldName::Name(name) => name.parse::<usize>().is_ok(),
        };
    let modifiers: &[&str] = if accessors {
        if field.is_readonly(db) {
            &["get "]
        } else {
            &["get ", "set "]
        }
    } else if field.is_readonly(db) {
        &["readonly "]
    } else {
        &[""]
    };
    modifiers
        .iter()
        .map(|modifier| {
            let prefix = format!("{indent}{modifier}{}", if wrapped { "[" } else { "" });
            let suffix = match (accessors, wrapped) {
                (true, true) => "](",
                (true, false) => "(",
                (false, true) => "]:",
                (false, false) => ":",
            };
            let line = find_name_line(generated, *start_line, &prefix, &field_name, suffix)
                .expect("rendered TypeScript field declaration should exist");
            *start_line = line + 1;
            (line, prefix.len())
        })
        .collect()
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

pub(super) fn declaration_mappings(
    generated_line: usize,
    generated_name_column: usize,
    original: &OriginalLocation,
    original_name_column: usize,
    name: &str,
) -> [Mapping; 2] {
    let (name_line, name_column) = original
        .name
        .unwrap_or((original.line, original_name_column));

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
            original_line: name_line,
            original_column: name_column,
            name: name.to_owned(),
        },
    ]
}
