use crate::theme::ThemePalette;
use eframe::egui::text::LayoutJob;
use eframe::egui::{FontId, TextFormat};
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Html,
    Css,
    Json,
    Markdown,
    Toml,
    Yaml,
    Shell,
    Sql,
    C,
    Cpp,
    Php,
    Java,
    Go,
    Plain,
}

impl Language {
    pub fn from_path(path: &Path) -> Self {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if name == "dockerfile" {
            return Self::Shell;
        }
        if name == "makefile" {
            return Self::Shell;
        }
        match path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
        {
            "rs" => Self::Rust,
            "py" | "pyw" => Self::Python,
            "js" | "mjs" | "cjs" | "jsx" => Self::JavaScript,
            "ts" | "tsx" | "mts" | "cts" => Self::TypeScript,
            "html" | "htm" | "xhtml" | "vue" | "svelte" => Self::Html,
            "css" | "scss" | "sass" | "less" => Self::Css,
            "json" | "jsonc" => Self::Json,
            "md" | "markdown" | "mdx" => Self::Markdown,
            "toml" => Self::Toml,
            "yaml" | "yml" => Self::Yaml,
            "sh" | "bash" | "zsh" | "fish" | "env" => Self::Shell,
            "sql" => Self::Sql,
            "c" | "h" => Self::C,
            "cpp" | "cc" | "cxx" | "hpp" | "hh" | "hxx" => Self::Cpp,
            "php" => Self::Php,
            "java" => Self::Java,
            "go" => Self::Go,
            "txt" | "log" => Self::Plain,
            _ => Self::Plain,
        }
    }

