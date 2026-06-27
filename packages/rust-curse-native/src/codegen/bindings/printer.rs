use std::fmt::{self, Write as _};

/// Builds generated source text with tab indentation helpers.
///
/// ```
/// use rust_curse::codegen::bindings::Printer;
///
/// let mut printer = Printer::default();
/// printer.line("let answer = 42;");
///
/// assert_eq!(String::from(printer), "let answer = 42;\n");
/// ```
#[derive(Debug, Clone, Default)]
pub struct Printer {
    out: String,
    indent: usize,
    at_line_start: bool,
}

impl Printer {
    /// Appends text to the output without adding a trailing newline.
    ///
    /// ```
    /// use rust_curse::codegen::bindings::Printer;
    ///
    /// let mut printer = Printer::default();
    /// printer.write("const ");
    /// printer.write("name = \"Alice\";");
    ///
    /// assert_eq!(String::from(printer), "const name = \"Alice\";");
    /// ```
    pub fn write(&mut self, text: &str) {
        if self.at_line_start {
            self.indent();
        }

        self.out.push_str(text);
        self.at_line_start = false;
    }

    /// Appends formatted text to the output without adding a trailing newline.
    ///
    /// ```
    /// use rust_curse::codegen::bindings::Printer;
    ///
    /// let mut printer = Printer::default();
    /// printer.write_fmt(format_args!("answer = {}", 42));
    ///
    /// assert_eq!(String::from(printer), "answer = 42");
    /// ```
    pub fn write_fmt(&mut self, args: fmt::Arguments<'_>) {
        if self.at_line_start {
            self.indent();
        }
        self.out.write_fmt(args).unwrap();
        self.at_line_start = false;
    }

    /// Appends an indented blank line followed by a newline.
    ///
    /// ```
    /// use rust_curse::codegen::bindings::Printer;
    ///
    /// let mut printer = Printer::default();
    /// printer.blank();
    ///
    /// assert_eq!(String::from(printer), "\n");
    /// ```
    pub fn blank(&mut self) {
        self.indent();
        self.newline();
    }

    /// Appends one line of text followed by a newline.
    ///
    /// ```
    /// use rust_curse::codegen::bindings::Printer;
    ///
    /// let mut printer = Printer::default();
    /// printer.line("export const value = 1;");
    ///
    /// assert_eq!(String::from(printer), "export const value = 1;\n");
    /// ```
    pub fn line(&mut self, text: &str) {
        debug_assert!(text.lines().count() == 1);

        self.indent();
        self.out.push_str(text);
        self.newline();
    }

    /// Appends one formatted line followed by a newline.
    ///
    /// ```
    /// use rust_curse::codegen::bindings::Printer;
    ///
    /// let mut printer = Printer::default();
    /// printer.line_fmt(format_args!("export const value = {};", 1));
    ///
    /// assert_eq!(String::from(printer), "export const value = 1;\n");
    /// ```
    pub fn line_fmt(&mut self, args: fmt::Arguments<'_>) {
        self.write_fmt(args);
        self.newline();
    }

    /// Appends every line in a text block as its own indented line.
    ///
    /// ```
    /// use rust_curse::codegen::bindings::Printer;
    ///
    /// let mut printer = Printer::default();
    /// printer.lines("let first = 1;\nlet second = 2;");
    ///
    /// assert_eq!(String::from(printer), "let first = 1;\nlet second = 2;\n");
    /// ```
    pub fn lines(&mut self, text: &str) {
        for line in text.lines() {
            self.line(line);
        }
    }

    /// Appends every line in formatted text as its own indented line.
    ///
    /// ```
    /// use rust_curse::codegen::bindings::Printer;
    ///
    /// let mut printer = Printer::default();
    /// printer.lines_fmt(format_args!("let first = {};\nlet second = {};", 1, 2));
    ///
    /// assert_eq!(String::from(printer), "let first = 1;\nlet second = 2;\n");
    /// ```
    pub fn lines_fmt(&mut self, args: fmt::Arguments<'_>) {
        for line in args.to_string().lines() {
            self.line(line);
        }
    }

    /// Appends an indented block wrapped in curly braces.
    ///
    /// ```
    /// use rust_curse::codegen::bindings::Printer;
    ///
    /// let mut printer = Printer::default();
    /// printer.curlies(|printer| {
    ///     printer.line("answer: 42");
    /// });
    ///
    /// assert_eq!(String::from(printer), "{\n\tanswer: 42\n}\n");
    /// ```
    pub fn curlies(&mut self, f: impl FnOnce(&mut Self)) {
        self.line("{");
        self.indent += 1;
        f(self);
        self.indent -= 1;
        self.line("}");
    }

    /// Appends an indented block wrapped in parentheses.
    ///
    /// ```
    /// use rust_curse::codegen::bindings::Printer;
    ///
    /// let mut printer = Printer::default();
    /// printer.parens(|printer| {
    ///     printer.line("answer");
    /// });
    ///
    /// assert_eq!(String::from(printer), "(\n\tanswer\n)");
    /// ```
    pub fn parens(&mut self, f: impl FnOnce(&mut Self)) {
        self.line("(");
        self.indent += 1;
        let length = self.out.len();
        f(self);
        self.indent -= 1;

        if self.out.len() == length {
            self.out.pop();
        }
        self.write(")");
    }

    /// Appends a comma-separated list with one item per indented line.
    ///
    /// ```
    /// use rust_curse::codegen::bindings::Printer;
    ///
    /// let mut printer = Printer::default();
    /// printer.comma_list(["first", "second"]);
    ///
    /// assert_eq!(String::from(printer), "first,\nsecond,\n");
    /// ```
    pub fn comma_list<I, S>(&mut self, args: I)
    where
        S: AsRef<str>,
        I: IntoIterator<Item = S>,
    {
        let mut wrote = false;
        for arg in args.into_iter() {
            debug_assert!(arg.as_ref().lines().count() == 1);
            self.indent();
            self.out.push_str(arg.as_ref());
            self.out.push(',');
            self.newline();
            wrote = true;
        }
        if wrote {
            self.out.pop();
            self.newline();
        }
    }

    fn indent(&mut self) {
        for _ in 0..self.indent {
            self.out.push('\t');
        }
    }

    fn newline(&mut self) {
        self.out.push('\n');
        self.at_line_start = true;
    }
}

impl From<Printer> for String {
    fn from(printer: Printer) -> Self {
        printer.out
    }
}
