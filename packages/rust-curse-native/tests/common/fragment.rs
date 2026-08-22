//! This test helper is a clone of `src/compiler/codegen/fragment.rs`.

use std::fmt::{self, Write as _};

#[derive(Clone, Debug)]
pub struct Printer {
    out: String,
    indent: usize,
    at_line_start: bool,
    new_lines: usize,
}

impl Default for Printer {
    fn default() -> Self {
        Self {
            out: String::new(),
            indent: 0,
            at_line_start: true,
            new_lines: 0,
        }
    }
}

impl Printer {
    fn write(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }

        let new_lines = std::mem::take(&mut self.new_lines);
        let existing_new_lines = self
            .out
            .bytes()
            .rev()
            .take_while(|byte| *byte == b'\n')
            .count()
            + text.bytes().take_while(|byte| *byte == b'\n').count();

        for _ in existing_new_lines..new_lines {
            self.out.push('\n');
            self.at_line_start = true;
        }

        for chunk in text.split_inclusive('\n') {
            if self.at_line_start && chunk != "\n" {
                for _ in 0..self.indent {
                    self.out.push('\t');
                }
            }

            self.out.push_str(chunk);
            self.at_line_start = chunk.ends_with('\n');
        }
    }
}

impl fmt::Write for Printer {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.write(text);
        Ok(())
    }
}

pub trait FragmentOnce {
    fn apply(self, printer: &mut Printer);
}

pub trait Fragment {
    fn apply_ref(&self, printer: &mut Printer);
}

#[derive(Clone)]
pub struct FnFragment<F>(F);

pub trait IntoFnFragment: FnOnce(&mut Printer) + Sized {
    fn into_fragment(self) -> FnFragment<Self> {
        FnFragment(self)
    }
}

impl<F> IntoFnFragment for F where F: FnOnce(&mut Printer) {}

impl<F> FragmentOnce for FnFragment<F>
where
    F: FnOnce(&mut Printer),
{
    fn apply(self, printer: &mut Printer) {
        (self.0)(printer);
    }
}

impl<F> Fragment for FnFragment<F>
where
    F: Fn(&mut Printer),
{
    fn apply_ref(&self, printer: &mut Printer) {
        (self.0)(printer);
    }
}

impl FragmentOnce for &(dyn Fn(&mut Printer) + '_) {
    fn apply(self, printer: &mut Printer) {
        self(printer);
    }
}

impl Fragment for &(dyn Fn(&mut Printer) + '_) {
    fn apply_ref(&self, printer: &mut Printer) {
        self(printer);
    }
}

impl<T> FragmentOnce for &T
where
    T: Fragment + ?Sized,
{
    fn apply(self, printer: &mut Printer) {
        self.apply_ref(printer);
    }
}

impl<T> Fragment for &T
where
    T: Fragment + ?Sized,
{
    fn apply_ref(&self, printer: &mut Printer) {
        (**self).apply_ref(printer);
    }
}

impl<T> FragmentOnce for Box<T>
where
    T: Fragment + ?Sized,
{
    fn apply(self, printer: &mut Printer) {
        self.apply_ref(printer);
    }
}

impl FragmentOnce for &str {
    fn apply(self, printer: &mut Printer) {
        printer.write(self);
    }
}

impl Fragment for &str {
    fn apply_ref(&self, printer: &mut Printer) {
        printer.write(self);
    }
}

impl FragmentOnce for String {
    fn apply(self, printer: &mut Printer) {
        printer.write(&self);
    }
}

impl Fragment for String {
    fn apply_ref(&self, printer: &mut Printer) {
        printer.write(self);
    }
}

macro_rules! impl_fragment_for_integer {
    ($($type:ty),+ $(,)?) => {
        $(
            impl FragmentOnce for $type {
                fn apply(self, printer: &mut Printer) {
                    write!(printer, "{self}").expect("writing to Printer cannot fail");
                }
            }

            impl Fragment for $type {
                fn apply_ref(&self, printer: &mut Printer) {
                    write!(printer, "{self}").expect("writing to Printer cannot fail");
                }
            }
        )+
    };
}

impl_fragment_for_integer!(
    i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize,
);

impl<T> FragmentOnce for Option<T>
where
    T: FragmentOnce,
{
    fn apply(self, printer: &mut Printer) {
        if let Some(fragment) = self {
            fragment.apply(printer);
        }
    }
}

impl<T> Fragment for Option<T>
where
    T: Fragment,
{
    fn apply_ref(&self, printer: &mut Printer) {
        if let Some(fragment) = self {
            fragment.apply_ref(printer);
        }
    }
}

impl FragmentOnce for () {
    fn apply(self, _printer: &mut Printer) {}
}

impl Fragment for () {
    fn apply_ref(&self, _printer: &mut Printer) {}
}