    pub fn display(self) -> &'static str {
        match self {
            Self::Rust => "Rust",
            Self::Python => "Python",
            Self::JavaScript => "JavaScript",
            Self::TypeScript => "TypeScript",
            Self::Html => "HTML",
            Self::Css => "CSS",
            Self::Json => "JSON",
            Self::Markdown => "Markdown",
            Self::Toml => "TOML",
            Self::Yaml => "YAML",
            Self::Shell => "Shell",
            Self::Sql => "SQL",
            Self::C => "C",
            Self::Cpp => "C++",
            Self::Php => "PHP",
            Self::Java => "Java",
            Self::Go => "Go",
            Self::Plain => "Plain Text",
        }
    }

    pub fn comment_affixes(self) -> (&'static str, &'static str) {
        match self {
            Self::Rust | Self::JavaScript | Self::TypeScript | Self::C | Self::Cpp | Self::Java
            | Self::Go => ("// ", ""),
            Self::Python | Self::Shell | Self::Toml | Self::Yaml | Self::Php => ("# ", ""),
            Self::Html => ("<!-- ", " -->"),
            Self::Css => ("/* ", " */"),
            Self::Sql => ("-- ", ""),
            Self::Json | Self::Markdown | Self::Plain => ("", ""),
        }
    }

    fn uses_c_like(self) -> bool {
        matches!(
            self,
            Self::Rust | Self::JavaScript | Self::TypeScript | Self::C | Self::Cpp | Self::Java | Self::Go
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Token {
    Plain,
    Keyword,
    String,
    Comment,
    Number,
    Function,
    Type,
    Variable,
    Macro,
    Operator,
    Tag,
    Attribute,
    Property,
    Constant,
}

impl Token {
    fn color(self, p: &ThemePalette) -> eframe::egui::Color32 {
        match self {
            Self::Plain => p.fg,
            Self::Keyword => p.syntax_keyword,
            Self::String => p.syntax_string,
            Self::Comment => p.syntax_comment,
            Self::Number => p.syntax_number,
            Self::Function => p.syntax_function,
            Self::Type => p.syntax_type,
            Self::Variable => p.syntax_variable,
            Self::Macro => p.syntax_macro,
            Self::Operator => p.syntax_operator,
            Self::Tag => p.syntax_tag,
            Self::Attribute => p.syntax_attribute,
            Self::Property => p.syntax_property,
            Self::Constant => p.syntax_constant,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum State {
    None,
    CComment,
    HtmlComment,
    Triple(char),
    Fence,
}

struct Builder<'a> {
    job: LayoutJob,
    p: &'a ThemePalette,
    font: FontId,
}

impl<'a> Builder<'a> {
    fn new(p: &'a ThemePalette, font: FontId, wrap_width: f32) -> Self {
        let mut job = LayoutJob::default();
        job.wrap.max_width = wrap_width;
        job.wrap.break_anywhere = false;
        job.wrap.overflow_character = None;
        Self { job, p, font }
    }

    fn push(&mut self, text: &str, token: Token) {
        if text.is_empty() {
            return;
        }
        if let Some(last) = self.job.sections.last() {
            if last.format.color == token.color(self.p) && last.byte_range.end == self.job.text.len()
            {
                let range_start = last.byte_range.start;
                let format = last.format.clone();
                self.job.sections.pop();
                self.job.text.push_str(text);
                self.job.sections.push(eframe::egui::text::LayoutSection {
                    leading_space: 0.0,
                    byte_range: range_start..self.job.text.len(),
                    format,
                });
                return;
            }
        }
        let start = self.job.text.len();
        self.job.text.push_str(text);
        self.job.sections.push(eframe::egui::text::LayoutSection {
            leading_space: 0.0,
            byte_range: start..start + text.len(),
            format: TextFormat {
                font_id: self.font.clone(),
                color: token.color(self.p),
                ..Default::default()
            },
        });
    }

    fn newline(&mut self) {
        self.push("\n", Token::Plain);
    }
}

pub fn highlight(
    text: &str,
    language: Language,
    font: FontId,
    wrap_width: f32,
    palette: &ThemePalette,
) -> LayoutJob {
    let mut b = Builder::new(palette, font, wrap_width);
    let mut state = State::None;
    for line in text.split('\n') {
        if !b.job.text.is_empty() {
            b.newline();
        }
        highlight_line(line, language, &mut state, &mut b);
    }
    if b.job.sections.is_empty() {
        b.push(" ", Token::Plain);
    }
    b.job
}

fn highlight_line(line: &str, language: Language, state: &mut State, b: &mut Builder<'_>) {
    match language {
        Language::Html => highlight_html(line, state, b),
        Language::Css => highlight_css(line, state, b),
        Language::Json => highlight_json(line, state, b),
        Language::Markdown => highlight_markdown(line, state, b),
        Language::Toml | Language::Yaml => {
            highlight_key_value(line, language, state, b)
        }
        Language::Sql => highlight_sql(line, state, b),
        Language::Python => highlight_python(line, state, b),
        Language::Php => highlight_php(line, state, b),
        language if language.uses_c_like() => {
            highlight_c_like(line, language, state, b)
        }
        Language::Shell => highlight_shell(line, state, b),
        Language::Plain => b.push(line, Token::Plain),
        _ => b.push(line, Token::Plain),
    }
}

fn scan_prefix<'a>(text: &'a str, start: usize, needle: &str) -> bool {
    text[start..].starts_with(needle)
}

fn utf8_char_len(ch: char) -> usize {
    ch.len_utf8()
}

fn is_ident_start(ch: char) -> bool {
    ch == '_' || ch == '$' || ch.is_alphabetic()
}

fn is_ident_continue(ch: char) -> bool {
    ch == '_' || ch == '$' || ch.is_alphanumeric()
}

fn take_quoted(line: &str, start: usize, quote: char) -> (usize, Option<State>) {
    let bytes = line.as_bytes();
    let mut i = start + quote.len_utf8();
    while i < bytes.len() {
        let ch = line[i..].chars().next().unwrap_or('\0');
        if ch == '\\' {
            i += 1;
            if i < bytes.len() {
                let escaped = line[i..].chars().next().unwrap_or('\0');
                i += utf8_char_len(escaped);
            }
            continue;
        }
        if ch == quote {
            return (i + utf8_char_len(ch), None);
        }
        i += utf8_char_len(ch);
    }
    (line.len(), Some(State::Triple(quote)))
}

fn highlight_c_like(line: &str, language: Language, state: &mut State, b: &mut Builder<'_>) {
    let mut i = 0usize;
    let mut plain_start = 0usize;

    if matches!(state, State::CComment) {
        if let Some(end) = line.find("*/") {
            b.push(&line[..end + 2], Token::Comment);
            i = end + 2;
            *state = State::None;
        } else {
            b.push(line, Token::Comment);
            return;
        }
    }

    while i < line.len() {
        let rest = &line[i..];
        let ch = rest.chars().next().unwrap_or('\0');

        if scan_prefix(line, i, "//") {
            flush_plain(line, plain_start, i, b);
            b.push(&line[i..], Token::Comment);
            return;
        }
        if scan_prefix(line, i, "/*") {
            flush_plain(line, plain_start, i, b);
            if let Some(end) = line[i + 2..].find("*/") {
                let end = i + 2 + end + 2;
                b.push(&line[i..end], Token::Comment);
                i = end;
                plain_start = i;
                continue;
            } else {
                b.push(&line[i..], Token::Comment);
                *state = State::CComment;
                return;
            }
        }
        if language == Language::Rust && scan_prefix(line, i, "#[") && plain_start < i {
            flush_plain(line, plain_start, i, b);
            plain_start = i;
        }
        if ch == '"' || ch == '\'' || (ch == '`' && !language.uses_c_like()) {
            flush_plain(line, plain_start, i, b);
            let (end, next_state) = take_quoted(line, i, ch);
            b.push(&line[i..end], Token::String);
            if let Some(next) = next_state {
                *state = next;
            }
            i = end;
            plain_start = i;
            continue;
        }
        if ch.is_ascii_digit() {
            flush_plain(line, plain_start, i, b);
            let mut end = i;
            while end < line.len() {
                let c = line[end..].chars().next().unwrap_or('\0');
                if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | 'x' | 'X') {
                    end += utf8_char_len(c);
                } else {
                    break;
                }
            }
            b.push(&line[i..end], Token::Number);
            i = end;
            plain_start = i;
            continue;
        }
        if is_ident_start(ch) {
            flush_plain(line, plain_start, i, b);
            let mut end = i;
            while end < line.len() {
                let c = line[end..].chars().next().unwrap_or('\0');
                if is_ident_continue(c) {
                    end += utf8_char_len(c);
                } else {
                    break;
                }
            }
            let word = &line[i..end];
            let after = line[end..].trim_start();
            let is_macro = after.starts_with('!')
                || (word.starts_with("r#") && after.starts_with('"'))
                || word.ends_with('!');
            let next_nonspace = line[end..].chars().find(|c| !c.is_whitespace());
            let class = if is_macro {
                Token::Macro
            } else if keyword(language, word) {
                Token::Keyword
            } else if constant(language, word) {
                Token::Constant
            } else if word.chars().all(|c| c.is_uppercase() || c == '_' || c.is_numeric())
                && word.chars().any(|c| c.is_alphabetic())
            {
                Token::Constant
            } else if next_nonspace == Some('(') && !matches!(word, "if" | "for" | "while" | "match" | "switch") {
                Token::Function
            } else if word.chars().next().is_some_and(|c| c.is_uppercase()) {
                Token::Type
            } else if language == Language::Php && word.starts_with('$') {
                Token::Variable
            } else {
                // Rust lifetimes are attributes-ish.
                if language == Language::Rust && word.starts_with('\'') {
                    Token::Attribute
                } else {
                    Token::Plain
                }
            };
            b.push(word, class);
            if is_macro && class != Token::Macro {
                let _bang_start = end + (line[end..].len() - after.len());
                b.push("!", Token::Macro);
            }
            i = end;
            plain_start = i;
            continue;
        }
        if ch.is_whitespace() {
            i += utf8_char_len(ch);
            continue;
        }
        flush_plain(line, plain_start, i, b);
        let end = i + utf8_char_len(ch);
        let class = if "{}[]()".contains(ch) {
            Token::Operator
        } else {
            Token::Operator
        };
        b.push(&line[i..end], class);
        i = end;
        plain_start = i;
    }
    flush_plain(line, plain_start, line.len(), b);
}

fn flush_plain(line: &str, start: usize, end: usize, b: &mut Builder<'_>) {
    if start < end {
        b.push(&line[start..end], Token::Plain);
    }
}

fn keyword(language: Language, word: &str) -> bool {
    match language {
        Language::Rust => matches!(
            word,
            "as" | "async" | "await" | "break" | "const" | "continue" | "crate" | "dyn"
                | "else" | "enum" | "extern" | "false" | "fn" | "for" | "if" | "impl" | "in"
                | "let" | "loop" | "match" | "mod" | "move" | "mut" | "pub" | "ref" | "return"
                | "self" | "Self" | "static" | "struct" | "super" | "trait" | "true" | "type"
                | "unsafe" | "use" | "where" | "while" | "yield"
        ),
        Language::Python => matches!(
            word,
            "and" | "as" | "assert" | "async" | "await" | "break" | "case" | "class"
                | "continue" | "def" | "del" | "elif" | "else" | "except" | "False" | "finally"
                | "for" | "from" | "global" | "if" | "import" | "in" | "is" | "lambda" | "None"
                | "nonlocal" | "not" | "or" | "pass" | "raise" | "return" | "True" | "try"
                | "while" | "with" | "yield" | "match"
        ),
        Language::JavaScript | Language::TypeScript => matches!(
            word,
            "async" | "await" | "break" | "case" | "catch" | "class" | "const" | "continue"
                | "debugger" | "default" | "delete" | "do" | "else" | "enum" | "export"
                | "extends" | "false" | "finally" | "for" | "function" | "if" | "implements"
                | "import" | "in" | "instanceof" | "interface" | "let" | "new" | "null" | "of"
                | "return" | "static" | "super" | "switch" | "this" | "throw" | "true" | "try"
                | "type" | "typeof" | "undefined" | "var" | "void" | "while" | "yield"
        ),
        Language::C | Language::Cpp => matches!(
            word,
            "auto" | "bool" | "break" | "case" | "catch" | "char" | "class" | "const"
                | "constexpr" | "continue" | "default" | "delete" | "do" | "double" | "else"
                | "enum" | "explicit" | "extern" | "false" | "float" | "for" | "friend" | "if"
                | "inline" | "int" | "long" | "namespace" | "new" | "nullptr" | "operator"
                | "private" | "protected" | "public" | "register" | "return" | "short"
                | "signed" | "sizeof" | "static" | "struct" | "switch" | "template" | "this"
                | "throw" | "true" | "try" | "typedef" | "typename" | "union" | "unsigned"
                | "using" | "virtual" | "void" | "volatile" | "while"
        ),
        Language::Java => matches!(
            word,
            "abstract" | "assert" | "boolean" | "break" | "byte" | "case" | "catch" | "char"
                | "class" | "const" | "continue" | "default" | "do" | "double" | "else"
                | "enum" | "extends" | "final" | "finally" | "float" | "for" | "if"
                | "implements" | "import" | "instanceof" | "int" | "interface" | "long"
                | "native" | "new" | "null" | "package" | "private" | "protected" | "public"
                | "return" | "short" | "static" | "strictfp" | "super" | "switch"
                | "synchronized" | "this" | "throw" | "throws" | "transient" | "true" | "try"
                | "void" | "volatile" | "while"
        ),
        Language::Go => matches!(
            word,
            "break" | "case" | "chan" | "const" | "continue" | "default" | "defer" | "else"
                | "fallthrough" | "for" | "func" | "go" | "goto" | "if" | "import"
                | "interface" | "map" | "package" | "range" | "return" | "select" | "struct"
                | "switch" | "type" | "var"
        ),
        Language::Php => matches!(
            word,
            "abstract" | "and" | "array" | "as" | "break" | "callable" | "case" | "catch"
                | "class" | "clone" | "const" | "continue" | "declare" | "default" | "die"
                | "do" | "echo" | "else" | "elseif" | "empty" | "enddeclare" | "endfor"
                | "endforeach" | "endif" | "endswitch" | "endwhile" | "eval" | "exit"
                | "extends" | "final" | "finally" | "fn" | "for" | "foreach" | "function"
                | "global" | "goto" | "if" | "implements" | "include" | "include_once"
                | "instanceof" | "insteadof" | "interface" | "isset" | "list" | "match"
                | "namespace" | "new" | "or" | "print" | "private" | "protected" | "public"
                | "readonly" | "require" | "require_once" | "return" | "static" | "switch"
                | "throw" | "trait" | "try" | "unset" | "use" | "var" | "while" | "xor"
                | "yield"
        ),
        _ => false,
    }
}

fn constant(language: Language, word: &str) -> bool {
    match language {
        Language::Rust => matches!(word, "None" | "Some" | "Ok" | "Err" | "true" | "false"),
        Language::Python => matches!(word, "True" | "False" | "None" | "self" | "cls"),
        Language::JavaScript | Language::TypeScript => {
            matches!(word, "true" | "false" | "null" | "undefined" | "NaN" | "Infinity")
        }
        Language::Php => matches!(word, "true" | "false" | "null" | "TRUE" | "FALSE" | "NULL" | "this"),
        Language::C | Language::Cpp => matches!(word, "true" | "false" | "NULL" | "nullptr"),
        Language::Java => matches!(word, "true" | "false" | "null"),
        Language::Go => matches!(word, "true" | "false" | "nil" | "iota"),
        _ => false,
    }
}

fn highlight_python(line: &str, state: &mut State, b: &mut Builder<'_>) {
    if let State::Triple(quote) = *state {
        let terminator = format!("{quote}{quote}{quote}");
        if let Some(end) = line.find(&terminator) {
            b.push(&line[..end + 3], Token::String);
            *state = State::None;
            highlight_c_like(&line[end + 3..], Language::Rust, state, b);
        } else {
            b.push(line, Token::String);
        }
        return;
    }

    let mut i = 0;
    let mut plain_start = 0;
    while i < line.len() {
        let ch = line[i..].chars().next().unwrap_or('\0');
        if ch == '#' {
            flush_plain(line, plain_start, i, b);
            b.push(&line[i..], Token::Comment);
            return;
        }
        if (line[i..].starts_with("\"\"\"") || line[i..].starts_with("'''"))
            && i + 3 <= line.len()
        {
            flush_plain(line, plain_start, i, b);
            let quote = ch;
            let terminator = if quote == '"' { "\"\"\"" } else { "'''" };
            if let Some(end) = line[i + 3..].find(terminator) {
                let end = i + 3 + end + 3;
                b.push(&line[i..end], Token::String);
                i = end;
                plain_start = i;
                continue;
            }
            b.push(&line[i..], Token::String);
            *state = State::Triple(quote);
            return;
        }
        if ch == '"' || ch == '\'' {
            flush_plain(line, plain_start, i, b);
            let (end, _) = take_quoted(line, i, ch);
            b.push(&line[i..end], Token::String);
            i = end;
            plain_start = i;
            continue;
        }
        if ch == '@' {
            flush_plain(line, plain_start, i, b);
            let mut end = i + 1;
            while end < line.len() {
                let c = line[end..].chars().next().unwrap_or('\0');
                if is_ident_continue(c) || c == '.' {
                    end += utf8_char_len(c);
                } else {
                    break;
                }
            }
            b.push(&line[i..end], Token::Attribute);
            i = end;
            plain_start = i;
            continue;
        }
        if ch.is_ascii_digit() {
            flush_plain(line, plain_start, i, b);
            let mut end = i;
            while end < line.len() {
                let c = line[end..].chars().next().unwrap_or('\0');
                if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | 'x' | 'o' | 'b') {
                    end += utf8_char_len(c);
                } else {
                    break;
                }
            }
            b.push(&line[i..end], Token::Number);
            i = end;
            plain_start = i;
            continue;
        }
        if is_ident_start(ch) {
            flush_plain(line, plain_start, i, b);
            let mut end = i;
            while end < line.len() {
                let c = line[end..].chars().next().unwrap_or('\0');
                if is_ident_continue(c) {
                    end += utf8_char_len(c);
                } else {
                    break;
                }
            }
            let word = &line[i..end];
            let next = line[end..].chars().find(|c| !c.is_whitespace());
            let class = if keyword(Language::Python, word) {
                Token::Keyword
            } else if constant(Language::Python, word) {
                Token::Constant
            } else if next == Some('(') {
                Token::Function
            } else if word.chars().next().is_some_and(|c| c.is_uppercase()) {
                Token::Type
            } else {
                Token::Plain
            };
            b.push(word, class);
            i = end;
            plain_start = i;
            continue;
        }
        if ch.is_whitespace() {
            i += utf8_char_len(ch);
            continue;
        }
        flush_plain(line, plain_start, i, b);
        let end = i + utf8_char_len(ch);
        b.push(&line[i..end], Token::Operator);
        i = end;
        plain_start = i;
    }
    flush_plain(line, plain_start, line.len(), b);
}

fn highlight_php(line: &str, state: &mut State, b: &mut Builder<'_>) {
    if line.trim_start().starts_with('<') && !line.contains("<?php") {
        highlight_html(line, state, b);
        return;
    }
    highlight_c_like(line, Language::Php, state, b);
}

fn highlight_html(line: &str, state: &mut State, b: &mut Builder<'_>) {
    if matches!(state, State::HtmlComment) {
        if let Some(end) = line.find("-->") {
            b.push(&line[..end + 3], Token::Comment);
            *state = State::None;
            highlight_html(&line[end + 3..], state, b);
        } else {
            b.push(line, Token::Comment);
        }
        return;
    }
    if line.contains("<!--") {
        let mut cursor = 0;
        while let Some(open) = line[cursor..].find("<!--") {
            let open = cursor + open;
            flush_plain(line, cursor, open, b);
            if let Some(close) = line[open + 4..].find("-->") {
                let close = open + 4 + close + 3;
                b.push(&line[open..close], Token::Comment);
                cursor = close;
            } else {
                b.push(&line[open..], Token::Comment);
                *state = State::HtmlComment;
                return;
            }
        }
        flush_plain(line, cursor, line.len(), b);
        return;
    }
    let mut i = 0;
    let mut plain_start = 0;
    while i < line.len() {
        let ch = line[i..].chars().next().unwrap_or('\0');
        if ch == '<' {
            flush_plain(line, plain_start, i, b);
            let mut end = i + 1;
            if line[end..].starts_with('/') {
                end += 1;
            }
            b.push("<", Token::Operator);
            if end > i + 1 {
                b.push("/", Token::Operator);
            }
            let name_start = end;
            while end < line.len() {
                let c = line[end..].chars().next().unwrap_or('\0');
                if c.is_alphanumeric() || matches!(c, ':' | '-' | '_') {
                    end += utf8_char_len(c);
                } else {
                    break;
                }
            }
            b.push(&line[name_start..end], Token::Tag);
            i = end;
            while i < line.len() {
                let c = line[i..].chars().next().unwrap_or('\0');
                if c == '>' {
                    b.push(">", Token::Operator);
                    i += 1;
                    break;
                }
                if c == '/' && line[i..].starts_with("/>") {
                    b.push("/>", Token::Operator);
                    i += 2;
                    break;
                }
                if c.is_whitespace() {
                    let start = i;
                    while i < line.len() {
                        let c = line[i..].chars().next().unwrap_or('\0');
                        if c.is_whitespace() {
                            i += utf8_char_len(c);
                        } else {
                            break;
                        }
                    }
                    b.push(&line[start..i], Token::Plain);
                    continue;
                }
                if c == '"' || c == '\'' {
                    let (next, _) = take_quoted(line, i, c);
                    b.push(&line[i..next], Token::String);
                    i = next;
                    continue;
                }
                if c == '=' {
                    b.push("=", Token::Operator);
                    i += 1;
                    continue;
                }
                let start = i;
                while i < line.len() {
                    let c = line[i..].chars().next().unwrap_or('\0');
                    if c.is_whitespace() || matches!(c, '>' | '=' | '"' | '\'' | '/') {
                        break;
                    }
                    i += utf8_char_len(c);
                }
                if start == i {
                    let len = utf8_char_len(c);
                    b.push(&line[i..i + len], Token::Operator);
                    i += len;
                } else {
                    b.push(&line[start..i], Token::Attribute);
                }
            }
            plain_start = i;
            continue;
        }
        i += utf8_char_len(ch);
    }
    flush_plain(line, plain_start, line.len(), b);
}

fn highlight_css(line: &str, state: &mut State, b: &mut Builder<'_>) {
    if matches!(state, State::CComment) {
        if let Some(end) = line.find("*/") {
            b.push(&line[..end + 2], Token::Comment);
            *state = State::None;
            highlight_css(&line[end + 2..], state, b);
        } else {
            b.push(line, Token::Comment);
        }
        return;
    }
    let mut i = 0;
    let mut plain_start = 0;
    while i < line.len() {
        let ch = line[i..].chars().next().unwrap_or('\0');
        if line[i..].starts_with("/*") {
            flush_plain(line, plain_start, i, b);
            if let Some(end) = line[i + 2..].find("*/") {
                let end = i + 2 + end + 2;
                b.push(&line[i..end], Token::Comment);
                i = end;
            } else {
                b.push(&line[i..], Token::Comment);
                *state = State::CComment;
                return;
            }
            plain_start = i;
            continue;
        }
        if ch == '"' || ch == '\'' {
            flush_plain(line, plain_start, i, b);
            let (end, _) = take_quoted(line, i, ch);
            b.push(&line[i..end], Token::String);
            i = end;
            plain_start = i;
            continue;
        }
        if ch == '@' {
            flush_plain(line, plain_start, i, b);
            let mut end = i + 1;
            while end < line.len() {
                let c = line[end..].chars().next().unwrap_or('\0');
                if c.is_alphanumeric() || c == '-' {
                    end += utf8_char_len(c);
                } else {
                    break;
                }
            }
            b.push(&line[i..end], Token::Keyword);
            i = end;
            plain_start = i;
            continue;
        }
        if ch.is_ascii_digit() || (ch == '.' && line[i + 1..].starts_with(|c: char| c.is_ascii_digit())) {
            flush_plain(line, plain_start, i, b);
            let mut end = i;
            while end < line.len() {
                let c = line[end..].chars().next().unwrap_or('\0');
                if c.is_ascii_alphanumeric() || matches!(c, '.' | '%' | '#') {
                    end += utf8_char_len(c);
                } else {
                    break;
                }
            }
            b.push(&line[i..end], Token::Number);
            i = end;
            plain_start = i;
            continue;
        }
        if is_ident_start(ch) || ch == '-' {
            let start = i;
            let mut end = i;
            while end < line.len() {
                let c = line[end..].chars().next().unwrap_or('\0');
                if c.is_alphanumeric() || matches!(c, '-' | '_') {
                    end += utf8_char_len(c);
                } else {
                    break;
                }
            }
            let word = &line[start..end];
            let after = line[end..].trim_start();
            if after.starts_with(':') {
                flush_plain(line, plain_start, start, b);
                b.push(word, Token::Property);
                i = end;
                plain_start = i;
                continue;
            }
            if line[..start].trim_end().ends_with('{')
                || line[..start].trim_end().ends_with(',')
                || start == 0
                || line[..start].trim().is_empty()
            {
                flush_plain(line, plain_start, start, b);
                b.push(word, Token::Tag);
                i = end;
                plain_start = i;
                continue;
            }
            i = end;
            continue;
        }
        if ch.is_whitespace() {
            i += utf8_char_len(ch);
            continue;
        }
        // 3/6 digit hex colors
        if ch == '#' {
            flush_plain(line, plain_start, i, b);
            let mut end = i + 1;
            while end < line.len()
                && line[end..]
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_hexdigit())
            {
                end += 1;
            }
            b.push(&line[i..end], Token::Number);
            i = end;
            plain_start = i;
            continue;
        }
        flush_plain(line, plain_start, i, b);
        let len = utf8_char_len(ch);
        b.push(&line[i..i + len], Token::Operator);
        i += len;
        plain_start = i;
    }
    flush_plain(line, plain_start, line.len(), b);
}

