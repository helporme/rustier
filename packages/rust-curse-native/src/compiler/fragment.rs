//! Allocation-free fragments for generating any source text. Types implementing
//! the [`FragmentOnce`] or [`Fragment`] traits replace [`String`] or [`str`],
//! and the [`frag`] macro replaces [`format`].
//!
//! Example:
//!
//! ```
//! # use rust_curse::codegen::fragment::{frag, FragmentPrint};
//! #
//! let name = "world";
//! let greeting = frag!(hello ~ {name}); // No allocation!
//!
//! assert_eq!(greeting.print(), "hello world"); // print() allocates
//! ```
//!
//! ---
//!
//! # Overview
//!
//! [`FragmentOnce`] or [`Fragment`] are simple traits that represent an action
//! of pushing a piece of text to a [`String`], once or many times,
//! respectively.
//!
//! Some common types implement these traits, such as [`String`], `&str`, or
//! [`Option`]. The utility comes from composing these simple fragments into
//! more complex ones, thereby representing a _fragment_ of source code.
//!
//! ## Function composition
//!
//! One way of composing a fragment is to use the [`fragment`] function that
//! converts a closure to a fragment by directly implementing the
//! [`FragmentOnce`] trait. For example:
//!
//! ```
//! # use rust_curse::codegen::fragment::{fragment, FragmentOnce, FragmentPrint};
//! #
//! let hello = "Hello ";
//! let greeting = fragment(move |p| {
//!     hello.apply(p);
//!     ("world", "!").apply(p);
//! });
//! assert_eq!(greeting.print(), "Hello world!");
//! ```
//!
//! You may notice that we do not push the text to the [`String`] directly,
//! but instead use [`FragmentOnce::apply`] to apply the fragment to a
//! [`Printer`]. Printer is a simple wrapper around [`String`] that keeps track
//! of indentation and line breaks, so generated source code is a bit easier to
//! read.
//!
//! ## Tuple composition
//!
//! Another way of composing a fragment is to combine fragments together using
//! tuples like `("Hello ", Some("world"), "!")`. Any tuple of reasonable size
//! that consists of types implementing either fragment trait is also a
//! fragment.
//!
//! Usually, though, you do not want to manually type tuples to combine code.
//! This is where the `frag` macro comes to help you. It accepts source-like
//! syntax and automatically converts it into a fragment.
//!
//! ```
//! # use rust_curse::codegen::fragment::{frag, FragmentPrint};
//! #
//! let name = "%username%";
//! let greeting = frag!(
//!     Hello ~{name}!
//! );
//! assert_eq!(greeting.print(), "Hello %username%!");
//! ```
//!
//! Check the [`frag`] macro documentation for more details about interpolations
//! and other syntax.

use std::fmt::{self, Write as _};

/// Accumulates fragment output while applying basic indentation.
///
/// `Printer` is normally created by [`FragmentPrint::print`], but fragments can
/// also be applied to one directly.
///
/// ```
/// # use rust_curse::codegen::fragment::{FragmentOnce, Printer};
/// #
/// let mut printer = Printer::default();
/// "hello".apply(&mut printer);
/// ```
#[derive(Clone, Debug)]
pub struct Printer {
    out: String,
    indent: usize,
    at_line_start: bool,
    new_lines: usize,
}

impl Default for Printer {
    /// Creates an empty printer positioned at the start of an unindented line.
    ///
    /// ```
    /// # use rust_curse::codegen::fragment::{FragmentOnce, Printer};
    /// #
    /// let mut printer = Printer::default();
    /// "hello".apply(&mut printer);
    /// ```
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
    /// Appends text and inserts the current indentation at each non-empty line
    /// start.
    ///
    /// ```
    /// # use rust_curse::codegen::fragment::{idented, FragmentPrint};
    /// #
    /// assert_eq!(idented("one\ntwo").print(), "\tone\n\ttwo");
    /// ```
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
    /// Appends a string using the printer's indentation rules.
    ///
    /// ```
    /// # use rust_curse::codegen::fragment::{FragmentPrint, IntoFnFragment, Printer};
    /// # use std::fmt::Write as _;
    /// #
    /// let fragment = (|printer: &mut Printer| {
    ///     printer.write_str("hello").unwrap();
    /// })
    /// .into_fragment();
    ///
    /// assert_eq!(fragment.print(), "hello");
    /// ```
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.write(text);
        Ok(())
    }
}

