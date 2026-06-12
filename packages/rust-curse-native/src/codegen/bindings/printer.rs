use std::fmt::{self, Write as _};

#[derive(Debug, Clone, Default)]
pub struct Printer {
    out: String,
    indent: usize,
    at_line_start: bool,
}

impl Printer {
    /// Adds a text to the output.
    pub fn write(&mut self, text: &str) {
        if self.at_line_start {
            self.indent();
        }

        self.out.push_str(text);
        self.at_line_start = false;
    }

    /// Adds formatted text to the output.
    pub fn write_fmt(&mut self, args: fmt::Arguments<'_>) {
        if self.at_line_start {
            self.indent();
        }
        self.out.write_fmt(args).unwrap();
        self.at_line_start = false;
    }

    /// Adds a blank line to the output, followed by a newline character.
    pub fn blank(&mut self) {
        self.indent();
        self.newline();
    }

    /// Adds a line of text to the output, followed by a newline character.
    pub fn line(&mut self, text: &str) {
        debug_assert!(text.lines().count() == 1);

        self.indent();
        self.out.push_str(text);
        self.newline();
    }

    /// Adds a line of formatted text to the output, followed by a newline
    /// character.
    pub fn line_fmt(&mut self, args: fmt::Arguments<'_>) {
        self.write_fmt(args);
        self.newline();
    }

    /// Adds a block of text to the output, reindenting each line.
    pub fn lines(&mut self, text: &str) {
        for line in text.lines() {
            self.line(line);
        }
    }

    /// Adds a block of formatted text to the output, reindenting each line.
    pub fn lines_fmt(&mut self, args: fmt::Arguments<'_>) {
        for line in args.to_string().lines() {
            self.line(line);
        }
    }

    /// Adds a block of text wrapped in curly braces to the output, with
    /// indentation.
    pub fn curlies(&mut self, f: impl FnOnce(&mut Self)) {
        self.line("{");
        self.indent += 1;
        f(self);
        self.indent -= 1;
        self.line("}");
    }

    /// Adds a block of text wrapped in parentheses to the output, with
    /// indentation.
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

    /// Adds a comma-separated list of items to the output, with indentation.
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
