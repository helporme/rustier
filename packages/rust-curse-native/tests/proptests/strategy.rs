use proptest::{collection::vec, prelude::*};
use rust_curse::{codegen::fragment::FragmentPrint as _, frag};

use super::program::{
    Enum, EnumVariant, Function, FunctionParam, Item, Module, Program, Shape, ShapeField, Struct,
    Type,
};

const MAX_DECLARATIONS: usize = 4;
const MAX_FUNCTIONS: usize = 4;
const MAX_FIELDS: usize = 3;
const MAX_PARAMETERS: usize = 3;
const MAX_ENUM_VARIANTS: usize = 3;
const MAX_MODULE_CHILDREN: usize = 3;
const OWNED_PRIMITIVES: &[&str] = &[
    "bool", "char", "i8", "i16", "i32", "u8", "u16", "u32", "f32", "f64",
];

#[derive(Debug, Clone, Copy)]
enum DeclarationKind {
    Struct,
    CLikeEnum,
    DataEnum,
}

#[derive(Debug, Clone)]
struct TypeSymbol {
    declaration_index: usize,
    path: String,
    is_clone: bool,
}

#[derive(Debug, Clone)]
struct FunctionSpec {
    params: Vec<Type>,
    return_type: Option<Type>,
}

#[derive(Debug, Clone)]
struct ScopeSpec {
    items: Vec<Item>,
    children: Vec<ScopeSpec>,
}

impl ScopeSpec {
    fn into_items(mut self) -> Vec<Item> {
        self.items
            .extend(self.children.into_iter().enumerate().map(|(index, child)| {
                Item::Mod(Module {
                    name: frag!(module_ { index }).print(),
                    items: child.into_items(),
                })
            }));
        self.items
    }
}

/// Generates programs from dependency-closed scopes in rust-curse's supported
/// Rust subset.
pub fn program() -> impl Strategy<Value = Program> {
    scope().prop_map(|scope| Program {
        items: scope.into_items(),
    })
}

fn scope() -> BoxedStrategy<ScopeSpec> {
    scope_items()
        .prop_map(|items| ScopeSpec {
            items,
            children: Vec::new(),
        })
        .prop_recursive(3, 24, MAX_MODULE_CHILDREN as u32, |inner| {
            (scope_items(), vec(inner, 1..=MAX_MODULE_CHILDREN))
                .prop_map(|(items, children)| ScopeSpec { items, children })
        })
        .boxed()
}

fn scope_items() -> BoxedStrategy<Vec<Item>> {
    vec(declaration_kind(), 0..=MAX_DECLARATIONS)
        .prop_flat_map(|declaration_kinds| {
            let symbols = declaration_kinds
                .iter()
                .copied()
                .enumerate()
                .filter_map(|(declaration_index, kind)| {
                    let is_clone = match kind {
                        DeclarationKind::Struct => true,
                        DeclarationKind::DataEnum => false,
                        DeclarationKind::CLikeEnum => return None,
                    };
                    Some(TypeSymbol {
                        declaration_index,
                        path: frag!(self::Type { declaration_index }).print(),
                        is_clone,
                    })
                })
                .collect::<Vec<_>>();

            let declarations = declaration_kinds
                .into_iter()
                .enumerate()
                .map(|(declaration_index, kind)| {
                    let preceding_symbols = symbols
                        .iter()
                        .filter(|symbol| symbol.declaration_index < declaration_index)
                        .cloned()
                        .collect();
                    declaration(kind, declaration_index, preceding_symbols)
                })
                .collect::<Vec<_>>();

            (declarations, vec(function_spec(symbols), 0..=MAX_FUNCTIONS)).prop_map(
                |(mut declarations, functions)| {
                    declarations.extend(functions.into_iter().enumerate().map(
                        |(function_index, function)| {
                            Item::Function(Function {
                                name: frag!(function_ { function_index }).print(),
                                params: function
                                    .params
                                    .into_iter()
                                    .enumerate()
                                    .map(|(parameter_index, ty)| FunctionParam {
                                        name: frag!(argument_ { parameter_index }).print(),
                                        ty,
                                    })
                                    .collect(),
                                return_type: function.return_type,
                            })
                        },
                    ));
                    declarations
                },
            )
        })
        .boxed()
}

fn declaration_kind() -> impl Strategy<Value = DeclarationKind> {
    prop_oneof![
        3 => Just(DeclarationKind::Struct),
        1 => Just(DeclarationKind::CLikeEnum),
        2 => Just(DeclarationKind::DataEnum),
    ]
}