pub trait FragmentPrint: FragmentOnce + Sized {
    fn print(self) -> String;
}

impl<F: FragmentOnce> FragmentPrint for F {
    fn print(self) -> String {
        let mut printer = Printer::default();
        self.apply(&mut printer);
        while printer.out.ends_with('\n') {
            printer.out.pop();
        }
        printer.out
    }
}

macro_rules! impl_fragment_for_tuple {
    ($($type:ident.$index:tt),+ $(,)?) => {
        impl<$($type),+> FragmentOnce for ($($type,)+)
        where
            $($type: FragmentOnce,)+
        {
            fn apply(self, printer: &mut Printer) {
                $(self.$index.apply(printer);)+
            }
        }

        impl<$($type),+> Fragment for ($($type,)+)
        where
            $($type: Fragment,)+
        {
            fn apply_ref(&self, printer: &mut Printer) {
                $(self.$index.apply_ref(printer);)+
            }
        }
    };
}

impl_fragment_for_tuple!(A.0);
impl_fragment_for_tuple!(A.0, B.1);
impl_fragment_for_tuple!(A.0, B.1, C.2);
impl_fragment_for_tuple!(A.0, B.1, C.2, D.3);
impl_fragment_for_tuple!(A.0, B.1, C.2, D.3, E.4);
impl_fragment_for_tuple!(A.0, B.1, C.2, D.3, E.4, F.5);
impl_fragment_for_tuple!(A.0, B.1, C.2, D.3, E.4, F.5, G.6);
impl_fragment_for_tuple!(A.0, B.1, C.2, D.3, E.4, F.5, G.6, H.7);
impl_fragment_for_tuple!(A.0, B.1, C.2, D.3, E.4, F.5, G.6, H.7, I.8);

pub fn fragment<F>(f: F) -> FnFragment<F>
where
    F: FnOnce(&mut Printer),
{
    f.into_fragment()
}

pub struct Blank;

impl FragmentOnce for Blank {
    fn apply(self, printer: &mut Printer) {
        printer.new_lines += 1;
    }
}

impl Fragment for Blank {
    fn apply_ref(&self, printer: &mut Printer) {
        printer.new_lines += 1;
    }
}

pub fn blank() -> Blank {
    Blank
}

pub struct Indented<F> {
    fragment: F,
}

impl<F> FragmentOnce for Indented<F>
where
    F: FragmentOnce,
{
    fn apply(self, printer: &mut Printer) {
        printer.indent += 1;
        self.fragment.apply(printer);
        printer.indent -= 1;
    }
}

impl<F> Fragment for Indented<F>
where
    F: Fragment,
{
    fn apply_ref(&self, printer: &mut Printer) {
        printer.indent += 1;
        self.fragment.apply_ref(printer);
        printer.indent -= 1;
    }
}

pub fn idented<F>(fragment: F) -> Indented<F>
where
    F: FragmentOnce,
{
    Indented { fragment }
}

pub struct Delimited<F> {
    opening: &'static str,
    fragment: F,
    closing: &'static str,
}

impl<F> FragmentOnce for Delimited<F>
where
    F: FragmentOnce,
{
    fn apply(self, printer: &mut Printer) {
        printer.write(self.opening);
        self.fragment.apply(printer);
        printer.write(self.closing);
    }
}

impl<F> Fragment for Delimited<F>
where
    F: Fragment,
{
    fn apply_ref(&self, printer: &mut Printer) {
        printer.write(self.opening);
        self.fragment.apply_ref(printer);
        printer.write(self.closing);
    }
}

pub fn curlies<F>(fragment: F) -> Delimited<Indented<F>>
where
    F: FragmentOnce,
{
    Delimited {
        opening: "{\n",
        fragment: idented(fragment),
        closing: "\n}",
    }
}

pub fn parens<F>(fragment: F) -> Delimited<Indented<F>>
where
    F: FragmentOnce,
{
    Delimited {
        opening: "(\n",
        fragment: idented(fragment),
        closing: "\n)",
    }
}

pub fn squares<F>(fragment: F) -> Delimited<F>
where
    F: FragmentOnce,
{
    Delimited {
        opening: "[",
        fragment,
        closing: "]",
    }
}

pub struct CommaList<I> {
    fragments: I,
}

impl<I> FragmentOnce for CommaList<I>
where
    I: Iterator,
    I::Item: FragmentOnce,
{
    fn apply(self, printer: &mut Printer) {
        for (index, fragment) in self.fragments.enumerate() {
            if index > 0 {
                printer.write(",\n");
            }
            fragment.apply(printer);
        }
    }
}

impl<I> Fragment for CommaList<I>
where
    I: Iterator + Clone,
    I::Item: FragmentOnce,
{
    fn apply_ref(&self, printer: &mut Printer) {
        for (index, fragment) in self.fragments.clone().enumerate() {
            if index > 0 {
                printer.write(",\n");
            }
            fragment.apply(printer);
        }
    }
}

