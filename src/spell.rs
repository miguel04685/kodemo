//! A small offline spell checker for prose (comments, strings and Markdown).
//!
//! Kodemo deliberately checks only human-language regions of a file. Code
//! identifiers are not underlined, which keeps squiggles meaningful on source
//! files while still flagging typos in comments, string literals and Markdown.

use crate::assist;
use crate::syntax::Language;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

const ENGLISH: &str = include_str!("../assets/dict/en_words.txt");
const PORTUGUESE: &str = include_str!("../assets/dict/pt_words.txt");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Issue {
    /// Character index (not bytes).
    pub start: usize,
    /// Character index, exclusive.
    pub end: usize,
}

pub fn issues(text: &str, language: Language) -> Vec<Issue> {
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return Vec::new();
    }
    let prose = prose_ranges(&chars, language);
    if prose.is_empty() {
        return Vec::new();
    }
    let mut mask = vec![false; chars.len()];
    for (start, end) in prose {
        let end = end.min(chars.len());
        if start < end {
            mask[start..end].fill(true);
        }
    }

    // A word that the author already uses elsewhere in the document is almost
    // always intentional (a name, product or domain term), so collect counts
    // before deciding what to underline.
    let words = collect_words(&chars, &mask);
    let mut counts: HashMap<String, u8> = HashMap::new();
    for (start, end, _) in &words {
        let lower = chars[*start..*end].iter().collect::<String>().to_lowercase();
        *counts.entry(lower).or_insert(0) = counts.get(&lower).copied().unwrap_or(0).saturating_add(1);
    }

    // Identifiers used in code should not be underlined when they reappear in
    // a comment or string. HTML attributes are values rather than identifiers,
    // so they are intentionally not part of this set.
    let mut code_words: HashSet<String> = HashSet::new();
    if matches!(
        language,
        Language::Rust
            | Language::Python
            | Language::JavaScript
            | Language::TypeScript
            | Language::Css
            | Language::Json
            | Language::Toml
            | Language::Yaml
            | Language::Shell
            | Language::Sql
            | Language::C
            | Language::Cpp
            | Language::Php
            | Language::Java
            | Language::Go
            | Language::CSharp
            | Language::Kotlin
            | Language::Lua
            | Language::Swift
            | Language::Ruby
            | Language::Dart
            | Language::Scala
            | Language::R
            | Language::Perl
            | Language::Elixir
            | Language::Haskell
            | Language::Zig
            | Language::Nim
            | Language::Julia
            | Language::PowerShell
            | Language::Assembly
            | Language::Solidity
            | Language::ObjectiveC
            | Language::Groovy
    ) {
        let inverse: Vec<bool> = mask.iter().map(|flag| !flag).collect();
        for (start, end, _) in collect_words(&chars, &inverse) {
            code_words.insert(chars[start..end].iter().collect::<String>().to_lowercase());
        }
    }

    let dict = dictionary();
    let mut issues = Vec::new();
    for (start, end, has_apostrophe) in words {
        let mut word: String = chars[start..end].iter().collect();
        while word.ends_with(['\'', '’']) {
            word.pop();
        }
        if !should_check(&word, has_apostrophe) {
            continue;
        }
        let lower = word.to_lowercase();
        if counts.get(&lower).copied().unwrap_or(0) > 1 || code_words.contains(&lower) {
            continue;
        }
        if in_url(&chars, start, end) {
            continue;
        }
        if is_known(&lower, dict) {
            continue;
        }
        issues.push(Issue { start, end });
        if issues.len() >= 400 {
            break;
        }
    }
    issues
}

fn should_check(word: &str, has_apostrophe: bool) -> bool {
    let count = word.chars().count();
    if !(4..=32).contains(&count) {
        return false;
    }
    let mut chars = word.chars();
    let first = chars.next().unwrap_or(' ');
    if !first.is_alphabetic() {
        return false;
    }
    // Acronyms and camelCase/PascalCase identifiers are intentional.
    if word.chars().all(|ch| !ch.is_lowercase()) {
        return false;
    }
    if word.chars().skip(1).any(|ch| ch.is_uppercase()) {
        return false;
    }
    if !word
        .chars()
        .all(|ch| ch.is_alphabetic() || (has_apostrophe && (ch == '\'' || ch == '’')))
    {
        return false;
    }
    true
}

