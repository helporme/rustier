//! Composable, allocation-light fragments for generated source text.
//!
//! ```
//! # use rust_curse::codegen::fragment::{frag, FragmentPrint};
//! #
//! let name = "world";
//! let greeting = frag!(hello ~ {name});
//!
//! assert_eq!(greeting.print(), "hello world");
//! ```

use std::fmt;

/// Accumulates fragment output while applying tab-based indentation.
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

    /// Appends preformatted arguments without allocating intermediate text.
    ///
    /// ```
    /// # use rust_curse::codegen::fragment::FragmentPrint;
    /// #
    /// assert_eq!(format_args!("answer: {}", 42).print(), "answer: 42");
    /// ```
    fn write_fmt(&mut self, args: fmt::Arguments<'_>) {
        fmt::write(self, args).expect("writing to a String cannot fail");
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

/// A source code fragment that can be rendered once.
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

/// A source code fragment that can be rendered through a shared reference.
///
/// This object-safe trait is intended for the uncommon cases that need reuse
/// or type erasure. Normal fragment composition should use [`FragmentOnce`].
///
/// ```
/// # use rust_curse::codegen::fragment::{Fragment, FragmentPrint};
/// #
/// let text = String::from("reusable");
/// let fragment: &dyn Fragment = &text;
///
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

impl Fragment for dyn Fn(&mut Printer) + '_ {
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

impl Fragment for str {
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

impl FragmentOnce for fmt::Arguments<'_> {
    fn apply(self, printer: &mut Printer) {
        printer.write_fmt(self);
    }
}

impl Fragment for fmt::Arguments<'_> {
    /// Formats these arguments directly into `printer`.
    ///
    /// ```
    /// # use rust_curse::codegen::fragment::FragmentPrint;
    /// #
    /// assert_eq!(format_args!("{} + {}", 1, 2).print(), "1 + 2");
    /// ```
    fn apply_ref(&self, printer: &mut Printer) {
        printer.write_fmt(*self);
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
pub fn blank() -> impl FragmentOnce {
    fragment(|p| p.new_lines += 1)
}

/// Increases indentation while rendering the given fragment.
///
/// ```
/// # use rust_curse::codegen::fragment::{idented, FragmentPrint};
/// #
/// assert_eq!(idented("one\ntwo").print(), "\tone\n\ttwo");
/// ```
pub fn idented(frag: impl FragmentOnce) -> impl FragmentOnce {
    (move |p: &mut Printer| {
        p.indent += 1;
        frag.apply(p);
        p.indent -= 1;
    })
    .into_fragment()
}

/// Wraps the given fragment in curly braces with indentation.
///
/// ```
/// # use rust_curse::codegen::fragment::{curlies, FragmentPrint};
/// #
/// assert_eq!(curlies("value").print(), "{\n\tvalue\n}");
/// ```
pub fn curlies(frag: impl FragmentOnce) -> impl FragmentOnce {
    ("{\n", idented(frag), "\n}")
}

/// Wraps the given fragment in parentheses with indentation.
///
/// ```
/// # use rust_curse::codegen::fragment::{parens, FragmentPrint};
/// #
/// assert_eq!(parens("value").print(), "(\n\tvalue\n)");
/// ```
pub fn parens(frag: impl FragmentOnce) -> impl FragmentOnce {
    ("(\n", idented(frag), "\n)")
}

/// Wraps the given fragment in single quotes.
///
/// ```
/// # use rust_curse::codegen::fragment::{singles, FragmentPrint};
/// #
/// assert_eq!(singles("value").print(), "'value'");
/// ```
pub fn singles(frag: impl FragmentOnce) -> impl FragmentOnce {
    ("'", frag, "'")
}

/// Wraps the given fragment in double quotes.
///
/// ```
/// # use rust_curse::codegen::fragment::{doubles, FragmentPrint};
/// #
/// assert_eq!(doubles("value").print(), "\"value\"");
/// ```
pub fn doubles(frag: impl FragmentOnce) -> impl FragmentOnce {
    ("\"", frag, "\"")
}

/// Joins fragments with a comma and a newline.
///
/// ```
/// # use rust_curse::codegen::fragment::{comma_list, FragmentPrint};
/// #
/// assert_eq!(
///     comma_list(["one", "two", "three"]).print(),
///     "one,\ntwo,\nthree"
/// );
/// assert_eq!(comma_list(Vec::<&str>::new()).print(), "");
/// ```
pub fn comma_list<I>(frags: I) -> impl FragmentOnce
where
    I: IntoIterator,
    I::Item: FragmentOnce,
{
    (move |p: &mut Printer| {
        for (index, frag) in frags.into_iter().enumerate() {
            if index > 0 {
                p.write(",\n");
            }

            frag.apply(p);
        }
    })
    .into_fragment()
}

/// Joins fragments with one blank line between them.
///
/// If the previous fragment does not end with a newline, `spaced` inserts two;
/// if it ends with one, `spaced` inserts one more. Existing blank lines are
/// preserved.
///
/// ```
/// # use rust_curse::codegen::fragment::{spaced, FragmentPrint};
/// #
/// assert_eq!(spaced(["one", "two"]).print(), "one\n\ntwo");
/// assert_eq!(spaced(["one\n", "two"]).print(), "one\n\ntwo");
/// assert_eq!(spaced(["one\n\n", "two"]).print(), "one\n\ntwo");
/// assert_eq!(spaced(Vec::<&str>::new()).print(), "");
/// ```
pub fn spaced<I>(frags: I) -> impl FragmentOnce
where
    I: IntoIterator,
    I::Item: FragmentOnce,
{
    (move |p: &mut Printer| {
        for (index, frag) in frags.into_iter().enumerate() {
            if index > 0 && !p.out.ends_with("\n\n") {
                if !p.at_line_start {
                    p.write("\n");
                }
                p.write("\n");
            }

            frag.apply(p);
        }
    })
    .into_fragment()
}

/// Builds a fragment from source-like tokens and interpolated fragments.
///
/// Static tokens are emitted as written, `~` inserts a space between tokens and
/// interpolations, `@` inserts a deferred newline separator, and `{expression}`
/// interpolates another
/// [`FragmentOnce`](crate::codegen::fragment::FragmentOnce).
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
/// # Auto formating
///
/// Curly braces and parentheses are automatically wrapped with
/// [`curlies`](crate::codegen::fragment::curlies) and
/// [`parens`](crate::codegen::fragment::parens), respectively.
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
            $crate::codegen::fragment::curlies(frag!($($inner)*)),
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
            $crate::codegen::fragment::parens(frag!($($inner)*)),
        ] [] $($rest)*)
    };

    // Parenthesized fragment after some static tokens.
    (@collect [$($parts:expr_2021,)*] [$($text:tt)+] ($($inner:tt)+) $($rest:tt)*) => {
        frag!(@collect [
            $($parts,)*
            stringify!($($text)+),
            $crate::codegen::fragment::parens(frag!($($inner)*)),
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
            $crate::codegen::fragment::blank(),
        ] [] $($rest)*)
    };

    // Deferred newline after static tokens.
    (@collect [$($parts:expr_2021,)*] [$($text:tt)+] @ $($rest:tt)*) => {
        frag!(@collect [
            $($parts,)*
            stringify!($($text)+),
            $crate::codegen::fragment::blank(),
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
