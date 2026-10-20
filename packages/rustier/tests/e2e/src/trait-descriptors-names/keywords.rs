#[allow(non_camel_case_types)]
pub trait r#default {
    fn check(&self) -> bool;
}

#[allow(non_camel_case_types)]
pub trait r#type<T> {
    fn check(&self) -> bool;
}

impl r#default for super::Widget {
    fn check(&self) -> bool {
        true
    }
}

impl r#type<u32> for super::Widget {
    fn check(&self) -> bool {
        true
    }
}

pub trait Object {
    fn check(&self) -> bool;
}

pub trait Array<T> {
    fn check(&self) -> bool;
}

pub trait TypeError<T> {
    fn check(&self) -> bool;
}

#[allow(non_camel_case_types)]
pub trait globalThis<T> {
    fn check(&self) -> bool;
}

pub trait Symbol {
    fn check(&self) -> bool;
}

#[derive(Clone)]
pub struct Local(pub u32);

impl Object for Local {
    fn check(&self) -> bool {
        true
    }
}

impl Array<&super::keys::Seed> for Local {
    fn check(&self) -> bool {
        true
    }
}

impl TypeError<u32> for Local {
    fn check(&self) -> bool {
        true
    }
}

impl globalThis<u32> for Local {
    fn check(&self) -> bool {
        true
    }
}

impl Symbol for Local {
    fn check(&self) -> bool {
        true
    }
}