fn highlight_json(line: &str, _state: &mut State, b: &mut Builder<'_>) {
    let mut i = 0;
    let mut plain_start = 0;
    while i < line.len() {
        let ch = line[i..].chars().next().unwrap_or('\0');
        if ch == '"' {
            flush_plain(line, plain_start, i, b);
            let (end, _) = take_quoted(line, i, '"');
            let after = line[end..].trim_start();
            let class = if after.starts_with(':') {
                Token::Property
            } else {
                Token::String
            };
            b.push(&line[i..end], class);
            i = end;
            plain_start = i;
            continue;
        }
        if ch.is_ascii_digit() || ch == '-' {
            flush_plain(line, plain_start, i, b);
            let mut end = i;
            while end < line.len() {
                let c = line[end..].chars().next().unwrap_or('\0');
                if c.is_ascii_digit() || matches!(c, '.' | '-' | 'e' | 'E' | '+') {
                    end += 1;
                } else {
                    break;
                }
            }
            b.push(&line[i..end], Token::Number);
            i = end;
            plain_start = i;
            continue;
        }
        if ch.is_alphabetic() {
            flush_plain(line, plain_start, i, b);
            let start = i;
            while i < line.len() {
                let c = line[i..].chars().next().unwrap_or('\0');
                if c.is_alphabetic() {
                    i += utf8_char_len(c);
                } else {
                    break;
                }
            }
            let class = if matches!(&line[start..i], "true" | "false" | "null") {
                Token::Constant
            } else {
                Token::Plain
            };
            b.push(&line[start..i], class);
            plain_start = i;
            continue;
        }
        i += utf8_char_len(ch);
    }
    flush_plain(line, plain_start, line.len(), b);
}

