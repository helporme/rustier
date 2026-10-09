use crate::{
    common::fragment::{Fragment, FragmentOnce, Printer, comma_list, fragment},
    frag,
};

#[derive(Debug, Clone)]
pub enum Type {
    Primitive(String),
    GenericParameter(String),
    ResolvedPath { path: String, args: Vec<Type> },
    Ref { is_mut: bool, inner: Box<Type> },
}

impl Fragment for Type {
    fn apply_ref(&self, p: &mut Printer) {
        match self {
            Self::Primitive(name) | Self::GenericParameter(name) => name.apply(p),
            Self::ResolvedPath { path, args } => {
                path.apply(p);

                if !args.is_empty() {
                    frag!(<{comma_list(args)}>).apply(p);
                }
            }
            Self::Ref { is_mut, inner } => {
                let inner: &Type = inner;

                if *is_mut {
                    frag!(&mut ~{ inner }).apply(p);
                } else {
                    frag!(&{ inner }).apply(p);
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub params: Vec<FunctionParam>,
    pub return_type: Option<Type>,
}

impl Fragment for Function {
    fn apply_ref(&self, p: &mut Printer) {
        let params = comma_list(&self.params);

        let body = fragment(|p| {
            for parameter in &self.params {
                frag!(let _ = {&parameter.name};@).apply(p);
            }
            frag!(todo!()).apply(p);
        });

        match &self.return_type {
            Some(return_type) => {
                frag!(
                    pub fn ~{&self.name}({params})~ -> ~{&return_type}~{{
                        {body}
                    }}
                )
                .apply(p);
            }
            None => {
                frag!(
                    pub fn ~{&self.name}({params})~ {{
                        {body}
                    }}
                )
                .apply(p);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct FunctionParam {
    pub name: String,
    pub ty: Type,
}

impl Fragment for FunctionParam {
    fn apply_ref(&self, p: &mut Printer) {
        frag!({&self.name}: ~{&self.ty}).apply(p);
    }
}

#[derive(Debug, Clone)]
pub enum Shape {
    Unit,
    Tuple(Vec<Type>),
    Struct { fields: Vec<ShapeField> },
}

#[derive(Debug, Clone)]
pub struct ShapeField {
    pub name: String,
    pub ty: Type,
}

#[derive(Debug, Clone)]
pub struct Struct {
    pub name: String,
    pub generic_params: Vec<String>,
    pub shape: Shape,
}

impl Fragment for Struct {
    fn apply_ref(&self, p: &mut Printer) {
        "#[derive(Clone)]\n".apply(p);

        let name = fragment(|p| {
            self.name.apply_ref(p);

            if !self.generic_params.is_empty() {
                frag!(<{comma_list(&self.generic_params)}>).apply(p);
            }
        });

        match &self.shape {
            Shape::Unit => {
                frag!(
                    pub struct ~{name};
                )
                .apply(p);
            }
            Shape::Tuple(fields) => {
                let fields = comma_list(fields.iter().map(|field| frag!(pub ~{field})));

                frag!(
                    pub struct ~{name}({fields});
                )
                .apply(p);
            }
            Shape::Struct { fields } => {
                let fields = comma_list(
                    fields
                        .iter()
                        .map(|field| frag!(pub ~{&field.name}: ~{&field.ty})),
                );

                frag!(
                    pub struct ~{name} ~{{
                        {fields}
                    }}
                )
                .apply(p);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct EnumVariant {
    pub name: String,
    pub shape: Shape,
}

impl Fragment for EnumVariant {
    fn apply_ref(&self, p: &mut Printer) {
        match &self.shape {
            Shape::Unit => {
                frag!(
                    ~{&self.name}
                )
                .apply(p);
            }
            Shape::Tuple(fields) => {
                let fields = comma_list(fields.iter().map(|field| frag!(~{field})));

                frag!(
                    ~{&self.name}({fields})
                )
                .apply(p);
            }
            Shape::Struct { fields } => {
                let fields = comma_list(
                    fields
                        .iter()
                        .map(|field| frag!(~{&field.name}: ~{&field.ty})),
                );

                frag!(
                    ~{&self.name} ~{{
                        {fields}
                    }}
                )
                .apply(p);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct Enum {
    pub name: String,
    pub generic_params: Vec<String>,
    pub is_clone: bool,
    pub variants: Vec<EnumVariant>,
}

impl Fragment for Enum {
    fn apply_ref(&self, p: &mut Printer) {
        if self.is_clone {
            "#[derive(Clone)]\n".apply(p);
        }

        let name = fragment(|p| {
            self.name.apply_ref(p);

            if !self.generic_params.is_empty() {
                frag!(<{comma_list(&self.generic_params)}>).apply(p);
            }
        });

        frag!(
            pub enum ~{name}~{{
                {comma_list(self.variants.iter())}
            }}
        )
        .apply(p);
    }
}

#[derive(Debug, Clone)]
pub enum Item {
    Function(Function),
    Mod(Module),
    Struct(Struct),
    Enum(Enum),
}

impl Fragment for Item {
    fn apply_ref(&self, p: &mut Printer) {
        match self {
            Item::Function(function) => function.apply(p),
            Item::Mod(module) => module.apply(p),
            Item::Struct(struct_) => struct_.apply(p),
            Item::Enum(enum_) => enum_.apply(p),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Module {
    pub name: String,
    pub items: Vec<Item>,
}

impl Fragment for Module {
    fn apply_ref(&self, p: &mut Printer) {
        let items = fragment(|p| {
            for item in &self.items {
                item.apply(p);
                frag!(@@).apply(p);
            }
        });

        frag!(
            pub mod ~{&self.name}~{{
                {items}
            }}
        )
        .apply(p);
    }
}

#[derive(Debug, Clone)]
pub struct Program {
    pub items: Vec<Item>,
}

impl Program {
    pub fn exported_functions(&self) -> Vec<Vec<String>> {
        let mut paths = Vec::new();
        let mut pending = vec![(&self.items, Vec::new())];

        while let Some((items, path)) = pending.pop() {
            for item in items {
                match item {
                    Item::Function(function) => {
                        let mut path = path.clone();
                        path.push(function.name.clone());
                        paths.push(path);
                    }
                    Item::Mod(module) => {
                        let mut path = path.clone();
                        path.push(module.name.clone());
                        pending.push((&module.items, path));
                    }
                    Item::Struct(_) | Item::Enum(_) => {}
                }
            }
        }

        paths
    }
}

impl Fragment for Program {
    fn apply_ref(&self, p: &mut Printer) {
        for item in &self.items {
            item.apply(p);
            frag!(@@).apply(p);
        }
    }
}