fn is_known(word: &str, dict: &HashSet<String>) -> bool {
    if dict.contains(word) {
        return true;
    }
    // English contractions are often split in frequency lists. Accept a known
    // stem plus a standard contraction suffix.
    if let Some((stem, suffix)) = word.split_once('\'') {
        if matches!(suffix, "s" | "t" | "re" | "ve" | "ll" | "d" | "m") {
            if dict.contains(stem) || matches!(stem, "i" | "you" | "we" | "they" | "he" | "she") {
                return true;
            }
        }
    }
    false
}

fn dictionary() -> &'static HashSet<String> {
    static DICTIONARY: OnceLock<HashSet<String>> = OnceLock::new();
    DICTIONARY.get_or_init(|| {
        let mut set = HashSet::with_capacity(130_000);
        for line in ENGLISH.lines().chain(PORTUGUESE.lines()) {
            let word = line.trim();
            if !word.is_empty() {
                set.insert(word.to_lowercase());
            }
        }
        for word in assist::all_language_words() {
            set.insert(word.to_lowercase());
        }
        for word in TECH_WORDS {
            set.insert((*word).to_lowercase());
        }
        set
    })
}

const TECH_WORDS: &[&str] = &[
    "api", "app", "async", "backend", "backends", "boolean", "bugfix", "changelog", "cli",
    "codebase", "codemo", "component", "config", "configs", "configurable", "configuring",
    "crate", "crates", "css", "dataset", "datasets", "deserialize", "deserialized", "dev",
    "devs", "docs", "dropdown", "dropdowns", "egui", "endpoint", "endpoints", "enum", "enums",
    "favicon", "frontend", "frontends", "gguf", "glsl", "gpu", "gui", "gzip", "html", "http",
    "https", "ide", "ides", "iframe", "iframes", "img", "init", "initialize", "initialized",
    "javascript", "json", "jsonc", "kodemo", "llama", "localhost", "macos", "markdown",
    "middleware", "minimap", "multithreaded", "nodejs", "npm", "offscreen", "ok", "okay",
    "opengl", "php", "png", "preview", "previews", "qwen", "readme", "readmes", "repo",
    "repos", "runtime", "rust", "rustc", "screenshot", "screenshots", "scrollbar", "serde",
    "settings", "sidebar", "snippets", "stderr", "stdin", "stdout", "svg", "toml", "toolbar",
    "tooltip", "tooltips", "typescript", "ui", "uis", "url", "urls", "utf", "ux", "webview",
    "webkit", "webview", "x11", "xml", "yaml", "zip", "don't", "doesn't", "isn't", "aren't",
    "wasn't", "weren't", "can't", "cannot", "won't", "wouldn't", "shouldn't", "couldn't",
    "didn't", "hasn't", "haven't", "hadn't", "it's", "that's", "there's", "what's", "let's",
    "i'm", "i've", "i'll", "i'd", "you're", "you've", "you'll", "we're", "they're", "he's",
    "she's", "won’t", "isn’t", "can’t", "don’t", "it’s", "that’s",
];

/// Words with their ranges, only where the prose mask is set.
fn collect_words(chars: &[char], mask: &[bool]) -> Vec<(usize, usize, bool)> {
    let mut words = Vec::new();
    let mut start = None;
    let mut apostrophe = false;
    for (index, ch) in chars.iter().copied().enumerate() {
        let is_letter = ch.is_alphabetic();
        let is_apostrophe = ch == '\'' || ch == '’';
        if mask[index] && (is_letter || (is_apostrophe && start.is_some())) {
            if start.is_none() {
                start = Some(index);
                apostrophe = false;
            }
            if is_apostrophe {
                apostrophe = true;
            }
        } else if let Some(begin) = start.take() {
            if index > begin {
                words.push((begin, index, apostrophe));
            }
            apostrophe = false;
        }
    }
    if let Some(begin) = start {
        if chars.len() > begin {
            words.push((begin, chars.len(), apostrophe));
        }
    }
    words
}

