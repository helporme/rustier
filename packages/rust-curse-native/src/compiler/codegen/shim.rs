// mod binding_fn;
// mod cast_graph;
mod class;
// mod primitive;
mod tethering;

#[cfg(test)]
#[doc(hidden)]
pub mod test;

// pub fn compile_bindings<'db>(db: &'db dyn Db, bindings: Bindings<'db>) ->
// String {     frag!(
//         #![allow(unused_mut, dead_code, unused_imports, nonstandard_style,
//     unreachable_patterns)]@@     {Api::get(db).prelude(db)}@@
//         {compile_tethering_prelude(db)}@@
//     )
//     .apply();

//     bindings.items.iter().copied().map(|item| match item {
//         BindingItem::Fn(fn_) => {}
//         BindingItem::Class(class) => {}
//     });

//     spaced(
//         modules
//             .iter()
//             .map(|mod_| compile_mod(db, *mod_))
//             .filter(|compiled| !compiled.trim().is_empty()),
//     )
//     .apply(p);

//     todo!()
// }

// impl<'db> ExternValueType<'db> {
//     fn render_type_decl(self, db: &'db dyn Db) -> impl FragmentOnce + 'db {
//         fragment(move |p| match self {
//             ExternValueType::bool => "bool".apply(p),
//             ExternValueType::i8 => "i8".apply(p),
//             ExternValueType::u8 => "u8".apply(p),
//             ExternValueType::i16 => "i16".apply(p),
//             ExternValueType::u16 => "u16".apply(p),
//             ExternValueType::i32 => "i32".apply(p),
//             ExternValueType::u32 => "u32".apply(p),
//             ExternValueType::f64 => "f64".apply(p),
//             ExternValueType::String => "String".apply(p),
//             ExternValueType::JsAny => Api::get(db).js_type(db).apply(p),
//             ExternValueType::Class(sym) => sym.value(db).apply(p),
//         })
//     }

//     fn uses_tethering(&self) -> bool {
//         match self {
//             ExternValueType::Class(_) => true,
//             ExternValueType::bool
//             | ExternValueType::i8
//             | ExternValueType::u8
//             | ExternValueType::i16
//             | ExternValueType::u16
//             | ExternValueType::i32
//             | ExternValueType::u32
//             | ExternValueType::f64
//             | ExternValueType::String
//             | ExternValueType::JsAny => false,
//         }
//     }
// }

// impl<'db> ExternType<'db> {
//     fn render_type_decl(self, db: &'db dyn Db) -> impl FragmentOnce + 'db {
//         fragment(move |p| match self {
//             ExternType::Value(extern_value_type) =>
// extern_value_type.render_type_decl(db).apply(p),
// ExternType::Borrowed(extern_value_type) => {                 frag!(&{
// extern_value_type.render_type_decl(db) }).apply(p)             }
//         })
//     }
// }

// impl<'db> ValueType<'db> {
//     fn symbol(self, db: &'db dyn Db) -> impl FragmentOnce + 'db {
//         fragment(move |p| match self {
//             ValueType::bool => "_bool".apply(p),
//             ValueType::char => "_char".apply(p),
//             ValueType::i8 => "_i8".apply(p),
//             ValueType::u8 => "_u8".apply(p),
//             ValueType::i16 => "_i16".apply(p),
//             ValueType::u16 => "_u16".apply(p),
//             ValueType::i32 => "_i32".apply(p),
//             ValueType::u32 => "_u32".apply(p),
//             ValueType::f32 => "_f32".apply(p),
//             ValueType::f64 => "_f64".apply(p),
//             ValueType::Str => "_str".apply(p),
//             ValueType::String => "_String".apply(p),
//             ValueType::Struct(struct_id) => struct_id.symbol(db).apply(p),
//             ValueType::Enum(enum_id) => enum_id.symbol(db).apply(p),
//         })
//     }

//     fn render_type_decl(self, db: &'db dyn Db) -> impl FragmentOnce + 'db {
//         fragment(move |p| match self {
//             ValueType::bool => "bool".apply(p),
//             ValueType::i8 => "i8".apply(p),
//             ValueType::u8 => "u8".apply(p),
//             ValueType::i16 => "i16".apply(p),
//             ValueType::u16 => "u16".apply(p),
//             ValueType::i32 => "i32".apply(p),
//             ValueType::u32 => "u32".apply(p),
//             ValueType::f64 => "f64".apply(p),
//             ValueType::String => "String".apply(p),
//             ValueType::char => "char".apply(p),
//             ValueType::f32 => "f32".apply(p),
//             ValueType::Str => "str".apply(p),
//             ValueType::Struct(struct_id) => struct_id.fqn(db).apply(p),
//             ValueType::Enum(enum_id) => enum_id.fqn(db).apply(p),
//         })
//     }
// }

// impl<'db> Type<'db> {
//     fn render_type_decl(self, db: &'db dyn Db) -> impl FragmentOnce + 'db {
//         fragment(move |p| match self {
//             Type::Value(type_) => type_.render_type_decl(db).apply(p),
//             Type::Borrowed { is_mutable, inner } => {
//                 if is_mutable {
//                     "&mut ".apply(p)
//                 } else {
//                     "&".apply(p)
//                 }
//                 inner.render_type_decl(db).apply(p)
//             }
//         })
//     }
// }