/// A trait for writing once a piece of source code to the [`Printer`].
///
/// ```
/// # use rust_curse::codegen::fragment::{fragment, FragmentOnce, FragmentPrint};
/// #
/// let text = String::from("owned");
/// let fragment = fragment(move |printer| text.apply(printer));
///
/// assert_eq!(fragment.print(), "owned");
/// ```
pub trait FragmentOnce {
    /// Consumes this fragment and appends it to `printer`.
    fn apply(self, printer: &mut Printer);
}

/// A trait for writing a piece of source code to the [`Printer`] multiple
/// times.
///
/// This object-safe trait is intended for the uncommon cases that need reuse
/// or type erasure. Every `Fragment` can also be consumed as a
/// [`FragmentOnce`]. Normal fragment composition should use [`FragmentOnce`].
///
/// ```
/// # use rust_curse::codegen::fragment::{Fragment, FragmentPrint};
/// #
/// fn print_fragment(fragment: impl Fragment) -> String {
///     fragment.print()
/// }
///
/// let text = String::from("reusable");
/// let fragment: &dyn Fragment = &text;
///
/// assert_eq!(print_fragment(String::from("owned")), "owned");
/// assert_eq!(fragment.print(), "reusable");
/// assert_eq!(fragment.print(), "reusable");
/// ```
pub trait Fragment {
    /// Appends this fragment to `printer` without consuming it.
    fn apply_ref(&self, printer: &mut Printer);
}

/// Wraps a printing function as a [`FragmentOnce`].
///
/// ```
/// # use rust_curse::codegen::fragment::{FragmentOnce, FragmentPrint, IntoFnFragment, Printer};
/// #
/// let fragment = (|printer: &mut Printer| {
///     "generated".apply(printer);
/// })
/// .into_fragment();
///
/// assert_eq!(fragment.print(), "generated");
/// ```
#[derive(Clone)]
pub struct FnFragment<F>(F);

impl<F> From<F> for FnFragment<F>
where
    F: FnOnce(&mut Printer),
{
    fn from(f: F) -> Self {
        Self(f)
    }
}