fn highlight_markdown(line: &str, state: &mut State, b: &mut Builder<'_>) {
    let trimmed = line.trim_start();
    if matches!(state, State::Fence) {
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            b.push(line, Token::Keyword);
            *state = State::None;
        } else {
            b.push(line, Token::String);
        }
        return;
    }
    if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
        b.push(line, Token::Keyword);
        *state = State::Fence;
        return;
    }
    if trimmed.starts_with('#') {
        b.push(line, Token::Keyword);
        return;
    }
    let mut i = 0;
    while i < line.len() {
        if line[i..].starts_with("**") {
            if let Some(end) = line[i + 2..].find("**") {
                b.push(&line[..i], Token::Plain);
                b.push(&line[i..i + 2 + end + 2], Token::Type);
                return;
            }
        }
        if line[i..].starts_with('`') {
            if let Some(end) = line[i + 1..].find('`') {
                b.push(&line[..i], Token::Plain);
                b.push(&line[i..i + 1 + end + 1], Token::String);
                return;
            }
        }
        i += utf8_char_len(line[i..].chars().next().unwrap_or('\0'));
    }
    if let Some(stripped) = trimmed.strip_prefix("- ").or_else(|| trimmed.strip_prefix("* ")) {
        let prefix = &line[..line.len() - stripped.len()];
        b.push(prefix, Token::Macro);
        b.push(stripped, Token::Plain);
    } else if let Some(url_start) = line.find("](") {
        if line.starts_with('[') {
            let end = line[url_start + 2..]
                .find(')')
                .map(|i| url_start + 2 + i + 1)
                .unwrap_or(line.len());
            b.push(&line[..=url_start + 1], Token::Attribute);
            b.push(&line[url_start + 2..end], Token::String);
            b.push(&line[end..], Token::Plain);
        } else {
            b.push(line, Token::Plain);
        }
    } else {
        b.push(line, Token::Plain);
    }
}