fn in_url(chars: &[char], start: usize, end: usize) -> bool {
    let look_back_start = start.saturating_sub(40);
    let mut since_space = look_back_start;
    for index in (look_back_start..start).rev() {
        if chars[index].is_whitespace() {
            since_space = index + 1;
            break;
        }
        since_space = index;
    }
    let before: String = chars[since_space..start].iter().collect();
    if before.contains("://")
        || before.ends_with('/')
        || before.ends_with('\\')
        || before.ends_with('@')
        || before.contains('@')
    {
        return true;
    }
    // A slash, backslash or colon immediately before the word makes it part of
    // a path, URL or CSS selector rather than prose.
    if start > 0 && matches!(chars[start - 1], '/' | '\\' | ':') {
        return true;
    }
    let after_end = (end + 18).min(chars.len());
    let after: String = chars[end..after_end].iter().collect();
    let immediate = after.trim_start();
    if immediate.starts_with("://") || immediate.starts_with(".com") || immediate.starts_with(".org") {
        return true;
    }
    false
}

fn prose_ranges(chars: &[char], language: Language) -> Vec<(usize, usize)> {
    match language {
        Language::Plain => vec![(0, chars.len())],
        Language::Markdown => markdown_ranges(chars),
        Language::Html => html_ranges(chars),
        _ => code_ranges(chars, language),
    }
}

fn code_ranges(chars: &[char], language: Language) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let n = chars.len();
    let line_comment = matches!(
        language,
        Language::JavaScript
            | Language::TypeScript
            | Language::Rust
            | Language::C
            | Language::Cpp
            | Language::Java
            | Language::Go
            | Language::Php
            | Language::Json
            | Language::Css
            | Language::CSharp
            | Language::Kotlin
            | Language::Swift
            | Language::Dart
            | Language::Scala
            | Language::Zig
            | Language::Solidity
            | Language::ObjectiveC
            | Language::Groovy
    );
    let slash_slash = matches!(
        language,
        Language::JavaScript
            | Language::TypeScript
            | Language::Rust
            | Language::C
            | Language::Cpp
            | Language::Java
            | Language::Go
            | Language::Php
            | Language::Json
            | Language::CSharp
            | Language::Kotlin
            | Language::Swift
            | Language::Dart
            | Language::Scala
            | Language::Zig
            | Language::Solidity
            | Language::ObjectiveC
            | Language::Groovy
    );
    let hash_comment = matches!(
        language,
        Language::Python
            | Language::Shell
            | Language::Toml
            | Language::Yaml
            | Language::Php
            | Language::Ruby
            | Language::R
            | Language::Perl
            | Language::Elixir
            | Language::Nim
            | Language::Julia
            | Language::PowerShell
    );
    let dash_comment = matches!(language, Language::Sql | Language::Lua | Language::Haskell);
    let semicolon_comment = matches!(language, Language::Assembly);
    let sql_comment = matches!(language, Language::Sql);
    let mut i = 0;
    while i < n {
        let ch = chars[i];
        let next = chars.get(i + 1).copied();
        if slash_slash && ch == '/' && next == Some('/') {
            let end = find_line_end(chars, i + 2);
            push_range(&mut ranges, i + 2, end);
            i = end;
            continue;
        }
        if (line_comment || sql_comment) && ch == '/' && next == Some('*') {
            let end = find_sequence(chars, i + 2, &['*', '/']).unwrap_or(n);
            push_range(&mut ranges, i + 2, end);
            i = (end + 2).min(n);
            continue;
        }
        if dash_comment && ch == '-' && next == Some('-') {
            let end = find_line_end(chars, i + 2);
            push_range(&mut ranges, i + 2, end);
            i = end;
            continue;
        }
        if semicolon_comment && ch == ';' {
            let end = find_line_end(chars, i + 1);
            push_range(&mut ranges, i + 1, end);
            i = end;
            continue;
        }
        if hash_comment && ch == '#' {
            let allowed = match language {
                Language::Python | Language::Php => true,
                _ => i == 0 || chars.get(i.saturating_sub(1)).copied().unwrap_or(' ').is_whitespace(),
            };
            if allowed {
                let end = find_line_end(chars, i + 1);
                push_range(&mut ranges, i + 1, end);
                i = end;
                continue;
            }
        }
        if ch == '"' || ch == '\'' || ch == '`' {
            // Rust lifetimes (`'static`) are not strings. Standalone char
            // literals are skipped entirely; double-quoted strings are prose.
            if language == Language::Rust && ch == '\'' {
                if chars.get(i + 2).copied() == Some('\'') {
                    i += 3;
                } else {
                    i += 1;
                }
                continue;
            }
            if language == Language::Python
                && next == Some(ch)
                && chars.get(i + 2).copied() == Some(ch)
            {
                let end = find_triple(chars, i + 3, ch).unwrap_or(n);
                push_range(&mut ranges, i + 3, end);
                i = (end + 3).min(n);
                continue;
            }
            let end = find_string_end(chars, i + 1, ch).unwrap_or(n);
            push_range(&mut ranges, i + 1, end);
            i = end.saturating_add(1).min(n);
            continue;
        }
        i += 1;
    }
    ranges
}