/// Converts a printing function into a [`FragmentOnce`].
pub trait IntoFnFragment: FnOnce(&mut Printer) + Sized {
    /// Wraps this function in a [`FnFragment`].
    ///
    /// ```
    /// # use rust_curse::codegen::fragment::{FragmentPrint, IntoFnFragment, Printer};
    /// # use std::fmt::Write as _;
    /// #
    /// let fragment = (|printer: &mut Printer| {
    ///     printer.write_str("callback").unwrap();
    /// })
    /// .into_fragment();
    ///
    /// assert_eq!(fragment.print(), "callback");
    /// ```
    fn into_fragment(self) -> FnFragment<Self> {
        self.into()
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
    /// ```
    /// # use rust_curse::codegen::fragment::FragmentPrint;
    /// #
    /// assert_eq!("slice".print(), "slice");
    /// ```
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
    /// ```
    /// # use rust_curse::codegen::fragment::FragmentPrint;
    /// #
    /// assert_eq!(String::from("owned").print(), "owned");
    /// ```
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
    /// Applies the contained fragment or leaves `printer` unchanged for `None`.
    ///
    /// ```
    /// # use rust_curse::codegen::fragment::FragmentPrint;
    /// #
    /// assert_eq!(Some("present").print(), "present");
    /// assert_eq!(None::<&str>.print(), "");
    /// ```
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
    /// Leaves `printer` unchanged.
    ///
    /// ```
    /// # use rust_curse::codegen::fragment::FragmentPrint;
    /// #
    /// assert_eq!(().print(), "");
    /// ```
    fn apply_ref(&self, _printer: &mut Printer) {}
}

/// Renders a [`FragmentOnce`] into an owned string.
///
/// ```
/// # use rust_curse::codegen::fragment::FragmentPrint;
/// #
/// assert_eq!(("hello", " ", "world").print(), "hello world");
/// ```
pub trait FragmentPrint: FragmentOnce + Sized {
    /// Consumes this fragment and prints it to a string.
    fn print(self) -> String;
}

impl<F: FragmentOnce> FragmentPrint for F {
    /// Applies this fragment to a fresh printer and returns its output.
    ///
    /// ```
    /// # use rust_curse::codegen::fragment::FragmentPrint;
    /// #
    /// assert_eq!(("a", "b", "c").print(), "abc");
    /// ```
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
            /// Appends each tuple element in order.
            ///
            /// ```
            /// # use rust_curse::codegen::fragment::FragmentPrint;
            /// #
            /// assert_eq!(("one", "two").print(), "onetwo");
            /// ```
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

/// Converts a printing closure into a composable [`FnFragment`].
///
/// ```
/// # use rust_curse::codegen::fragment::{fragment, FragmentOnce, FragmentPrint};
/// #
/// let greeting = fragment(|p| {
///     "Hello, ".apply(p);
///     "world!".apply(p);
/// });
///
/// assert_eq!(greeting.print(), "Hello, world!");
/// ```
pub fn fragment<F>(f: F) -> FnFragment<F>
where
    F: FnOnce(&mut Printer),
{
    f.into_fragment()
}

/// A reusable deferred newline fragment.
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

/// Inserts a newline separator if another non-empty fragment follows it.
///
/// Existing newlines at either side of the boundary count toward the requested
/// separator, and repeated calls request additional newlines.
///
/// ```
/// # use rust_curse::codegen::fragment::{blank, FragmentPrint};
/// #
/// assert_eq!(("one", blank(), "two").print(), "one\ntwo");
/// assert_eq!(("one\n", blank(), "two").print(), "one\ntwo");
/// assert_eq!(("one", blank(), blank(), "two").print(), "one\n\ntwo");
/// assert_eq!(("one", blank()).print(), "one");
/// ```
pub fn blank() -> Blank {
    Blank
}

/// A fragment that increases indentation while rendering its inner fragment.
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

/// Increases indentation while rendering the given fragment.
///
/// The returned fragment is reusable when the input implements [`Fragment`].
///
/// ```
/// # use rust_curse::codegen::fragment::{idented, Fragment, FragmentPrint};
/// #
/// fn require_fragment(_: impl Fragment) {}
///
/// assert_eq!(idented("one\ntwo").print(), "\tone\n\ttwo");
/// require_fragment(idented(String::from("reusable")));
/// ```
pub fn idented<F>(fragment: F) -> Indented<F>
where
    F: FragmentOnce,
{
    Indented { fragment }
}

/// A fragment surrounded by static opening and closing delimiters.
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

/// Wraps the given fragment in curly braces with indentation.
///
/// ```
/// # use rust_curse::codegen::fragment::{curlies, FragmentPrint};
/// #
/// assert_eq!(curlies("value").print(), "{\n\tvalue\n}");
/// ```
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

/// Wraps the given fragment in parentheses with indentation.
///
/// ```
/// # use rust_curse::codegen::fragment::{parens, FragmentPrint};
/// #
/// assert_eq!(parens("value").print(), "(\n\tvalue\n)");
/// ```
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

/// Wraps the given fragment in square brackets.
///
/// ```
/// # use rust_curse::codegen::fragment::{squares, FragmentPrint};
/// #
/// assert_eq!(squares("value").print(), "[value]");
/// ```
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

/// Wraps the given fragment in single quotes.
///
/// ```
/// # use rust_curse::codegen::fragment::{singles, FragmentPrint};
/// #
/// assert_eq!(singles("value").print(), "'value'");
/// ```
pub fn singles<F>(fragment: F) -> Delimited<F>
where
    F: FragmentOnce,
{
    Delimited {
        opening: "'",
        fragment,
        closing: "'",
    }
}

/// Wraps the given fragment in double quotes.
///
/// ```
/// # use rust_curse::codegen::fragment::{doubles, FragmentPrint};
/// #
/// assert_eq!(doubles("value").print(), "\"value\"");
/// ```
pub fn doubles<F>(fragment: F) -> Delimited<F>
where
    F: FragmentOnce,
{
    Delimited {
        opening: "\"",
        fragment,
        closing: "\"",
    }
}

/// A fragment iterator joined with a comma and a newline.
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

/// Joins fragments with a comma and a newline.
///
/// The returned fragment is reusable when the input's iterator is cloneable.
///
/// ```
/// # use rust_curse::codegen::fragment::{comma_list, Fragment, FragmentPrint};
/// #
/// fn require_fragment(_: impl Fragment) {}
///
/// assert_eq!(
///     comma_list(["one", "two", "three"]).print(),
///     "one,\ntwo,\nthree"
/// );
/// assert_eq!(comma_list(Vec::<&str>::new()).print(), "");
/// require_fragment(comma_list(["one", "two"]));
/// ```
pub fn comma_list<F>(fragments: F) -> CommaList<F::IntoIter>
where
    F: IntoIterator,
    F::Item: FragmentOnce,
{
    CommaList {
        fragments: fragments.into_iter(),
    }
}

/// A fragment iterator joined with one blank line between items.
pub struct Spaced<I> {
    fragments: I,
}

impl<I> FragmentOnce for Spaced<I>
where
    I: Iterator,
    I::Item: FragmentOnce,
{
    fn apply(self, printer: &mut Printer) {
        for (index, fragment) in self.fragments.enumerate() {
            if index > 0 && !printer.out.ends_with("\n\n") {
                if !printer.at_line_start {
                    printer.write("\n");
                }
                printer.write("\n");
            }

            fragment.apply(printer);
        }
    }
}

impl<I> Fragment for Spaced<I>
where
    I: Iterator + Clone,
    I::Item: FragmentOnce,
{
    fn apply_ref(&self, printer: &mut Printer) {
        for (index, fragment) in self.fragments.clone().enumerate() {
            if index > 0 && !printer.out.ends_with("\n\n") {
                if !printer.at_line_start {
                    printer.write("\n");
                }
                printer.write("\n");
            }

            fragment.apply(printer);
        }
    }
}

/// Joins fragments with one blank line between them.
///
/// The returned fragment is reusable when the input's iterator is cloneable.
/// If the previous fragment does not end with a newline, `spaced` inserts two;
/// if it ends with one, `spaced` inserts one more. Existing blank lines are
/// preserved.
///
/// ```
/// # use rust_curse::codegen::fragment::{spaced, Fragment, FragmentPrint};
/// #
/// fn require_fragment(_: impl Fragment) {}
///
/// assert_eq!(spaced(["one", "two"]).print(), "one\n\ntwo");
/// assert_eq!(spaced(["one\n", "two"]).print(), "one\n\ntwo");
/// assert_eq!(spaced(["one\n\n", "two"]).print(), "one\n\ntwo");
/// assert_eq!(spaced(Vec::<&str>::new()).print(), "");
/// require_fragment(spaced(["one", "two"]));
/// ```
pub fn spaced<F>(fragments: F) -> Spaced<F::IntoIter>
where
    F: IntoIterator,
    F::Item: FragmentOnce,
{
    Spaced {
        fragments: fragments.into_iter(),
    }
}

/// Builds a fragment from source-like text supporting interpolation.
///
/// ```
/// # use rust_curse::codegen::fragment::{frag, FragmentPrint};
/// #
/// let name = "world";
/// let greeting = frag!(hello ~ {name} @ goodbye);
///
/// assert_eq!(greeting.print(), "hello world\ngoodbye");
/// ```
///
/// # Special syntax
///
/// ## Escaping curly braces
/// Double braces are are escaped.
///
/// ```
/// # use rust_curse::codegen::fragment::{frag, FragmentPrint};
/// #
/// let field = "value";
/// let struct_ = frag!(struct Foo ~{{ {field}: i32 }});
///
/// assert_eq!(struct_.print(), "struct Foo {\n\tvalue: i32\n}");
/// ```
///
/// ## Space insertion (`~`)
/// Use `~` to insert a space between static tokens (e.g text) and
/// parentheses or curly braces. While working, it's not neccessary to use `~`
/// between static tokens; the macro will insert a space automatically.
///
/// ```
/// # use rust_curse::codegen::fragment::{frag, FragmentPrint};
/// #
/// assert_eq!(frag!(hello ~ world).print(), "hello world");
/// assert_eq!(frag!(hello world).print(), "hello world");
///
/// let world = "world";
/// assert_eq!(frag!(hello {world}).print(), "helloworld");
/// assert_eq!(frag!(hello ~{world}).print(), "hello world");
///
/// assert_eq!(frag!(hello (world)).print(), "hello(\n\tworld\n)");
/// assert_eq!(frag!(hello~(world)).print(), "hello (\n\tworld\n)");
/// ```
///
/// ## Newline insertion (`@`)
/// Use `@` to insert a newline if non-empty text follows it.
///
/// ```
/// # use rust_curse::codegen::fragment::{frag, FragmentPrint};
/// #
/// let name = "world";
/// let greeting = frag!(
///     hello ~ {name}@
///     goodbye
/// );
///
/// assert_eq!(greeting.print(), "hello world\ngoodbye");
/// ```
///
/// # Auto formatting
///
/// Double curly braces, parentheses, and square brackets are automatically
/// wrapped with [`curlies`](crate::codegen::fragment::curlies),
/// [`parens`](crate::codegen::fragment::parens), and
/// [`squares`](crate::codegen::fragment::squares), respectively.
///
/// ```
/// # use rust_curse::codegen::fragment::{frag, FragmentPrint};
/// #
/// let function = frag!(
///     fn greet(name: &str) ~{{
///         hello
///     }}
/// );
///
/// assert_eq!(function.print(), "fn greet(\n\tname: &str\n) {\n\thello\n}");
///
/// let index = "0";
/// assert_eq!(frag!(values[{index}]).print(), "values[0]");
/// ```
#[macro_export]
macro_rules! frag {
    (@nest) => {
        ()
    };

    (@nest $single:expr_2021,) => {
        $single
    };

    (@nest $first:expr_2021, $($rest:expr_2021,)+) => {
        ($first, frag!(@nest $($rest,)+))
    };

    // End with no pending text.
    (@collect [$($parts:expr_2021,)*] []) => {
        frag!(@nest $($parts,)*)
    };

    // End with pending text.
    (@collect [$($parts:expr_2021,)*] [$($text:tt)+]) => {
        frag!(@nest $($parts,)* stringify!($($text)+),)
    };

    // Curly fragment with no pending text.
    (@collect [$($parts:expr_2021,)*] [] {{ $($inner:tt)* }} $($rest:tt)*) => {
        frag!(@collect [
            $($parts,)*
            $crate::compiler::fragment::curlies(frag!($($inner)*)),
        ] [] $($rest)*)
    };

    // Curly fragment after some static tokens.
    (@collect [$($parts:expr_2021,)*] [$($text:tt)+] {{ $($inner:tt)* }} $($rest:tt)*) => {
        frag!(@collect [
            $($parts,)*
            stringify!($($text)+),
            $crate::codegen::fragment::curlies(frag!($($inner)*)),
        ] [] $($rest)*)
    };

    // Square fragment with no pending text.
    (@collect [$($parts:expr_2021,)*] [] [$($inner:tt)*] $($rest:tt)*) => {
        frag!(@collect [
            $($parts,)*
            $crate::compiler::fragment::squares(frag!($($inner)*)),
        ] [] $($rest)*)
    };

    // Square fragment after some static tokens.
    (@collect [$($parts:expr_2021,)*] [$($text:tt)+] [$($inner:tt)*] $($rest:tt)*) => {
        frag!(@collect [
            $($parts,)*
            stringify!($($text)+),
            $crate::compiler::fragment::squares(frag!($($inner)*)),
        ] [] $($rest)*)
    };

    // Interpolation with no pending text.
    (@collect [$($parts:expr_2021,)*] [] {$expr:expr_2021} $($rest:tt)*) => {
        frag!(@collect [$($parts,)* $expr,] [] $($rest)*)
    };

    // Interpolation after some static tokens.
    (@collect [$($parts:expr_2021,)*] [$($text:tt)+] {$expr:expr_2021} $($rest:tt)*) => {
        frag!(@collect [
            $($parts,)*
            stringify!($($text)+),
            $expr,
        ] [] $($rest)*)
    };

    // Empty parentheses with no pending text.
    (@collect [$($parts:expr_2021,)*] [] () $($rest:tt)*) => {
        frag!(@collect [$($parts,)* "()",] [] $($rest)*)
    };

    // Empty parentheses after some static tokens.
    (@collect [$($parts:expr_2021,)*] [$($text:tt)+] () $($rest:tt)*) => {
        frag!(@collect [
            $($parts,)*
            stringify!($($text)+),
            "()",
        ] [] $($rest)*)
    };

    // Parenthesized fragment with no pending text.
    (@collect [$($parts:expr_2021,)*] [] ($($inner:tt)+) $($rest:tt)*) => {
        frag!(@collect [
            $($parts,)*
            $crate::compiler::fragment::parens(frag!($($inner)*)),
        ] [] $($rest)*)
    };

    // Parenthesized fragment after some static tokens.
    (@collect [$($parts:expr_2021,)*] [$($text:tt)+] ($($inner:tt)+) $($rest:tt)*) => {
        frag!(@collect [
            $($parts,)*
            stringify!($($text)+),
            $crate::compiler::fragment::parens(frag!($($inner)*)),
        ] [] $($rest)*)
    };

    // Explicit space with no pending text.
    (@collect [$($parts:expr_2021,)*] [] ~ $($rest:tt)*) => {
        frag!(@collect [$($parts,)* " ",] [] $($rest)*)
    };

    // Explicit space after static tokens.
    (@collect [$($parts:expr_2021,)*] [$($text:tt)+] ~ $($rest:tt)*) => {
        frag!(@collect [
            $($parts,)*
            stringify!($($text)+),
            " ",
        ] [] $($rest)*)
    };

    // Deferred newline with no pending text.
    (@collect [$($parts:expr_2021,)*] [] @ $($rest:tt)*) => {
        frag!(@collect [
            $($parts,)*
            $crate::compiler::fragment::blank(),
        ] [] $($rest)*)
    };

    // Deferred newline after static tokens.
    (@collect [$($parts:expr_2021,)*] [$($text:tt)+] @ $($rest:tt)*) => {
        frag!(@collect [
            $($parts,)*
            stringify!($($text)+),
            $crate::compiler::fragment::blank(),
        ] [] $($rest)*)
    };

    // Accumulate another static token.
    (@collect [$($parts:expr_2021,)*] [$($text:tt)*] $token:tt $($rest:tt)*) => {
        frag!(@collect [$($parts,)*] [$($text)* $token] $($rest)*)
    };

    // Entry point.
    ($($input:tt)*) => {
        frag!(@collect [] [] $($input)*)
    };
}

pub use crate::frag;