fn highlight_key_value(line: &str, language: Language, state: &mut State, b: &mut Builder<'_>) {
    if let State::CComment = *state {
        if let Some(end) = line.find("*/") {
            b.push(&line[..end + 2], Token::Comment);
            *state = State::None;
        } else {
            b.push(line, Token::Comment);
            return;
        }
    }
    let mut i = 0;
    let mut plain_start = 0;
    let mut has_seen_equals = false;
    while i < line.len() {
        let ch = line[i..].chars().next().unwrap_or('\0');
        if ch == '#' {
            flush_plain(line, plain_start, i, b);
            b.push(&line[i..], Token::Comment);
            return;
        }
        if ch == '"' || ch == '\'' {
            flush_plain(line, plain_start, i, b);
            let (end, _) = take_quoted(line, i, ch);
            let after = line[end..].trim_start();
            let class = if !has_seen_equals && (after.starts_with('=') || after.starts_with(':')) {
                Token::Property
            } else {
                Token::String
            };
            b.push(&line[i..end], class);
            i = end;
            plain_start = i;
            continue;
        }
        if ch == '=' || ch == ':' {
            flush_plain(line, plain_start, i, b);
            b.push(&line[i..i + 1], Token::Operator);
            has_seen_equals = true;
            i += 1;
            plain_start = i;
            continue;
        }
        if language == Language::Yaml && i == line.len() - line.trim_start().len() && ch == '-' {
            flush_plain(line, plain_start, i, b);
            b.push("-", Token::Macro);
            i += 1;
            plain_start = i;
            continue;
        }
        if (i == line.len() - line.trim_start().len()) && language == Language::Toml && ch == '[' {
            flush_plain(line, plain_start, i, b);
            b.push(&line[i..], Token::Keyword);
            return;
        }
        if ch.is_ascii_digit() {
            flush_plain(line, plain_start, i, b);
            let mut end = i;
            while end < line.len()
                && line[end..]
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_digit() || matches!(c, '.' | '_' | ':'))
            {
                end += 1;
            }
            b.push(&line[i..end], Token::Number);
            i = end;
            plain_start = i;
            continue;
        }
        if !has_seen_equals && is_ident_start(ch) {
            flush_plain(line, plain_start, i, b);
            let start = i;
            while i < line.len() {
                let c = line[i..].chars().next().unwrap_or('\0');
                if is_ident_continue(c) || c == '-' || c == '.' {
                    i += utf8_char_len(c);
                } else {
                    break;
                }
            }
            b.push(&line[start..i], Token::Property);
            plain_start = i;
            continue;
        }
        i += utf8_char_len(ch);
    }
    flush_plain(line, plain_start, line.len(), b);
}