fn html_ranges(chars: &[char]) -> Vec<(usize, usize)> {
    let n = chars.len();
    let mut ranges = Vec::new();
    let mut i = 0;
    while i < n {
        if starts_with(chars, i, "<!--") {
            let end = find_str(chars, i + 4, "-->").unwrap_or(n);
            push_range(&mut ranges, i + 4, end);
            i = (end + 3).min(n);
            continue;
        }
        if chars[i] == '<' {
            let tag_start = i;
            let mut j = i + 1;
            let mut quote: Option<char> = None;
            while j < n {
                let ch = chars[j];
                if let Some(q) = quote {
                    if ch == q {
                        quote = None;
                    }
                } else if ch == '"' || ch == '\'' {
                    quote = Some(ch);
                } else if ch == '>' {
                    break;
                }
                j += 1;
            }
            let mut name_start = tag_start + 1;
            while chars.get(name_start).is_some_and(|ch| *ch == '/' || *ch == '!') {
                name_start += 1;
            }
            let name: String = chars[name_start..j.min(n)]
                .iter()
                .take_while(|ch| ch.is_ascii_alphanumeric())
                .collect();
            i = (j + 1).min(n);
            if name.eq_ignore_ascii_case("script") {
                i = find_tag_close(chars, i, "script");
            } else if name.eq_ignore_ascii_case("style") {
                i = find_tag_close(chars, i, "style");
            }
            continue;
        }
        let start = i;
        while i < n && chars[i] != '<' {
            i += 1;
        }
        push_range(&mut ranges, start, i);
    }
    ranges
}

fn markdown_ranges(chars: &[char]) -> Vec<(usize, usize)> {
    let n = chars.len();
    let mut ranges = Vec::new();
    let mut i = 0;
    let mut in_fence = false;
    let mut fence_char = '`';
    let mut fence_len = 0;
    while i < n {
        let line_end = find_line_end(chars, i);
        let line: String = chars[i..line_end].iter().collect();
        let trimmed = line.trim_start();
        let fence_run = trimmed.chars().take_while(|ch| *ch == '`' || *ch == '~').count();
        if fence_run >= 3 {
            let ch = trimmed.chars().next().unwrap_or('`');
            if !in_fence {
                in_fence = true;
                fence_char = ch;
                fence_len = fence_run;
            } else if ch == fence_char && fence_run >= fence_len {
                in_fence = false;
            }
            i = (line_end + 1).min(n);
            continue;
        }
        if !in_fence {
            markdown_line(chars, i, line_end, &mut ranges);
        }
        i = (line_end + 1).min(n);
    }
    ranges
}

fn markdown_line(chars: &[char], start: usize, end: usize, ranges: &mut Vec<(usize, usize)>) {
    let mut i = start;
    let mut prose_start = start;
    while i < end {
        match chars[i] {
            '`' => {
                push_range(ranges, prose_start, i);
                let run = chars[i..end].iter().take_while(|ch| **ch == '`').count();
                if let Some(close) = find_n(chars, i + run, '`', run) {
                    i = close + run;
                } else {
                    i = end;
                }
                prose_start = i;
            }
            '<' => {
                push_range(ranges, prose_start, i);
                let close = find_char(chars, i + 1, '>').unwrap_or(end);
                i = (close + 1).min(end);
                prose_start = i;
            }
            _ => i += 1,
        }
    }
    push_range(ranges, prose_start, end);
}

fn push_range(ranges: &mut Vec<(usize, usize)>, start: usize, end: usize) {
    if start < end {
        ranges.push((start, end));
    }
}

fn find_line_end(chars: &[char], from: usize) -> usize {
    chars[from.min(chars.len())..]
        .iter()
        .position(|ch| *ch == '\n')
        .map(|offset| from + offset)
        .unwrap_or(chars.len())
}

