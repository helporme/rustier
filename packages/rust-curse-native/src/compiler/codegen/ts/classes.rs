use crate::{
    compiler::{
        codegen::{
            fragment::{FragmentOnce, FragmentPrint, blank, curlies, doubles, fragment, idented},
            ts::jsdoc::render_jsdoc,
        },
        db::Db,
        ir::decals::{DecalClass, DecalClassKind, DecalDiscriminator, DecalField, DecalVariant},
        ir::rust::FieldName,
    },
    frag,
};

#[salsa::tracked]
impl<'db> DecalClass<'db> {
    #[salsa::tracked(returns(ref))]
    pub(super) fn render_ts(self, db: &'db dyn Db) -> String {
        match self.kind(db) {
            DecalClassKind::Plain { fields } => self.render_plain_ts(db, fields),
            DecalClassKind::DiscriminatedUnion {
                variants,
                discriminator,
            } => self.render_discriminated_ts(db, variants, *discriminator),
        }
    }

    fn render_plain_ts(self, db: &'db dyn Db, fields: &'db [DecalField<'db>]) -> String {
        let fields = fragment(move |p| {
            for (index, field) in fields.iter().enumerate() {
                if index > 0 {
                    frag!(@).apply(p);
                }
                field.render_ts(db, true).apply(p);
            }

            if !fields.is_empty() {
                frag!(@).apply(p);
            }
            brand().apply(p);
        });

        frag!(
            {render_jsdoc(self.docs(db).map(String::as_str))}
            export class ~{self.name(db)}~{{ {fields} }}
        )
        .print()
    }

    fn render_discriminated_ts(
        self,
        db: &'db dyn Db,
        variants: &'db [DecalVariant<'db>],
        discriminator: DecalDiscriminator<'db>,
    ) -> String {
        let rendered_variants = fragment(move |p| {
            for (index, variant) in variants.iter().enumerate() {
                if index > 0 {
                    blank().apply(p);
                }
                variant
                    .render_ts(db, discriminator.name(db), variant.name(db))
                    .apply(p);
            }
            frag!(;).apply(p);
        });
        let plain = fragment(move |p| {
            frag!(export type Plain =).apply(p);

            if variants.is_empty() {
                frag!( never;).apply(p);
            } else {
                blank().apply(p);
                idented(rendered_variants).apply(p);
            }
        });

        frag!(
            {render_jsdoc(self.docs(db).map(String::as_str))}
            export type ~{self.name(db)}~ = ~{self.name(db)}.Instance & ~{self.name(db)}.Plain;@@

            export namespace ~{self.name(db)}~{{
                {plain}@@

                export class Instance~{{
                    {brand()}
                }}
            }}
        )
        .print()
    }
}

impl<'db> DecalVariant<'db> {
    fn render_ts(
        self,
        db: &'db dyn Db,
        discriminator_name: &'db str,
        tag_name: &'db str,
    ) -> impl FragmentOnce + 'db {
        let properties = fragment(move |p| {
            frag!(readonly ~{discriminator_name}:~{doubles(tag_name)};).apply(p);

            for field in self.fields(db) {
                blank().apply(p);
                field.render_ts(db, false).apply(p);
            }
        });

        frag!(
            {render_jsdoc(self.docs(db).map(String::as_str))}
            |~{curlies(properties)}
        )
    }
}

impl<'db> DecalField<'db> {
    fn render_ts(self, db: &'db dyn Db, is_class_field: bool) -> impl FragmentOnce + 'db {
        let visibility = is_class_field.then_some("public ").unwrap_or_default();
        let readonly = self
            .is_readonly(db)
            .then_some("readonly ")
            .unwrap_or_default();
        let name = fragment(move |p| {
            let name = self.name(db);
            match name {
                FieldName::Idx(_) if !is_class_field => ("[", name, "]").apply(p),
                FieldName::Name(value) if !is_class_field && value.parse::<usize>().is_ok() => {
                    ("[", name, "]").apply(p);
                }
                _ => name.apply(p),
            }
        });

        frag!(
            {render_jsdoc(self.docs(db).map(String::as_str))}
            {visibility}{readonly}{name}:~{self.type_(db).render_ts(db)};
        )
    }
}

fn brand() -> impl FragmentOnce {
    frag!(
        private __rust_curse_brand: never;@
        private constructor();
    )
}
