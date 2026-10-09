use proptest::{collection::vec, prelude::*};

use crate::{common::fragment::FragmentPrint as _, frag};

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
const MAX_GENERIC_PARAMETERS: usize = 2;
const MAX_TYPE_DEPTH: u32 = 2;
const MAX_TYPE_SIZE: u32 = 8;
const OWNED_PRIMITIVES: &[&str] = &[
    "bool", "char", "i8", "i16", "i32", "u8", "u16", "u32", "f32", "f64",
];

#[derive(Debug, Clone, Copy)]
enum DeclarationKind {
    Struct,
    Enum,
}

#[derive(Debug, Clone, Copy)]
struct DeclarationSpec {
    kind: DeclarationKind,
    generic_arity: usize,
    is_clone: bool,
}

#[derive(Debug, Clone)]
struct TypeSymbol {
    declaration_index: usize,
    path: String,
    generic_arity: usize,
    is_clone: bool,
}

#[derive(Debug, Clone)]
struct FunctionSpec {
    params: Vec<Type>,
    return_type: Option<Type>,
}

impl FunctionSpec {
    fn into_function(self, name: String) -> Item {
        Item::Function(Function {
            name,
            params: self
                .params
                .into_iter()
                .enumerate()
                .map(|(parameter_index, ty)| FunctionParam {
                    name: frag!(argument_ { parameter_index }).print(),
                    ty,
                })
                .collect(),
            return_type: self.return_type,
        })
    }
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

/// Generates programs from dependency-closed scopes in rustier's supported
/// Rust subset.
pub fn program() -> impl Strategy<Value = Program> {
    scope().prop_map(|scope| Program {
        items: scope.into_items(),
    })
}

pub fn generic_program() -> impl Strategy<Value = Program> {
    let generic_params = vec!["T0".to_owned(), "T1".to_owned()];
    let struct_spec = DeclarationSpec {
        kind: DeclarationKind::Struct,
        generic_arity: 2,
        is_clone: true,
    };
    let struct_symbol = TypeSymbol {
        declaration_index: 0,
        path: "self::Type0".to_owned(),
        generic_arity: 2,
        is_clone: true,
    };
    let enum_fields = owned_type(vec![struct_symbol], generic_params.clone());

    (
        declaration(struct_spec, 0, Vec::new()),
        data_enum("Type1".to_owned(), generic_params, true, enum_fields),
        primitive_owned_type(),
    )
        .prop_map(|(struct_, enum_, primitive)| {
            let applied = |path: &str, args| Type::ResolvedPath {
                path: path.to_owned(),
                args,
            };
            let string = applied("String", Vec::new());
            let option_string = applied("Option", vec![string.clone()]);
            let nested_vec = applied("Vec", vec![applied("Vec", vec![primitive.clone()])]);
            let struct_type = applied("self::Type0", vec![primitive.clone(), string.clone()]);
            let enum_type = applied("self::Type1", vec![struct_type.clone(), nested_vec.clone()]);
            let nonclone_type = applied("self::Type2", vec![primitive.clone(), string.clone()]);
            let mut nonclone_enum = enum_.clone();
            nonclone_enum.name = "Type2".to_owned();
            nonclone_enum.is_clone = false;

            let nested = Item::Mod(Module {
                name: "models".to_owned(),
                items: vec![Item::Struct(Struct {
                    name: "Nested".to_owned(),
                    generic_params: vec!["T".to_owned()],
                    shape: Shape::Tuple(vec![applied(
                        "super::Type0",
                        vec![
                            Type::GenericParameter("T".to_owned()),
                            applied("Option", vec![Type::GenericParameter("T".to_owned())]),
                        ],
                    )]),
                })],
            });
            let mut items = vec![
                struct_,
                Item::Enum(enum_),
                Item::Enum(nonclone_enum),
                nested,
            ];
            let types = [
                (option_string.clone(), true),
                (applied("Option", vec![primitive.clone()]), true),
                (nested_vec.clone(), true),
                (
                    applied(
                        "Vec",
                        vec![applied("Vec", vec![Type::Primitive("u32".to_owned())])],
                    ),
                    true,
                ),
                (
                    applied("Result", vec![option_string.clone(), nested_vec.clone()]),
                    true,
                ),
                (struct_type.clone(), true),
                (applied("self::Type0", vec![string, primitive]), true),
                (
                    applied("self::Type0", vec![option_string, nested_vec]),
                    true,
                ),
                (enum_type.clone(), true),
                (nonclone_type.clone(), false),
                (applied("Vec", vec![nonclone_type.clone()]), false),
                (applied("models::Nested", vec![enum_type]), true),
                (applied("models::Nested", vec![nonclone_type]), false),
            ];

            items.extend(
                types
                    .into_iter()
                    .enumerate()
                    .map(|(index, (ty, is_clone))| {
                        FunctionSpec {
                            params: vec![
                                if is_clone {
                                    ty.clone()
                                } else {
                                    Type::Ref {
                                        is_mut: false,
                                        inner: Box::new(ty.clone()),
                                    }
                                },
                                Type::Ref {
                                    is_mut: false,
                                    inner: Box::new(ty.clone()),
                                },
                                Type::Ref {
                                    is_mut: true,
                                    inner: Box::new(ty.clone()),
                                },
                            ],
                            return_type: Some(ty),
                        }
                        .into_function(frag!(generic_ { index }).print())
                    }),
            );

            Program { items }
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
    vec(declaration_spec(), 0..=MAX_DECLARATIONS)
        .prop_flat_map(|specs| {
            let symbols = specs
                .iter()
                .copied()
                .enumerate()
                .map(|(declaration_index, spec)| TypeSymbol {
                    declaration_index,
                    path: frag!(self::Type { declaration_index }).print(),
                    generic_arity: spec.generic_arity,
                    is_clone: spec.is_clone,
                })
                .collect::<Vec<_>>();

            let declarations = specs
                .into_iter()
                .enumerate()
                .map(|(declaration_index, spec)| {
                    let preceding_symbols = symbols
                        .iter()
                        .filter(|symbol| symbol.declaration_index < declaration_index)
                        .cloned()
                        .collect();
                    declaration(spec, declaration_index, preceding_symbols)
                })
                .collect::<Vec<_>>();
            let instantiations = symbols
                .iter()
                .filter(|symbol| symbol.generic_arity > 0)
                .map(|symbol| {
                    let symbol = symbol.clone();
                    let arguments = owned_type(
                        symbols
                            .iter()
                            .filter(|symbol| symbol.is_clone)
                            .cloned()
                            .collect(),
                        Vec::new(),
                    );

                    vec(arguments, symbol.generic_arity).prop_map(move |args| {
                        let ty = Type::ResolvedPath {
                            path: symbol.path.clone(),
                            args,
                        };
                        FunctionSpec {
                            params: vec![Type::Ref {
                                is_mut: false,
                                inner: Box::new(ty.clone()),
                            }],
                            return_type: Some(ty),
                        }
                        .into_function(frag!(instantiate_ {&symbol.declaration_index}).print())
                    })
                })
                .collect::<Vec<_>>();

            (
                declarations,
                instantiations,
                vec(function_spec(symbols), 0..=MAX_FUNCTIONS),
            )
                .prop_map(|(mut declarations, instantiations, functions)| {
                    declarations.extend(instantiations);
                    declarations.extend(functions.into_iter().enumerate().map(
                        |(function_index, function)| {
                            function.into_function(frag!(function_ { function_index }).print())
                        },
                    ));
                    declarations
                })
        })
        .boxed()
}

fn declaration_spec() -> impl Strategy<Value = DeclarationSpec> {
    prop_oneof![
        (0..=MAX_GENERIC_PARAMETERS).prop_map(|generic_arity| DeclarationSpec {
            kind: DeclarationKind::Struct,
            generic_arity,
            is_clone: true,
        }),
        (0..=MAX_GENERIC_PARAMETERS, any::<bool>()).prop_map(|(generic_arity, is_clone)| {
            DeclarationSpec {
                kind: DeclarationKind::Enum,
                generic_arity,
                is_clone,
            }
        }),
    ]
}

fn declaration(
    spec: DeclarationSpec,
    declaration_index: usize,
    preceding_symbols: Vec<TypeSymbol>,
) -> BoxedStrategy<Item> {
    let name = frag!(Type { declaration_index }).print();
    let generic_params = (0..spec.generic_arity)
        .map(|index| frag!(T { index }).print())
        .collect::<Vec<_>>();
    let field_symbols = preceding_symbols
        .into_iter()
        .filter(|symbol| !spec.is_clone || symbol.is_clone)
        .collect();
    let field_type = owned_type(field_symbols, generic_params.clone());

    match spec.kind {
        DeclarationKind::Struct => shape(field_type, generic_params.clone(), true)
            .prop_map(move |shape| {
                Item::Struct(Struct {
                    name: name.clone(),
                    generic_params: generic_params.clone(),
                    shape,
                })
            })
            .boxed(),
        DeclarationKind::Enum => {
            let data = data_enum(
                name.clone(),
                generic_params.clone(),
                spec.is_clone,
                field_type,
            );

            if generic_params.is_empty() {
                prop_oneof![
                    1 => unit_enum(name, spec.is_clone),
                    2 => data,
                ]
                .prop_map(Item::Enum)
                .boxed()
            } else {
                data.prop_map(Item::Enum).boxed()
            }
        }
    }
}

fn primitive_owned_type() -> BoxedStrategy<Type> {
    prop::sample::select(OWNED_PRIMITIVES.to_vec())
        .prop_map(|name| Type::Primitive(name.to_owned()))
        .boxed()
}

fn owned_type(symbols: Vec<TypeSymbol>, generic_params: Vec<String>) -> BoxedStrategy<Type> {
    let mut leaves = vec![Type::ResolvedPath {
        path: "String".to_owned(),
        args: Vec::new(),
    }];
    leaves.extend(generic_params.into_iter().map(Type::GenericParameter));
    leaves.extend(
        symbols
            .iter()
            .filter(|symbol| symbol.generic_arity == 0)
            .map(|symbol| Type::ResolvedPath {
                path: symbol.path.clone(),
                args: Vec::new(),
            }),
    );

    let mut constructors = vec![
        ("Option".to_owned(), 1),
        ("Vec".to_owned(), 1),
        ("Result".to_owned(), 2),
    ];
    constructors.extend(
        symbols
            .into_iter()
            .filter(|symbol| symbol.generic_arity > 0)
            .map(|symbol| (symbol.path, symbol.generic_arity)),
    );

    prop_oneof![
        5 => primitive_owned_type(),
        3 => prop::sample::select(leaves),
    ]
    .prop_recursive(
        MAX_TYPE_DEPTH,
        MAX_TYPE_SIZE,
        MAX_GENERIC_PARAMETERS as u32,
        move |inner| {
            prop::sample::select(constructors.clone()).prop_flat_map(move |(path, arity)| {
                vec(inner.clone(), arity).prop_map(move |args| Type::ResolvedPath {
                    path: path.clone(),
                    args,
                })
            })
        },
    )
    .boxed()
}

fn parameter_type(symbols: Vec<TypeSymbol>) -> BoxedStrategy<Type> {
    let moved = owned_type(
        symbols
            .iter()
            .filter(|symbol| symbol.is_clone)
            .cloned()
            .collect(),
        Vec::new(),
    );
    let referent = prop_oneof![
        6 => owned_type(symbols, Vec::new()),
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
    (
        vec(parameter_type(symbols.clone()), 0..=MAX_PARAMETERS),
        prop::option::of(owned_type(symbols, Vec::new())),
    )
        .prop_map(|(params, return_type)| FunctionSpec {
            params,
            return_type,
        })
        .boxed()
}

fn shape(
    field_type: BoxedStrategy<Type>,
    generic_params: Vec<String>,
    allow_unit: bool,
) -> BoxedStrategy<Shape> {
    let allow_unit = allow_unit && generic_params.is_empty();
    let fields =
        vec(field_type, 0..=MAX_FIELDS - generic_params.len()).prop_map(move |mut fields| {
            fields.extend(generic_params.iter().cloned().map(Type::GenericParameter));
            fields
        });
    let tuple = fields.clone().prop_map(Shape::Tuple);
    let struct_ = fields.prop_map(|types| Shape::Struct {
        fields: types
            .into_iter()
            .enumerate()
            .map(|(index, ty)| ShapeField {
                name: frag!(field_ { index }).print(),
                ty,
            })
            .collect(),
    });

    if allow_unit {
        prop_oneof![Just(Shape::Unit), tuple, struct_,].boxed()
    } else {
        prop_oneof![tuple, struct_].boxed()
    }
}

fn unit_enum(name: String, is_clone: bool) -> BoxedStrategy<Enum> {
    (1usize..=MAX_ENUM_VARIANTS)
        .prop_map(move |variant_count| Enum {
            name: name.clone(),
            generic_params: Vec::new(),
            is_clone,
            variants: (0..variant_count)
                .map(|index| EnumVariant {
                    name: frag!(Variant { index }).print(),
                    shape: Shape::Unit,
                })
                .collect(),
        })
        .boxed()
}

fn data_enum(
    name: String,
    generic_params: Vec<String>,
    is_clone: bool,
    field_type: BoxedStrategy<Type>,
) -> BoxedStrategy<Enum> {
    (
        shape(field_type, generic_params.clone(), false),
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
                generic_params: generic_params.clone(),
                is_clone,
                variants,
            }
        })
        .boxed()
}