pub fn comma_list<F>(fragments: F) -> CommaList<F::IntoIter>
where
    F: IntoIterator,
    F::Item: FragmentOnce,
{
    CommaList {
        fragments: fragments.into_iter(),
    }
}

#[macro_export]
macro_rules! frag {
    (@nest) => { () };
    (@nest $single:expr_2021,) => { $single };
    (@nest $first:expr_2021, $($rest:expr_2021,)+) => {
        ($first, frag!(@nest $($rest,)+))
    };
    (@collect [$($parts:expr_2021,)*] []) => {
        frag!(@nest $($parts,)*)
    };
    (@collect [$($parts:expr_2021,)*] [$($text:tt)+]) => {
        frag!(@nest $($parts,)* stringify!($($text)+),)
    };
    (@collect [$($parts:expr_2021,)*] [] {{ $($inner:tt)* }} $($rest:tt)*) => {
        frag!(@collect [
            $($parts,)*
            $crate::common::fragment::curlies(frag!($($inner)*)),
        ] [] $($rest)*)
    };
    (@collect [$($parts:expr_2021,)*] [$($text:tt)+] {{ $($inner:tt)* }} $($rest:tt)*) => {
        frag!(@collect [
            $($parts,)*
            stringify!($($text)+),
            $crate::common::fragment::curlies(frag!($($inner)*)),
        ] [] $($rest)*)
    };
    (@collect [$($parts:expr_2021,)*] [] [$($inner:tt)*] $($rest:tt)*) => {
        frag!(@collect [
            $($parts,)*
            $crate::common::fragment::squares(frag!($($inner)*)),
        ] [] $($rest)*)
    };
    (@collect [$($parts:expr_2021,)*] [$($text:tt)+] [$($inner:tt)*] $($rest:tt)*) => {
        frag!(@collect [
            $($parts,)*
            stringify!($($text)+),
            $crate::common::fragment::squares(frag!($($inner)*)),
        ] [] $($rest)*)
    };
    (@collect [$($parts:expr_2021,)*] [] {$expr:expr_2021} $($rest:tt)*) => {
        frag!(@collect [$($parts,)* $expr,] [] $($rest)*)
    };
    (@collect [$($parts:expr_2021,)*] [$($text:tt)+] {$expr:expr_2021} $($rest:tt)*) => {
        frag!(@collect [
            $($parts,)*
            stringify!($($text)+),
            $expr,
        ] [] $($rest)*)
    };
    (@collect [$($parts:expr_2021,)*] [] () $($rest:tt)*) => {
        frag!(@collect [$($parts,)* "()",] [] $($rest)*)
    };
    (@collect [$($parts:expr_2021,)*] [$($text:tt)+] () $($rest:tt)*) => {
        frag!(@collect [
            $($parts,)*
            stringify!($($text)+),
            "()",
        ] [] $($rest)*)
    };
    (@collect [$($parts:expr_2021,)*] [] ($($inner:tt)+) $($rest:tt)*) => {
        frag!(@collect [
            $($parts,)*
            $crate::common::fragment::parens(frag!($($inner)*)),
        ] [] $($rest)*)
    };
    (@collect [$($parts:expr_2021,)*] [$($text:tt)+] ($($inner:tt)+) $($rest:tt)*) => {
        frag!(@collect [
            $($parts,)*
            stringify!($($text)+),
            $crate::common::fragment::parens(frag!($($inner)*)),
        ] [] $($rest)*)
    };
    (@collect [$($parts:expr_2021,)*] [] ~ $($rest:tt)*) => {
        frag!(@collect [$($parts,)* " ",] [] $($rest)*)
    };
    (@collect [$($parts:expr_2021,)*] [$($text:tt)+] ~ $($rest:tt)*) => {
        frag!(@collect [
            $($parts,)*
            stringify!($($text)+),
            " ",
        ] [] $($rest)*)
    };
    (@collect [$($parts:expr_2021,)*] [] @ $($rest:tt)*) => {
        frag!(@collect [
            $($parts,)*
            $crate::common::fragment::blank(),
        ] [] $($rest)*)
    };
    (@collect [$($parts:expr_2021,)*] [$($text:tt)+] @ $($rest:tt)*) => {
        frag!(@collect [
            $($parts,)*
            stringify!($($text)+),
            $crate::common::fragment::blank(),
        ] [] $($rest)*)
    };
    (@collect [$($parts:expr_2021,)*] [$($text:tt)*] $token:tt $($rest:tt)*) => {
        frag!(@collect [$($parts,)*] [$($text)* $token] $($rest)*)
    };
    ($($input:tt)*) => {
        frag!(@collect [] [] $($input)*)
    };
}