fn declaration(
    kind: DeclarationKind,
    declaration_index: usize,
    preceding_symbols: Vec<TypeSymbol>,
) -> BoxedStrategy<Item> {
    let name = frag!(Type { declaration_index }).print();

    match kind {
        DeclarationKind::Struct => {
            let clone_paths = preceding_symbols
                .into_iter()
                .filter(|symbol| symbol.is_clone)
                .map(|symbol| symbol.path)
                .collect();
            shape(owned_type(clone_paths))
                .prop_map(move |shape| {
                    Item::Struct(Struct {
                        name: name.clone(),
                        shape,
                    })
                })
                .boxed()
        }
        DeclarationKind::CLikeEnum => c_like_enum(name).prop_map(Item::Enum).boxed(),
        DeclarationKind::DataEnum => {
            let resolved_paths = preceding_symbols
                .into_iter()
                .map(|symbol| symbol.path)
                .collect();
            data_enum(name, resolved_paths).prop_map(Item::Enum).boxed()
        }
    }
}

fn primitive_owned_type() -> BoxedStrategy<Type> {
    prop::sample::select(OWNED_PRIMITIVES.to_vec())
        .prop_map(|name| Type::Primitive(name.to_owned()))
        .boxed()
}

fn owned_type(resolved_paths: Vec<String>) -> BoxedStrategy<Type> {
    let primitive = primitive_owned_type();
    let string = Just(Type::ResolvedPath("String".to_owned()));

    if resolved_paths.is_empty() {
        prop_oneof![
            5 => primitive,
            2 => string,
        ]
        .boxed()
    } else {
        prop_oneof![
            5 => primitive,
            2 => string,
            3 => prop::sample::select(resolved_paths).prop_map(Type::ResolvedPath),
        ]
        .boxed()
    }
}

fn parameter_type(resolved_paths: Vec<String>, clone_paths: Vec<String>) -> BoxedStrategy<Type> {
    let moved = owned_type(clone_paths);
    let referent = prop_oneof![
        6 => owned_type(resolved_paths),
        1 => Just(Type::Primitive("str".to_owned())),
    ];
    let borrowed = (any::<bool>(), referent).prop_map(|(is_mut, inner)| Type::Ref {
        is_mut,
        inner: Box::new(inner),
    });

    prop_oneof![
        3 => moved,
        2 => borrowed,
    ]
    .boxed()
}

fn function_spec(symbols: Vec<TypeSymbol>) -> BoxedStrategy<FunctionSpec> {
    let resolved_paths = symbols
        .iter()
        .map(|symbol| symbol.path.clone())
        .collect::<Vec<_>>();
    let clone_paths = symbols
        .into_iter()
        .filter(|symbol| symbol.is_clone)
        .map(|symbol| symbol.path)
        .collect::<Vec<_>>();

    (
        vec(
            parameter_type(resolved_paths.clone(), clone_paths),
            0..=MAX_PARAMETERS,
        ),
        prop::option::of(owned_type(resolved_paths)),
    )
        .prop_map(|(params, return_type)| FunctionSpec {
            params,
            return_type,
        })
        .boxed()
}

fn shape(field_type: BoxedStrategy<Type>) -> BoxedStrategy<Shape> {
    let tuple = vec(field_type.clone(), 0..=MAX_FIELDS).prop_map(Shape::Tuple);
    let struct_ = vec(field_type, 0..=MAX_FIELDS).prop_map(|types| Shape::Struct {
        fields: types
            .into_iter()
            .enumerate()
            .map(|(index, ty)| ShapeField {
                name: frag!(field_ { index }).print(),
                ty,
            })
            .collect(),
    });

    prop_oneof![Just(Shape::Unit), tuple, struct_,].boxed()
}

fn data_variant_shape(field_type: BoxedStrategy<Type>) -> BoxedStrategy<Shape> {
    let tuple = vec(field_type.clone(), 0..=MAX_FIELDS).prop_map(Shape::Tuple);
    let struct_ = vec(field_type, 0..=MAX_FIELDS).prop_map(|types| Shape::Struct {
        fields: types
            .into_iter()
            .enumerate()
            .map(|(index, ty)| ShapeField {
                name: frag!(field_ { index }).print(),
                ty,
            })
            .collect(),
    });

    prop_oneof![tuple, struct_].boxed()
}

fn c_like_enum(name: String) -> BoxedStrategy<Enum> {
    (1usize..=MAX_ENUM_VARIANTS)
        .prop_map(move |variant_count| Enum {
            name: name.clone(),
            variants: (0..variant_count)
                .map(|index| EnumVariant {
                    name: frag!(Variant { index }).print(),
                    shape: Shape::Unit,
                })
                .collect(),
        })
        .boxed()
}

fn data_enum(name: String, resolved_paths: Vec<String>) -> BoxedStrategy<Enum> {
    (
        data_variant_shape(owned_type(resolved_paths)),
        0usize..MAX_ENUM_VARIANTS,
    )
        .prop_map(move |(data_shape, unit_variant_count)| {
            let mut variants = Vec::with_capacity(unit_variant_count + 1);
            variants.push(EnumVariant {
                name: frag!(Variant0).print(),
                shape: data_shape,
            });
            variants.extend((1..=unit_variant_count).map(|index| EnumVariant {
                name: frag!(Variant { index }).print(),
                shape: Shape::Unit,
            }));

            Enum {
                name: name.clone(),
                variants,
            }
        })
        .boxed()
}