fn highlight_sql(line: &str, state: &mut State, b: &mut Builder<'_>) {
    if let State::CComment = *state {
        if let Some(end) = line.find("*/") {
            b.push(&line[..end + 2], Token::Comment);
            *state = State::None;
        } else {
            b.push(line, Token::Comment);
            return;
        }
    }
    let mut i = 0;
    let mut plain_start = 0;
    while i < line.len() {
        let ch = line[i..].chars().next().unwrap_or('\0');
        if line[i..].starts_with("--") {
            flush_plain(line, plain_start, i, b);
            b.push(&line[i..], Token::Comment);
            return;
        }
        if line[i..].starts_with("/*") {
            flush_plain(line, plain_start, i, b);
            if let Some(end) = line[i + 2..].find("*/") {
                let end = i + 2 + end + 2;
                b.push(&line[i..end], Token::Comment);
                i = end;
            } else {
                b.push(&line[i..], Token::Comment);
                *state = State::CComment;
                return;
            }
            plain_start = i;
            continue;
        }
        if ch == '\'' || ch == '"' || ch == '`' {
            flush_plain(line, plain_start, i, b);
            let (end, _) = take_quoted(line, i, ch);
            b.push(&line[i..end], Token::String);
            i = end;
            plain_start = i;
            continue;
        }
        if ch.is_ascii_digit() {
            flush_plain(line, plain_start, i, b);
            let mut end = i;
            while end < line.len()
                && line[end..]
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_digit() || c == '.')
            {
                end += 1;
            }
            b.push(&line[i..end], Token::Number);
            i = end;
            plain_start = i;
            continue;
        }
        if ch.is_alphabetic() || ch == '_' {
            flush_plain(line, plain_start, i, b);
            let start = i;
            while i < line.len() {
                let c = line[i..].chars().next().unwrap_or('\0');
                if c.is_alphanumeric() || c == '_' {
                    i += utf8_char_len(c);
                } else {
                    break;
                }
            }
            let word = &line[start..i];
            let upper = word.to_ascii_uppercase();
            let class = if matches!(
                upper.as_str(),
                "SELECT" | "FROM" | "WHERE" | "INSERT" | "INTO" | "VALUES" | "UPDATE" | "SET"
                    | "DELETE" | "CREATE" | "TABLE" | "ALTER" | "DROP" | "JOIN" | "LEFT"
                    | "RIGHT" | "INNER" | "OUTER" | "ON" | "GROUP" | "BY" | "ORDER" | "HAVING"
                    | "LIMIT" | "OFFSET" | "AS" | "AND" | "OR" | "NOT" | "NULL" | "IS" | "IN"
                    | "CASE" | "WHEN" | "THEN" | "ELSE" | "END" | "PRIMARY" | "KEY" | "FOREIGN"
                    | "REFERENCES" | "INDEX" | "UNIQUE" | "DEFAULT" | "CASCADE" | "DISTINCT"
                    | "COUNT" | "SUM" | "AVG" | "MIN" | "MAX" | "UNION" | "ALL"
            ) {
                Token::Keyword
            } else if matches!(upper.as_str(), "TRUE" | "FALSE" | "NULL") {
                Token::Constant
            } else {
                Token::Plain
            };
            b.push(word, class);
            plain_start = i;
            continue;
        }
        i += utf8_char_len(ch);
    }
    flush_plain(line, plain_start, line.len(), b);
}