fn find_sequence(chars: &[char], from: usize, needle: &[char]) -> Option<usize> {
    if needle.is_empty() {
        return Some(from);
    }
    let mut i = from;
    while i + needle.len() <= chars.len() {
        if chars[i..i + needle.len()] == *needle {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn find_str(chars: &[char], from: usize, needle: &str) -> Option<usize> {
    let needle: Vec<char> = needle.chars().collect();
    find_sequence(chars, from, &needle)
}

fn starts_with(chars: &[char], at: usize, needle: &str) -> bool {
    let mut i = at;
    for ch in needle.chars() {
        if chars.get(i).copied() != Some(ch) {
            return false;
        }
        i += 1;
    }
    true
}

fn find_string_end(chars: &[char], from: usize, quote: char) -> Option<usize> {
    let mut i = from;
    while i < chars.len() {
        if chars[i] == '\\' {
            i += 2;
            continue;
        }
        if chars[i] == quote {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn find_triple(chars: &[char], from: usize, quote: char) -> Option<usize> {
    let mut i = from;
    while i + 2 < chars.len() {
        if chars[i] == quote && chars[i + 1] == quote && chars[i + 2] == quote {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn find_tag_close(chars: &[char], from: usize, tag: &str) -> usize {
    let close = format!("</{tag}");
    let lower: Vec<char> = chars[from..]
        .iter()
        .map(|ch| ch.to_ascii_lowercase())
        .collect();
    let needle: Vec<char> = close.chars().collect();
    find_sequence(&lower, 0, &needle)
        .map(|offset| from + offset)
        .unwrap_or(chars.len())
}

fn find_char(chars: &[char], from: usize, needle: char) -> Option<usize> {
    let mut i = from;
    while i < chars.len() {
        if chars[i] == needle {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn find_n(chars: &[char], from: usize, needle: char, count: usize) -> Option<usize> {
    let mut i = from;
    while i + count <= chars.len() {
        if (0..count).all(|offset| chars[i + offset] == needle) {
            return Some(i);
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bad(text: &str, language: Language) -> Vec<String> {
        let chars: Vec<char> = text.chars().collect();
        issues(text, language)
            .into_iter()
            .map(|issue| chars[issue.start..issue.end].iter().collect())
            .collect()
    }

    #[test]
    fn marks_typos_in_markdown() {
        let words = bad("# Notes\n\nThis is a qwzx sentence.\n", Language::Markdown);
        assert_eq!(words, vec!["qwzx"]);
    }

    #[test]
    fn accepts_english_and_portuguese_words() {
        assert!(bad("The configuration is ready.\n", Language::Markdown).is_empty());
        assert!(bad("A configuração não está pronta.\n", Language::Markdown).is_empty());
    }

    #[test]
    fn ignores_identifiers_in_code() {
        let source = "let qwzx = 1; // this qwzx comment is fine\n";
        assert!(bad(source, Language::JavaScript).is_empty());
    }

    #[test]
    fn checks_comments_and_strings() {
        let source = "const value = \"qwzx\"; // another zzzt\n";
        let words = bad(source, Language::JavaScript);
        assert!(words.contains(&"qwzx".to_owned()));
        assert!(words.contains(&"zzzt".to_owned()));
    }

    #[test]
    fn checks_html_text_but_not_attributes() {
        let source = "<div class=\"qwzx\">qwzx</div>\n";
        assert_eq!(bad(source, Language::Html), vec!["qwzx"]);
    }

    #[test]
    fn skips_fenced_markdown_code() {
        let source = "A qwzx word.\n\n```rust\nlet qwzx = 1;\n```\n";
        assert_eq!(bad(source, Language::Markdown), vec!["qwzx"]);
    }

    #[test]
    fn rust_lifetimes_do_not_swallow_comments() {
        let source = "fn main<'a>(x: &'a str) { let _ = x; } // zzzt\n";
        assert_eq!(bad(source, Language::Rust), vec!["zzzt"]);
    }

    #[test]
    fn skips_urls() {
        let source = "See https://example.com/qwzx for details.\n";
        assert!(bad(source, Language::Markdown).is_empty());
    }

    #[test]
    fn counts_chars_with_unicode() {
        let text = "A configuração não está pronta. qwzx";
        let result = issues(text, Language::Plain);
        assert_eq!(result.len(), 1);
        let chars: Vec<char> = text.chars().collect();
        assert_eq!(chars[result[0].start..result[0].end].iter().collect::<String>(), "qwzx");
    }
}