fn highlight_shell(line: &str, state: &mut State, b: &mut Builder<'_>) {
    if matches!(state, State::CComment) {
        b.push(line, Token::Comment);
        return;
    }
    let mut i = 0;
    let mut plain_start = 0;
    while i < line.len() {
        let ch = line[i..].chars().next().unwrap_or('\0');
        if ch == '#' && (i == 0 || line[..i].ends_with(|c: char| c.is_whitespace())) {
            flush_plain(line, plain_start, i, b);
            b.push(&line[i..], Token::Comment);
            return;
        }
        if ch == '"' || ch == '\'' {
            flush_plain(line, plain_start, i, b);
            let (end, _) = take_quoted(line, i, ch);
            b.push(&line[i..end], Token::String);
            i = end;
            plain_start = i;
            continue;
        }
        if ch == '$' {
            flush_plain(line, plain_start, i, b);
            let mut end = i + 1;
            if line[end..].starts_with('{') {
                if let Some(close) = line[end..].find('}') {
                    end = end + close + 1;
                }
            } else {
                while end < line.len()
                    && line[end..]
                        .chars()
                        .next()
                        .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
                {
                    end += 1;
                }
            }
            b.push(&line[i..end], Token::Variable);
            i = end;
            plain_start = i;
            continue;
        }
        if ch.is_alphabetic() || ch == '_' {
            flush_plain(line, plain_start, i, b);
            let start = i;
            while i < line.len() {
                let c = line[i..].chars().next().unwrap_or('\0');
                if c.is_alphanumeric() || c == '_' || c == '-' {
                    i += utf8_char_len(c);
                } else {
                    break;
                }
            }
            let word = &line[start..i];
            let class = if matches!(
                word,
                "if" | "then" | "else" | "elif" | "fi" | "for" | "while" | "do" | "done" | "case"
                    | "esac" | "function" | "in" | "return" | "export" | "local" | "readonly"
                    | "source" | "alias" | "echo" | "cd" | "exit"
            ) {
                Token::Keyword
            } else {
                Token::Plain
            };
            b.push(word, class);
            plain_start = i;
            continue;
        }
        i += utf8_char_len(ch);
    }
    flush_plain(line, plain_start, line.len(), b);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::builtin;

    #[test]
    fn detects_languages() {
        assert_eq!(Language::from_path(Path::new("main.rs")), Language::Rust);
        assert_eq!(Language::from_path(Path::new("app.tsx")), Language::TypeScript);
        assert_eq!(Language::from_path(Path::new("Dockerfile")), Language::Shell);
    }

    #[test]
    fn highlight_preserves_text() {
        let text = "fn main() {\n    let x = \"hello\"; // hi\n}\n";
        for lang in [Language::Rust, Language::Python, Language::Html] {
            let job = highlight(text, lang, FontId::monospace(14.0), f32::INFINITY, &builtin("Kodemo Dark").unwrap());
            assert_eq!(job.text, text);
        }
    }
}
