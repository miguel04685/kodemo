//! Language-aware word suggestions.
//!
//! The completion popup is intentionally lightweight: it offers words from the
//! active language (keywords, tags, properties, builtins) plus identifiers that
//! already exist in the current document. No model, no network, no allocation
//! spikes while typing.
//!
//! The lexicons below are deliberately broad. They are plain static slices, so
//! they cost nothing at startup beyond the bytes in the binary, and they make
//! the popup useful long before the local AI has warmed up.

use crate::syntax::Language;
use crate::util;
use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompletionItem {
    pub text: String,
    /// Short label shown on the right side of the popup row.
    pub detail: &'static str,
    /// Language words are rendered with an accent badge; document words use a
    /// quieter one.
    pub keyword: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WordContext {
    pub start: usize,
    pub prefix: String,
}

/// The word being typed immediately before `cursor` (character index).
/// Returns the character index where the word starts and the text itself.
pub fn word_at_cursor(text: &str, cursor: usize) -> Option<WordContext> {
    let cursor = cursor.min(text.chars().count());
    let byte = util::char_to_byte(text, cursor);
    if byte == 0 {
        return None;
    }
    let before = &text[..byte];
    let mut start_byte = byte;
    for (index, ch) in before.char_indices().rev() {
        if is_word_char(ch) {
            start_byte = index;
        } else {
            break;
        }
    }
    // CSS custom properties and vendor prefixes start with `-`; the dash is
    // part of the word, but we only replace the identifier that follows it.
    while let Some(ch) = text[start_byte..byte].chars().next() {
        if ch == '-' {
            start_byte += ch.len_utf8();
        } else {
            break;
        }
    }
    let prefix = &text[start_byte..byte];
    let first = prefix.chars().next()?;
    if !first.is_alphabetic() {
        return None;
    }
    Some(WordContext {
        start: util::byte_to_char(text, start_byte),
        prefix: prefix.to_owned(),
    })
}

fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_' || ch == '$' || ch == '-'
}

pub fn completions(
    language: Language,
    prefix: &str,
    text: &str,
    cursor: usize,
    limit: usize,
) -> Vec<CompletionItem> {
    if prefix.is_empty() || limit == 0 {
        return Vec::new();
    }
    let lower = prefix.to_lowercase();
    let case_sensitive = prefix.chars().any(|ch| ch.is_uppercase());
    let mut seen = HashSet::new();
    // (kind, match position, length, item). Kind: 0 document identifier,
    // 1 shell/build command, 2 language word.
    let mut items: Vec<(u8, usize, usize, CompletionItem)> = Vec::new();

    let matches = |word: &str| -> Option<usize> {
        if case_sensitive {
            if let Some(pos) = word.find(prefix) {
                return Some(if pos == 0 { 0 } else { pos + 1 });
            }
        } else {
            let haystack = word.to_lowercase();
            if let Some(pos) = haystack.find(&lower) {
                // Prefix hits always rank before mid-word hits.
                return Some(if pos == 0 { 0 } else { pos + 1 });
            }
        }
        None
    };

    // Identifiers from the open document are often more useful than generic
    // keywords, so give them the first rank.
    'doc: for word in document_words(text) {
        if word == prefix {
            continue;
        }
        let Some(position) = matches(word) else {
            continue;
        };
        if !seen.insert(word.to_owned()) {
            continue;
        }
        items.push((
            0,
            position,
            word.chars().count(),
            CompletionItem {
                text: word.to_owned(),
                detail: "document",
                keyword: false,
            },
        ));
        if items.len() >= limit * 8 {
            break 'doc;
        }
    }

    for slice in lexicon_slices(language) {
        for word in *slice {
            // Skip only the exact word being typed: a same-length match like
            // `Vec` for `vec` is a useful capitalisation fix.
            if *word == prefix {
                continue;
            }
            let Some(position) = matches(word) else {
                continue;
            };
            if !seen.insert((*word).to_owned()) {
                continue;
            }
            items.push((
                2,
                position,
                word.chars().count(),
                CompletionItem {
                    text: (*word).to_owned(),
                    detail: "keyword",
                    keyword: true,
                },
            ));
        }
    }

    // Build, run and test commands are only offered on a fresh line, where
    // inserting a whole command is useful instead of replacing an identifier.
    let before: String = text.chars().take(cursor).collect();
    let line = before.rsplit('\n').next().unwrap_or_default();
    let stem = line.strip_suffix(prefix).unwrap_or(line);
    if stem.trim().is_empty() {
        for command in COMMANDS {
            let Some(position) = matches(command) else {
                continue;
            };
            if !seen.insert((*command).to_owned()) {
                continue;
            }
            items.push((
                1,
                position,
                0,
                CompletionItem {
                    text: (*command).to_owned(),
                    detail: "command",
                    keyword: false,
                },
            ));
        }
    }

    // Prefix matches come before mid-word matches, then shorter words, then
    // document identifiers, commands and language words in that order. This
    // keeps the first row stable while the user types.
    items.sort_by(|a, b| {
        a.1.cmp(&b.1)
            .then_with(|| a.2.cmp(&b.2))
            .then_with(|| a.0.cmp(&b.0))
            .then_with(|| a.3.text.to_lowercase().cmp(&b.3.text.to_lowercase()))
    });
    items
        .into_iter()
        .take(limit)
        .map(|(_, _, _, item)| item)
        .collect()
}

fn document_words(text: &str) -> impl Iterator<Item = &str> {
    text.split(|ch: char| !(ch.is_alphanumeric() || ch == '_' || ch == '$'))
        .filter(|word| {
            let mut chars = word.chars();
            let Some(first) = chars.next() else {
                return false;
            };
            first.is_alphabetic() && word.chars().count() >= 2 && word.len() <= 48
        })
}

const COMMON: &[&str] = &[
    "abstract", "add", "after", "alias", "all", "allow", "and", "any", "append", "apply",
    "argument", "args", "array", "as", "assert", "async", "attribute", "auto", "await", "base",
    "before", "begin", "bind", "body", "bool", "boolean", "break", "buffer", "build", "button",
    "byte", "cache", "call", "callback", "cancel", "case", "catch", "channel", "char", "check",
    "child", "class", "clear", "click", "client", "clone", "close", "code", "collection", "color",
    "column", "command", "comment", "commit", "compare", "compile", "component", "concat",
    "config", "connection", "console", "const", "constant", "constructor", "content", "context",
    "continue", "convert", "copy", "count", "create", "current", "cursor", "data", "database",
    "date", "debug", "decimal", "declare", "default", "define", "delay", "delete", "description",
    "destroy", "detail", "device", "directory", "disable", "display", "document", "domain",
    "done", "double", "driver", "dump", "duration", "dynamic", "each", "edit", "element",
    "else", "empty", "enable", "encode", "end", "entry", "enum", "env", "equal", "error",
    "event", "exception", "exists", "exit", "export", "extend", "extends", "external", "false",
    "fetch", "field", "file", "filter", "final", "find", "first", "flat", "float", "folder",
    "for", "foreach", "format", "from", "front", "function", "generate", "get", "global",
    "goto", "group", "handle", "handler", "hash", "head", "header", "height", "help", "hidden",
    "history", "hook", "host", "hover", "html", "http", "id", "identifier", "if", "image",
    "import", "in", "include", "index", "info", "init", "inline", "input", "insert", "instance",
    "int", "integer", "interface", "internal", "interval", "invalid", "is", "item", "iterate",
    "join", "json", "key", "keys", "keyword", "label", "last", "layout", "left", "length",
    "let", "level", "library", "license", "line", "link", "list", "listen", "load", "local",
    "lock", "log", "login", "loop", "main", "make", "map", "margin", "match", "max", "menu",
    "message", "method", "middleware", "min", "mode", "model", "modify", "module", "move",
    "name", "namespace", "native", "new", "next", "node", "none", "normal", "not", "null",
    "number", "object", "offset", "on", "open", "option", "options", "or", "order", "output",
    "override", "owner", "package", "page", "pair", "param", "params", "parent", "parse",
    "parser", "partition", "pass", "path", "pattern", "payload", "pending", "permission",
    "pointer", "pop", "port", "position", "post", "prefix", "prepend", "preview", "previous",
    "print", "private", "process", "promise", "property", "protected", "protocol", "public",
    "push", "query", "queue", "range", "read", "readme", "record", "reduce", "ref", "reference",
    "regex", "register", "remove", "render", "repeat", "replace", "reply", "request", "require",
    "reset", "resolve", "resource", "response", "result", "retry", "return", "reverse",
    "revision", "right", "role", "root", "route", "router", "row", "run", "sample", "save",
    "schedule", "schema", "scope", "screen", "script", "scroll", "search", "select", "self",
    "send", "sequence", "server", "service", "session", "set", "setting", "setup", "shared",
    "short", "show", "signal", "size", "slice", "sort", "source", "split", "stack", "start",
    "state", "static", "status", "step", "stop", "storage", "stream", "string", "struct",
    "style", "submit", "success", "sum", "super", "switch", "sync", "system", "table", "tag",
    "target", "task", "template", "test", "text", "then", "this", "thread", "throw", "time",
    "timeout", "timestamp", "title", "to", "token", "tooltip", "topic", "toString", "total",
    "transform", "tree", "trigger", "true", "try", "type", "typeof", "undefined", "unique",
    "unit", "update", "upload", "url", "use", "user", "util", "valid", "validate", "value",
    "values", "variable", "version", "view", "virtual", "visible", "void", "warning", "web",
    "weight", "while", "width", "window", "with", "work", "workspace", "wrap", "write", "yield",
    "zoom", "kodemo", "json", "config",
];

const HTML_TAGS: &[&str] = &[
    "a", "abbr", "address", "area", "article", "aside", "audio", "b", "base", "bdi", "bdo",
    "blockquote", "body", "br", "button", "canvas", "caption", "cite", "code", "col", "colgroup",
    "data", "datalist", "dd", "del", "details", "dfn", "dialog", "div", "dl", "dt", "em",
    "embed", "fieldset", "figcaption", "figure", "footer", "form", "h1", "h2", "h3", "h4", "h5",
    "h6", "head", "header", "hgroup", "hr", "html", "i", "iframe", "img", "input", "ins", "kbd",
    "label", "legend", "li", "link", "main", "map", "mark", "menu", "meta", "meter", "nav",
    "noscript", "object", "ol", "optgroup", "option", "output", "p", "picture", "pre", "progress",
    "q", "rp", "rt", "ruby", "s", "samp", "script", "section", "select", "slot", "small",
    "source", "span", "strong", "style", "sub", "summary", "sup", "table", "tbody", "td",
    "template", "textarea", "tfoot", "th", "thead", "time", "title", "tr", "track", "u", "ul",
    "var", "video", "wbr", "svg", "path", "circle", "rect", "line", "polygon", "polyline",
    "g", "defs", "use", "text", "tspan", "linearGradient", "radialGradient", "stop", "clipPath",
    "mask", "pattern", "foreignObject", "animate", "animateTransform", "filter", "feBlend",
    "feColorMatrix", "feGaussianBlur", "feOffset",
];

const HTML_ATTRS: &[&str] = &[
    "accept", "accept-charset", "accesskey", "action", "allow", "alt", "async", "autocomplete",
    "autofocus", "autoplay", "charset", "checked", "cite", "class", "cols", "colspan", "content",
    "contenteditable", "controls", "coords", "crossorigin", "data", "datetime", "decoding",
    "default", "defer", "dir", "dirname", "disabled", "download", "draggable", "enctype", "enterkeyhint",
    "for", "form", "formaction", "formenctype", "formmethod", "formnovalidate", "formtarget",
    "headers", "height", "hidden", "high", "href", "hreflang", "http-equiv", "id", "integrity",
    "ismap", "kind", "label", "lang", "list", "loading", "loop", "low", "max", "maxlength",
    "media", "method", "min", "minlength", "multiple", "muted", "name", "novalidate", "open",
    "optimum", "pattern", "placeholder", "playsinline", "poster", "preload", "readonly",
    "referrerpolicy", "rel", "required", "reversed", "role", "rows", "rowspan", "sandbox",
    "scope", "selected", "shape", "size", "sizes", "slot", "span", "spellcheck", "src", "srcset",
    "start", "step", "style", "tabindex", "target", "title", "translate", "type", "usemap",
    "value", "width", "wrap", "xmlns", "viewBox", "fill", "stroke", "stroke-width", "d",
    "cx", "cy", "r", "rx", "ry", "x", "y", "x1", "x2", "y1", "y2", "points", "transform",
    "aria-label", "aria-labelledby", "aria-describedby", "aria-hidden", "aria-live",
    "aria-expanded", "aria-controls", "aria-current", "aria-selected", "aria-checked",
    "aria-disabled", "aria-haspopup", "aria-pressed", "data-id", "data-name", "data-value",
    "data-target", "data-toggle", "data-role", "onclick", "onchange", "onsubmit", "oninput",
    "onkeydown", "onkeyup", "onkeypress", "onload", "onerror", "onmouseover", "onmouseout",
    "onfocus", "onblur", "onscroll", "onresize", "ondblclick", "oncontextmenu",
];

const CSS: &[&str] = &[
    "accent-color", "align-content", "align-items", "align-self", "all", "animation",
    "animation-delay", "animation-direction", "animation-duration", "animation-fill-mode",
    "animation-iteration-count", "animation-name", "animation-play-state",
    "animation-timing-function", "aspect-ratio", "backdrop-filter", "backface-visibility",
    "background", "background-attachment", "background-blend-mode", "background-clip",
    "background-color", "background-image", "background-origin", "background-position",
    "background-repeat", "background-size", "block-size", "border", "border-bottom",
    "border-bottom-color", "border-bottom-left-radius", "border-bottom-right-radius",
    "border-bottom-style", "border-bottom-width", "border-collapse", "border-color",
    "border-image", "border-left", "border-left-color", "border-left-style", "border-left-width",
    "border-radius", "border-right", "border-right-color", "border-spacing", "border-style",
    "border-top", "border-top-left-radius", "border-top-right-radius", "border-top-style",
    "border-top-width", "border-width", "bottom", "box-decoration-break", "box-shadow",
    "box-sizing", "break-after", "break-before", "break-inside", "caption-side", "caret-color",
    "clear", "clip", "clip-path", "color", "color-scheme", "column-count", "column-fill",
    "column-gap", "column-rule", "column-span", "column-width", "columns", "contain",
    "content", "content-visibility", "counter-increment", "counter-reset", "cursor",
    "direction", "display", "filter", "flex", "flex-basis", "flex-direction", "flex-flow",
    "flex-grow", "flex-shrink", "flex-wrap", "float", "font", "font-display", "font-family",
    "font-feature-settings", "font-size", "font-size-adjust", "font-stretch", "font-style",
    "font-variant", "font-weight", "gap", "grid", "grid-area", "grid-auto-columns",
    "grid-auto-flow", "grid-auto-rows", "grid-column", "grid-column-end", "grid-column-start",
    "grid-gap", "grid-row", "grid-row-end", "grid-row-start", "grid-template",
    "grid-template-areas", "grid-template-columns", "grid-template-rows", "height", "hyphens",
    "image-rendering", "inset", "isolation", "justify-content", "justify-items", "justify-self",
    "left", "letter-spacing", "line-break", "line-clamp", "line-height", "list-style",
    "list-style-image", "list-style-position", "list-style-type", "margin", "margin-bottom",
    "margin-left", "margin-right", "margin-top", "mask", "max-height", "max-width",
    "min-height", "min-width", "mix-blend-mode", "object-fit", "object-position", "opacity",
    "order", "orphans", "outline", "outline-color", "outline-offset", "outline-style",
    "outline-width", "overflow", "overflow-wrap", "overflow-x", "overflow-y", "overscroll-behavior",
    "padding", "padding-bottom", "padding-left", "padding-right", "padding-top", "page-break-after",
    "perspective", "place-content", "place-items", "place-self", "pointer-events", "position",
    "quotes", "resize", "right", "rotate", "row-gap", "scale", "scroll-behavior",
    "scroll-margin", "scroll-padding", "scroll-snap-align", "scroll-snap-type", "tab-size",
    "table-layout", "text-align", "text-decoration", "text-decoration-color",
    "text-decoration-line", "text-decoration-style", "text-emphasis", "text-indent",
    "text-overflow", "text-shadow", "text-transform", "text-wrap", "top", "touch-action",
    "transform", "transform-origin", "transform-style", "transition", "transition-delay",
    "transition-duration", "transition-property", "transition-timing-function", "translate",
    "unicode-bidi", "user-select", "vertical-align", "visibility", "white-space", "widows",
    "width", "will-change", "word-break", "word-spacing", "word-wrap", "writing-mode", "z-index",
    "absolute", "auto", "baseline", "block", "bold", "border-box", "both", "center", "column",
    "contain", "cover", "currentColor", "dashed", "dotted", "ease", "ease-in", "ease-in-out",
    "ease-out", "fixed", "flex", "flex-end", "flex-start", "grid", "hidden", "inherit",
    "initial", "inline", "inline-block", "inline-flex", "inline-grid", "italic", "justify",
    "left", "none", "normal", "nowrap", "pointer", "relative", "repeat", "revert", "row",
    "solid", "space-around", "space-between", "space-evenly", "sticky", "transparent",
    "underline", "unset", "uppercase", "visible", "wrap", "calc", "clamp", "min", "max", "var",
    "rgb", "rgba", "hsl", "hsla", "linear-gradient", "radial-gradient", "url", "hover", "focus",
    "active", "visited", "before", "after", "first-child", "last-child", "nth-child", "root",
    "media", "keyframes", "import", "supports", "font-face",
];

const JAVASCRIPT: &[&str] = &[
    "async", "await", "break", "case", "catch", "class", "const", "continue", "debugger",
    "default", "delete", "do", "else", "export", "extends", "finally", "for", "from", "function",
    "get", "if", "import", "in", "instanceof", "let", "new", "of", "return", "set", "static",
    "super", "switch", "this", "throw", "try", "typeof", "var", "void", "while", "with", "yield",
    "arguments", "Array", "ArrayBuffer", "BigInt", "Boolean", "Date", "Error", "EvalError",
    "Function", "Infinity", "JSON", "Map", "Math", "NaN", "Number", "Object", "Promise", "Proxy",
    "RangeError", "ReferenceError", "Reflect", "RegExp", "Set", "String", "Symbol", "SyntaxError",
    "TypeError", "URIError", "WeakMap", "WeakRef", "WeakSet", "Intl", "BigInt64Array",
    "Float32Array", "Float64Array", "Int8Array", "Int16Array", "Int32Array", "Uint8Array",
    "Uint8ClampedArray", "Uint16Array", "Uint32Array", "console", "document", "window",
    "globalThis", "navigator", "location", "history", "screen", "performance", "alert", "confirm",
    "prompt", "fetch", "setTimeout", "setInterval", "clearTimeout", "clearInterval",
    "requestAnimationFrame", "cancelAnimationFrame", "queueMicrotask", "structuredClone",
    "localStorage", "sessionStorage", "indexedDB", "customElements", "addEventListener",
    "removeEventListener", "dispatchEvent", "querySelector", "querySelectorAll", "getElementById",
    "getElementsByClassName", "getElementsByTagName", "createElement", "createTextNode",
    "appendChild", "removeChild", "replaceChild", "insertBefore", "append", "prepend", "remove",
    "cloneNode", "classList", "className", "textContent", "innerHTML", "innerText", "outerHTML",
    "dataset", "style", "value", "checked", "selected", "disabled", "preventDefault",
    "stopPropagation", "stopImmediatePropagation", "then", "catch", "finally", "resolve",
    "reject", "all", "allSettled", "race", "any", "forEach", "map", "filter", "reduce",
    "reduceRight", "find", "findIndex", "findLast", "some", "every", "includes", "indexOf",
    "lastIndexOf", "push", "pop", "shift", "unshift", "slice", "splice", "concat", "join",
    "reverse", "sort", "flat", "flatMap", "fill", "copyWithin", "entries", "keys", "values",
    "split", "trim", "trimStart", "trimEnd", "toLowerCase", "toUpperCase", "startsWith",
    "endsWith", "replace", "replaceAll", "match", "matchAll", "search", "substring", "substr",
    "charAt", "charCodeAt", "codePointAt", "padStart", "padEnd", "repeat", "normalize", "test",
    "exec", "stringify", "parse", "assign", "freeze", "seal", "create", "defineProperty",
    "defineProperties", "getOwnPropertyNames", "getPrototypeOf", "setPrototypeOf", "hasOwnProperty",
    "isArray", "from", "isInteger", "isNaN", "isFinite", "prototype", "constructor", "length",
    "bind", "call", "apply", "toString", "valueOf", "toFixed", "toPrecision", "toISOString",
    "toLocaleDateString", "toLocaleTimeString", "getTime", "getFullYear", "getMonth", "getDate",
    "getHours", "getMinutes", "getSeconds", "setTimeout", "clearTimeout", "module", "require",
    "process", "Buffer", "dirname", "filename", "exports", "default", "global", "encodeURI",
    "encodeURIComponent", "decodeURIComponent", "parseInt", "parseFloat", "isNaN", "isFinite",
    "atob", "btoa", "crypto", "randomUUID", "getRandomValues", "event", "target", "currentTarget",
    "key", "code", "clientX", "clientY", "offsetX", "offsetY", "pageX", "pageY", "button",
    "touches", "changedTouches", "files", "formData", "headers", "method", "mode", "credentials",
    "signal", "abort", "json", "text", "blob", "arrayBuffer", "ok", "status", "statusText",
];

const TYPESCRIPT: &[&str] = &[
    "any", "as", "asserts", "bigint", "boolean", "declare", "enum", "implements", "infer",
    "interface", "is", "keyof", "namespace", "never", "number", "object", "override", "readonly",
    "satisfies", "string", "symbol", "type", "undefined", "unique", "unknown", "void", "satisfies",
    "abstract", "private", "protected", "public", "static", "Partial", "Required", "Readonly",
    "Record", "Pick", "Omit", "Exclude", "Extract", "NonNullable", "Parameters", "ReturnType",
    "InstanceType", "ConstructorParameters", "Awaited", "ThisType", "Uppercase", "Lowercase",
    "Capitalize", "Uncapitalize", "Promise", "PromiseLike", "Array", "ReadonlyArray", "Map",
    "ReadonlyMap", "Set", "ReadonlySet", "WeakMap", "WeakSet", "Date", "RegExp", "Error",
    "Function", "Object", "String", "Number", "Boolean", "BigInt", "Symbol", "Iterable",
    "Iterator", "AsyncIterable", "Generator", "AsyncGenerator", "JSON", "Math", "Intl",
    "PropertyKey", "TemplateStringsArray", "Exclude", "Extract", "Omit", "NonNullable",
];

const PYTHON: &[&str] = &[
    "and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del", "elif",
    "else", "except", "finally", "for", "from", "global", "if", "import", "in", "is", "lambda",
    "match", "case", "nonlocal", "not", "or", "pass", "raise", "return", "try", "while", "with",
    "yield", "None", "True", "False", "abs", "aiter", "all", "anext", "any", "ascii", "bin",
    "bool", "breakpoint", "bytearray", "bytes", "callable", "chr", "classmethod", "compile",
    "complex", "delattr", "dict", "dir", "divmod", "enumerate", "eval", "exec", "filter",
    "float", "format", "frozenset", "getattr", "globals", "hasattr", "hash", "help", "hex", "id",
    "input", "int", "isinstance", "issubclass", "iter", "len", "list", "locals", "map", "max",
    "memoryview", "min", "next", "object", "oct", "open", "ord", "pow", "print", "property",
    "range", "repr", "reversed", "round", "set", "setattr", "slice", "sorted", "staticmethod",
    "str", "sum", "super", "tuple", "type", "vars", "zip", "__init__", "__name__", "__main__",
    "__str__", "__repr__", "__len__", "__iter__", "__next__", "__enter__", "__exit__", "self",
    "cls", "Exception", "BaseException", "ArithmeticError", "AssertionError", "AttributeError",
    "ImportError", "ModuleNotFoundError", "IndexError", "KeyError", "NameError", "OSError",
    "FileNotFoundError", "PermissionError", "RuntimeError", "StopIteration", "TypeError",
    "ValueError", "ZeroDivisionError", "Warning", "DeprecationWarning", "collections",
    "defaultdict", "deque", "namedtuple", "OrderedDict", "Counter", "ChainMap", "dataclass",
    "field", "functools", "lru_cache", "partial", "reduce", "wraps", "itertools", "chain",
    "combinations", "permutations", "product", "groupby", "cycle", "islice", "pathlib", "Path",
    "PurePath", "os", "path", "sys", "argv", "exit", "stdin", "stdout", "stderr", "platform",
    "environ", "getenv", "json", "loads", "dumps", "load", "dump", "re", "compile", "search",
    "findall", "sub", "split", "match", "fullmatch", "escape", "math", "sqrt", "ceil", "floor",
    "pi", "inf", "nan", "sin", "cos", "tan", "log", "exp", "random", "randint", "choice",
    "shuffle", "time", "sleep", "perf_counter", "monotonic", "strftime", "strptime",
    "datetime", "date", "timedelta", "timezone", "typing", "Optional", "Union", "List", "Dict",
    "Tuple", "Set", "FrozenSet", "Any", "Callable", "Iterable", "Iterator", "Sequence",
    "Mapping", "MutableMapping", "TypeVar", "Generic", "Protocol", "Literal", "Final", "ClassVar",
    "cast", "get_type_hints", "asyncio", "run", "gather", "create_task", "sleep", "Queue",
    "threading", "Thread", "Lock", "Event", "subprocess", "run", "Popen", "check_output",
    "logging", "getLogger", "debug", "info", "warning", "error", "critical", "basicConfig",
    "unittest", "pytest", "fixture", "parametrize", "sqlite3", "connect", "execute",
    "fetchone", "fetchall", "commit", "close", "csv", "reader", "writer", "DictReader",
    "DictWriter", "argparse", "ArgumentParser", "add_argument", "parse_args", "main", "args",
    "kwargs", "setup", "teardown", "yield", "raise_for_status", "requests", "get", "post",
    "put", "delete", "session", "responses",
];

const PHP: &[&str] = &[
    "abstract", "and", "array", "as", "break", "callable", "case", "catch", "class", "clone",
    "const", "continue", "declare", "default", "do", "echo", "else", "elseif", "empty",
    "enddeclare", "endfor", "endforeach", "endif", "endswitch", "endwhile", "enum", "eval",
    "exit", "extends", "final", "finally", "fn", "for", "foreach", "function", "global", "goto",
    "if", "implements", "include", "include_once", "instanceof", "insteadof", "interface",
    "isset", "list", "match", "namespace", "new", "or", "print", "private", "protected",
    "public", "readonly", "require", "require_once", "return", "static", "switch", "throw",
    "trait", "try", "unset", "use", "var", "while", "xor", "yield", "from", "true", "false",
    "null", "self", "parent", "this", "string", "int", "float", "bool", "array", "object",
    "mixed", "void", "never", "iterable", "String", "strlen", "strpos", "stripos", "str_replace",
    "str_contains", "str_starts_with", "str_ends_with", "substr", "strtolower", "strtoupper",
    "ucfirst", "ucwords", "trim", "ltrim", "rtrim", "explode", "implode", "sprintf",
    "printf", "number_format", "count", "array_key_exists", "array_keys", "array_values",
    "array_map", "array_filter", "array_reduce", "array_merge", "array_merge_recursive",
    "array_slice", "array_splice", "array_search", "array_unique", "array_push", "array_pop",
    "array_shift", "array_unshift", "in_array", "sort", "rsort", "usort", "ksort", "asort",
    "json_encode", "json_decode", "serialize", "unserialize", "file_get_contents",
    "file_put_contents", "fopen", "fclose", "fread", "fwrite", "fgets", "file_exists", "is_file",
    "is_dir", "mkdir", "rmdir", "unlink", "scandir", "glob", "basename", "dirname", "realpath",
    "preg_match", "preg_match_all", "preg_replace", "preg_split", "htmlspecialchars",
    "html_entity_decode", "urlencode", "urldecode", "header", "setcookie", "session_start",
    "print_r", "var_dump", "var_export", "error_log", "date", "time", "strtotime", "mktime",
    "date_default_timezone_set", "function_exists", "class_exists", "method_exists",
    "property_exists", "call_user_func", "PDO", "Exception", "Error", "RuntimeException",
    "InvalidArgumentException", "stdClass", "Closure", "Generator", "Traversable", "Iterator",
    "ArrayAccess", "Countable", "Stringable", "self", "parent", "static", "namespace",
    "use", "autoload", "composer", "namespace", "interface", "trait",
];

const JAVA: &[&str] = &[
    "abstract", "assert", "boolean", "break", "byte", "case", "catch", "char", "class", "const",
    "continue", "default", "do", "double", "else", "enum", "extends", "final", "finally", "float",
    "for", "goto", "if", "implements", "import", "instanceof", "int", "interface", "long",
    "native", "new", "package", "private", "protected", "public", "record", "return", "sealed",
    "short", "static", "strictfp", "super", "switch", "synchronized", "this", "throw", "throws",
    "transient", "try", "var", "void", "volatile", "while", "true", "false", "null", "yield",
    "String", "Object", "System", "Math", "Integer", "Long", "Double", "Float", "Boolean",
    "Character", "Byte", "Short", "Number", "BigDecimal", "BigInteger", "List", "ArrayList",
    "LinkedList", "Map", "HashMap", "LinkedHashMap", "TreeMap", "Set", "HashSet", "TreeSet",
    "Queue", "Deque", "ArrayDeque", "PriorityQueue", "Optional", "Stream", "Collectors",
    "StringBuilder", "StringBuffer", "StringJoiner", "Arrays", "Collections", "Comparator",
    "Iterable", "Iterator", "Collection", "Exception", "RuntimeException",
    "IllegalArgumentException", "IllegalStateException", "NullPointerException", "IOException",
    "FileNotFoundException", "ClassNotFoundException", "Override", "Deprecated",
    "SuppressWarnings", "FunctionalInterface", "AutoCloseable", "Comparable", "Serializable",
    "Thread", "Runnable", "ExecutorService", "Executors", "CompletableFuture", "Duration",
    "Instant", "LocalDate", "LocalTime", "LocalDateTime", "DateTimeFormatter", "ZoneId",
    "Files", "Path", "Paths", "File", "Reader", "Writer", "BufferedReader", "BufferedWriter",
    "InputStream", "OutputStream", "Scanner", "println", "print", "printf", "format", "equals",
    "hashCode", "toString", "length", "size", "isEmpty", "get", "put", "add", "remove",
    "contains", "containsKey", "keySet", "values", "entrySet", "stream", "filter", "map",
    "collect", "forEach", "reduce", "sorted", "distinct", "limit", "skip", "toList", "of",
    "ofNullable", "orElse", "orElseThrow", "ifPresent", "isPresent", "parseInt", "parseDouble",
    "valueOf", "main", "args", "void", "extends", "implements", "instanceof", "this", "super",
];

const RUST: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
    "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move",
    "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true",
    "type", "unsafe", "use", "where", "while", "yield", "Box", "Option", "Result", "String",
    "str", "Vec", "HashMap", "HashSet", "BTreeMap", "BTreeSet", "VecDeque", "BinaryHeap", "Rc",
    "Arc", "Cell", "RefCell", "Mutex", "RwLock", "Cow", "Iterator", "IntoIterator", "From",
    "Into", "TryFrom", "TryInto", "Default", "Clone", "Copy", "Debug", "Display", "Error",
    "Send", "Sync", "Sized", "Fn", "FnMut", "FnOnce", "Some", "None", "Ok", "Err", "println",
    "print", "eprintln", "eprint", "format", "write", "writeln", "vec", "panic", "assert",
    "assert_eq", "assert_ne", "debug_assert", "matches", "todo", "unimplemented", "unreachable",
    "dbg", "include_str", "include_bytes", "concat", "stringify", "println", "unwrap",
    "expect", "iter", "into_iter", "iter_mut", "collect", "filter", "filter_map", "map",
    "flat_map", "fold", "for_each", "any", "all", "find", "position", "count", "sum", "product",
    "zip", "enumerate", "chain", "take", "skip", "rev", "peekable", "next", "push", "pop",
    "insert", "remove", "retain", "clear", "extend", "drain", "get", "get_mut", "len",
    "is_empty", "first", "last", "contains", "contains_key", "keys", "values", "entry", "or_insert",
    "to_string", "to_owned", "as_str", "as_ref", "as_mut", "as_slice", "clone", "into", "from",
    "parse", "split", "split_whitespace", "splitn", "lines", "trim", "trim_start", "trim_end",
    "starts_with", "ends_with", "replace", "chars", "bytes", "char_indices", "as_bytes",
    "to_lowercase", "to_uppercase", "parse", "spawn", "join", "sleep", "read", "write", "open",
    "create", "create_dir_all", "read_to_string", "write_all", "Path", "PathBuf", "File", "Read",
    "Write", "BufRead", "BufReader", "BufWriter", "io", "fs", "env", "args", "current_dir",
    "std", "core", "alloc", "cfg", "derive", "serde", "egui", "eframe", "egui_glow", "tokio",
    "chrono", "anyhow", "thiserror", "log", "tracing", "regex", "rand", "clap", "Kodemo",
];

const C: &[&str] = &[
    "auto", "break", "case", "char", "const", "continue", "default", "do", "double", "else",
    "enum", "extern", "float", "for", "goto", "if", "inline", "int", "long", "register",
    "restrict", "return", "short", "signed", "sizeof", "static", "struct", "switch", "typedef",
    "union", "unsigned", "void", "volatile", "while", "_Bool", "_Complex", "_Generic",
    "_Static_assert", "NULL", "size_t", "ssize_t", "ptrdiff_t", "intptr_t", "uintptr_t", "int8_t",
    "int16_t", "int32_t", "int64_t", "uint8_t", "uint16_t", "uint32_t", "uint64_t", "bool",
    "true", "false", "printf", "fprintf", "sprintf", "snprintf", "vprintf", "scanf", "fscanf",
    "sscanf", "puts", "putchar", "getchar", "gets", "fgets", "fputs", "perror", "malloc",
    "calloc", "realloc", "free", "memcpy", "memmove", "memset", "memcmp", "strlen", "strcmp",
    "strncmp", "strcpy", "strncpy", "strcat", "strncat", "strchr", "strrchr", "strstr",
    "strtok", "strdup", "atoi", "atol", "atof", "strtol", "strtoul", "strtod", "fopen",
    "fclose", "fread", "fwrite", "fseek", "ftell", "rewind", "fflush", "feof", "ferror", "stdin",
    "stdout", "stderr", "FILE", "exit", "abort", "atexit", "system", "getenv", "setenv", "main",
    "argc", "argv", "assert", "errno", "time", "clock", "difftime", "mktime", "localtime",
    "gmtime", "strftime", "sqrt", "pow", "abs", "fabs", "ceil", "floor", "round", "sin", "cos",
    "tan", "log", "log10", "exp", "rand", "srand", "RAND_MAX", "qsort", "bsearch", "isalpha",
    "isdigit", "isspace", "isupper", "islower", "toupper", "tolower", "isalnum", "isprint",
    "uint32", "int32", "uint64", "int64",
];

const CPP: &[&str] = &[
    "alignas", "alignof", "and", "and_eq", "asm", "bitand", "bitor", "catch", "class", "compl",
    "concept", "constexpr", "consteval", "constinit", "const_cast", "co_await", "co_return",
    "co_yield", "decltype", "delete", "dynamic_cast", "explicit", "export", "friend", "mutable",
    "namespace", "new", "noexcept", "not", "not_eq", "operator", "or", "or_eq", "private",
    "protected", "public", "reinterpret_cast", "requires", "static_assert", "static_cast",
    "template", "this", "thread_local", "throw", "try", "typeid", "typename", "using", "virtual",
    "xor", "xor_eq", "override", "final", "nullptr", "auto", "std", "string", "wstring",
    "string_view", "vector", "array", "deque", "list", "forward_list", "map", "unordered_map",
    "multimap", "set", "unordered_set", "multiset", "stack", "queue", "priority_queue", "pair",
    "tuple", "optional", "variant", "any", "function", "reference_wrapper", "unique_ptr",
    "shared_ptr", "weak_ptr", "make_unique", "make_shared", "cout", "cin", "cerr", "clog",
    "endl", "getline", "iostream", "fstream", "sstream", "iomanip", "algorithm", "numeric",
    "utility", "memory", "thread", "mutex", "atomic", "chrono", "filesystem", "regex", "random",
    "sort", "stable_sort", "find", "find_if", "for_each", "transform", "copy", "copy_if",
    "count", "count_if", "accumulate", "reduce", "inner_product", "min", "max", "min_element",
    "max_element", "swap", "move", "forward", "begin", "end", "size", "empty", "push_back",
    "emplace_back", "pop_back", "insert", "erase", "clear", "reserve", "resize", "at", "front",
    "back", "data", "c_str", "substr", "length", "compare", "append", "to_string", "stoi",
    "stol", "stod", "stof", "printf", "main", "int", "float", "double", "char", "bool", "void",
    "const", "static", "inline", "unsigned", "long", "short", "struct", "union", "enum",
];

const GO: &[&str] = &[
    "break", "case", "chan", "const", "continue", "default", "defer", "else", "fallthrough",
    "for", "func", "go", "goto", "if", "import", "interface", "map", "package", "range",
    "return", "select", "struct", "switch", "type", "var", "bool", "byte", "complex64",
    "complex128", "error", "float32", "float64", "int", "int8", "int16", "int32", "int64",
    "rune", "string", "uint", "uint8", "uint16", "uint32", "uint64", "uintptr", "true", "false",
    "nil", "iota", "append", "cap", "close", "complex", "copy", "delete", "imag", "len", "make",
    "new", "panic", "print", "println", "real", "recover", "clear", "min", "max", "fmt",
    "Println", "Printf", "Sprintf", "Errorf", "Sprint", "Scanln", "Scanf", "context", "Context",
    "Background", "WithCancel", "WithTimeout", "WithValue", "sync", "WaitGroup", "Mutex",
    "RWMutex", "Once", "Map", "errors", "New", "Is", "As", "Unwrap", "strings", "Contains",
    "HasPrefix", "HasSuffix", "Split", "Join", "TrimSpace", "ReplaceAll", "ToLower", "ToUpper",
    "strconv", "Atoi", "Itoa", "ParseInt", "ParseFloat", "FormatInt", "time", "Now", "Since",
    "Sleep", "Duration", "Time", "http", "Get", "Post", "HandleFunc", "ListenAndServe",
    "Request", "ResponseWriter", "Client", "Server", "json", "Marshal", "Unmarshal",
    "NewEncoder", "NewDecoder", "os", "Args", "Exit", "Getenv", "Setenv", "ReadFile", "WriteFile",
    "Open", "Create", "Remove", "MkdirAll", "Stat", "File", "io", "Reader", "Writer", "ReadAll",
    "Copy", "EOF", "log", "Print", "Fatal", "Fatalf", "Printf", "sort", "Slice", "Ints",
    "Strings", "math", "Sqrt", "Pow", "Abs", "Max", "Min", "rand", "Intn", "Seed", "main",
];

const SQL: &[&str] = &[
    "add", "all", "alter", "and", "any", "as", "asc", "backup", "between", "by", "case", "check",
    "column", "constraint", "create", "database", "default", "delete", "desc", "distinct", "drop",
    "exec", "exists", "foreign", "from", "full", "group", "having", "in", "index", "inner",
    "insert", "into", "is", "join", "key", "left", "like", "ilike", "limit", "not", "null", "on",
    "or", "order", "outer", "primary", "procedure", "right", "rownum", "select", "set", "table",
    "top", "truncate", "union", "unique", "update", "values", "view", "where", "with", "returning",
    "offset", "fetch", "next", "distinct", "count", "sum", "avg", "min", "max", "coalesce",
    "nullif", "cast", "convert", "round", "floor", "ceil", "abs", "length", "lower", "upper",
    "trim", "concat", "substring", "replace", "now", "current_date", "current_timestamp",
    "date_trunc", "extract", "to_char", "to_date", "generate_series", "row_number", "rank",
    "dense_rank", "over", "partition", "window", "varchar", "text", "char", "integer", "int",
    "bigint", "smallint", "numeric", "decimal", "real", "double", "boolean", "timestamp",
    "timestamptz", "date", "time", "interval", "uuid", "json", "jsonb", "serial", "bigserial",
    "bytea", "begin", "commit", "rollback", "savepoint", "transaction", "grant", "revoke",
    "trigger", "function", "returns", "language", "declare", "loop", "while", "if", "then",
    "else", "end", "cascade", "restrict", "references", "auto_increment", "identity", "schema",
    "explain", "analyze", "vacuum", "pragma", "sqlite_master",
];

const SHELL: &[&str] = &[
    "alias", "bg", "bind", "break", "builtin", "case", "cd", "command", "continue", "declare",
    "do", "done", "echo", "elif", "else", "esac", "eval", "exec", "exit", "export", "false",
    "fg", "fi", "for", "function", "getopts", "hash", "if", "in", "jobs", "kill", "let", "local",
    "printf", "pwd", "read", "readonly", "return", "select", "set", "shift", "source", "test",
    "then", "time", "times", "trap", "true", "type", "typeset", "ulimit", "umask", "unalias",
    "unset", "until", "wait", "while", "sudo", "git", "cargo", "rustc", "grep", "egrep", "sed",
    "awk", "find", "xargs", "curl", "wget", "chmod", "chown", "chgrp", "mkdir", "rmdir", "rm",
    "cp", "mv", "ls", "cat", "less", "more", "tail", "head", "touch", "make", "npm", "npx",
    "yarn", "pnpm", "node", "deno", "bun", "python", "python3", "pip", "pip3", "venv", "poetry",
    "docker", "podman", "docker-compose", "kubectl", "systemctl", "service", "journalctl", "ssh",
    "scp", "rsync", "tar", "zip", "unzip", "gzip", "gunzip", "env", "printenv", "which",
    "whereis", "whoami", "id", "groups", "uname", "hostname", "date", "cal", "df", "du", "free",
    "top", "htop", "ps", "killall", "pkill", "pgrep", "nohup", "screen", "tmux", "cron",
    "crontab", "apt", "apt-get", "dpkg", "snap", "flatpak", "pacman", "dnf", "yum", "brew",
    "sort", "uniq", "wc", "tr", "cut", "paste", "join", "split", "tee", "diff", "cmp", "comm",
    "stat", "file", "ln", "readlink", "realpath", "basename", "dirname", "md5sum", "sha256sum",
    "ping", "netstat", "ss", "ip", "ifconfig", "dig", "nslookup", "nc", "openssl", "gpg",
    "jq", "yq", "tree", "watch", "timeout", "yes", "seq", "expr", "bc", "man", "info", "help",
    "bash", "sh", "zsh", "fish", "source", "export", "PATH", "HOME", "PWD", "USER", "SHELL",
];

const JSON: &[&str] = &[
    "true", "false", "null", "object", "array", "string", "number", "boolean", "value", "values",
    "version", "name", "description", "dependencies", "devDependencies", "peerDependencies",
    "optionalDependencies", "scripts", "main", "module", "types", "exports", "files", "bin",
    "license", "author", "contributors", "repository", "bugs", "homepage", "keywords", "private",
    "package", "packageManager", "engines", "browserslist", "build", "target", "features",
    "workspace", "workspaces", "edition", "profile", "release", "debug", "test", "lint",
    "format", "start", "dev", "serve", "config", "settings", "schema", "type", "id", "title",
    "content", "items", "properties", "required", "default", "enum", "minimum", "maximum",
    "pattern", "format", "additionalProperties", "definitions", "$schema", "$ref", "$id",
    "allOf", "anyOf", "oneOf", "not", "if", "then", "else", "data", "error", "message", "code",
    "status", "success", "results", "meta", "pagination", "total", "offset", "limit", "count",
];

const MARKDOWN: &[&str] = &[
    "title", "description", "features", "installation", "installing", "usage", "quickstart",
    "configuration", "config", "license", "contributing", "screenshot",
    "screenshots", "example", "examples", "documentation", "docs", "requirements", "changelog",
    "roadmap", "support", "credits", "acknowledgements",    "contents", "intro",
    "introduction", "overview", "api", "reference", "guide", "tutorial", "faq", "todo",
    "notes", "summary", "resources", "links", "acknowledgments", "authors", "sponsors",
    "security", "testing", "deployment", "architecture", "structure", "benchmarks",
];

const TOML: &[&str] = &[
    "true", "false", "version", "name", "description", "authors", "license", "edition",
    "rust-version", "readme", "repository", "homepage", "documentation", "keywords", "categories",
    "dependencies", "dev-dependencies", "build-dependencies", "features", "workspace", "members",
    "exclude", "include", "package", "profile", "release", "debug", "default", "opt-level", "lto",
    "codegen-units", "strip", "panic", "inherits", "target", "bin", "lib", "test", "bench",
    "example", "path", "git", "branch", "tag", "rev", "optional", "workspace", "tool", "env",
];

const YAML: &[&str] = &[
    "true", "false", "null", "yes", "no", "on", "off", "name", "version", "description", "author",
    "license", "services", "image", "container_name", "ports", "volumes", "environment", "env",
    "depends_on", "restart", "build", "context", "dockerfile", "command", "entrypoint", "healthcheck",
    "networks", "steps", "jobs", "runs-on", "uses", "with", "run", "secrets", "needs", "strategy",
    "matrix", "branch", "branches", "tags", "on", "workflow_dispatch", "schedule", "permissions",
    "concurrency", "timeout-minutes", "continue-on-error", "if", "id", "outputs", "artifacts",
    "checkout", "setup", "install", "cache", "deploy", "stages", "stage", "script", "before_script",
    "after_script", "only", "except", "variables", "extends", "include", "apiVersion", "kind",
    "metadata", "spec", "labels", "annotations", "selector", "template", "containers", "replicas",
    "resources", "limits", "requests", "host", "hosts", "tasks", "handlers", "become", "vars",
];

const CSHARP: &[&str] = &[
    "abstract", "add", "alias", "and", "as", "ascending", "async", "await", "base", "bool",
    "break", "by", "byte", "case", "catch", "char", "checked", "class", "const", "continue",
    "decimal", "default", "delegate", "descending", "do", "double", "dynamic", "else", "enum",
    "equals", "event", "explicit", "extern", "false", "finally", "fixed", "float", "for",
    "foreach", "from", "get", "global", "goto", "group", "if", "implicit", "in", "init",
    "int", "interface", "internal", "into", "is", "join", "let", "lock", "long", "nameof",
    "namespace", "new", "not", "notnull", "null", "object", "on", "operator", "or", "orderby",
    "out", "override", "params", "partial", "private", "protected", "public", "readonly",
    "record", "ref", "remove", "required", "return", "sbyte", "sealed", "select", "set",
    "short", "sizeof", "stackalloc", "static", "string", "struct", "switch", "this", "throw",
    "true", "try", "typeof", "uint", "ulong", "unchecked", "unsafe", "ushort", "using", "value",
    "var", "virtual", "void", "volatile", "when", "where", "while", "with", "yield",
    "Console", "WriteLine", "ReadLine", "StringBuilder", "List", "Dictionary", "HashSet", "Queue",
    "Stack", "LinkedList", "IEnumerable", "IList", "IDictionary", "Task", "TaskCompletionSource",
    "CancellationToken", "Exception", "ArgumentException", "InvalidOperationException", "LINQ",
    "Select", "Where", "OrderBy", "FirstOrDefault", "Any", "All", "Count", "ToList", "ToArray",
    "Add", "Remove", "Contains", "TryGetValue", "TryParse", "Parse", "ToString", "Equals",
    "GetHashCode", "Dispose", "IDisposable", "Span", "Memory", "DateTime", "TimeSpan", "Guid",
    "Math", "Random", "Regex", "File", "Directory", "Path", "Stream", "StreamReader", "StreamWriter",
];

const KOTLIN: &[&str] = &[
    "abstract", "actual", "annotation", "as", "break", "by", "catch", "class", "companion",
    "const", "constructor", "continue", "crossinline", "data", "delegate", "do", "dynamic", "else",
    "enum", "expect", "external", "false", "field", "file", "final", "finally", "for", "fun",
    "get", "if", "import", "in", "infix", "init", "inline", "inner", "interface", "internal", "is",
    "lateinit", "noinline", "null", "object", "open", "operator", "out", "override", "package",
    "param", "private", "property", "protected", "public", "receiver", "reified", "return", "sealed",
    "set", "setparam", "super", "suspend", "tailrec", "this", "throw", "true", "try", "typealias",
    "typeof", "val", "value", "var", "vararg", "when", "where", "while", "it", "also", "apply",
    "let", "run", "with", "takeIf", "takeUnless", "runCatching", "require", "check", "lazy",
    "String", "Int", "Long", "Double", "Float", "Boolean", "Char", "Byte", "Short", "Any",
    "Unit", "Nothing", "Array", "List", "MutableList", "Map", "MutableMap", "Set", "MutableSet",
    "Sequence", "Pair", "Triple", "Result", "Throwable", "Exception", "RuntimeException",
    "println", "print", "readLine", "listOf", "mutableListOf", "mapOf", "mutableMapOf", "setOf",
    "emptyList", "emptyMap", "arrayOf", "intArrayOf", "repeat", "forEach", "map", "filter",
    "fold", "reduce", "sortedBy", "groupBy", "associateBy", "joinToString", "first", "last", "sum",
    "CoroutineScope", "launch", "async", "await", "Job", "Dispatchers", "Flow", "StateFlow",
];

const LUA: &[&str] = &[
    "and", "break", "do", "else", "elseif", "end", "false", "for", "function", "goto", "if",
    "in", "local", "nil", "not", "or", "repeat", "return", "then", "true", "until", "while",
    "self", "ipairs", "pairs", "next", "select", "type", "tostring", "tonumber", "print", "assert",
    "error", "pcall", "xpcall", "setmetatable", "getmetatable", "rawget", "rawset", "rawequal",
    "rawlen", "require", "collectgarbage", "dofile", "load", "loadfile", "table", "insert",
    "remove", "concat", "sort", "unpack", "pack", "string", "format", "sub", "len", "lower",
    "upper", "rep", "find", "match", "gmatch", "gsub", "byte", "char", "math", "abs", "ceil",
    "floor", "max", "min", "sqrt", "pow", "random", "randomseed", "huge", "pi", "io", "open",
    "read", "write", "close", "lines", "os", "time", "clock", "date", "getenv", "exit", "coroutine",
    "create", "resume", "yield", "status", "wrap", "running", "utf8", "charpattern", "codepoint",
    "garray", "len", "offset", "codepoint",
];

const SWIFT: &[&str] = &[
    "associatedtype", "as", "async", "await", "break", "case", "catch", "class", "continue",
    "default", "defer", "deinit", "do", "else", "enum", "extension", "fallthrough", "false",
    "fileprivate", "final", "for", "func", "get", "guard", "if", "import", "in", "indirect",
    "init", "inout", "internal", "is", "lazy", "let", "mutating", "nil", "nonmutating", "open",
    "operator", "optional", "override", "private", "protocol", "public", "repeat", "required",
    "rethrows", "return", "self", "Self", "set", "some", "static", "struct", "subscript", "super",
    "switch", "throw", "throws", "true", "try", "typealias", "var", "weak", "where", "while",
    "String", "Int", "Double", "Float", "Bool", "Character", "Array", "Dictionary", "Set",
    "Optional", "Result", "Error", "Codable", "Encodable", "Decodable", "Equatable", "Hashable",
    "Identifiable", "Comparable", "CustomStringConvertible", "AnyObject", "Any", "Void", "Never",
    "print", "debugPrint", "dump", "fatalError", "precondition", "assert", "map", "compactMap",
    "flatMap", "filter", "reduce", "forEach", "sorted", "contains", "first", "last", "append",
    "remove", "insert", "count", "isEmpty", "URL", "URLSession", "Data", "Date", "JSONDecoder",
    "JSONEncoder", "DispatchQueue", "Task", "MainActor", "Publisher", "ObservableObject", "Published",
];

const RUBY: &[&str] = &[
    "BEGIN", "END", "alias", "and", "begin", "break", "case", "class", "def", "defined", "do",
    "else", "elsif", "end", "ensure", "false", "for", "if", "in", "module", "next", "nil",
    "not", "or", "redo", "rescue", "retry", "return", "self", "super", "then", "true", "undef",
    "unless", "until", "when", "while", "yield", "require", "require_relative", "include", "extend",
    "prepend", "attr_accessor", "attr_reader", "attr_writer", "private", "protected", "public",
    "module_function", "raise", "rescue", "puts", "print", "p", "pp", "gets", "loop", "lambda",
    "proc", "block_given", "catch", "throw", "sleep", "exit", "abort", "at_exit", "freeze",
    "frozen", "dup", "clone", "tap", "then", "yield_self", "each", "map", "select", "reject",
    "reduce", "inject", "find", "detect", "sort_by", "group_by", "partition", "zip", "flatten",
    "compact", "uniq", "tally", "sum", "min", "max", "String", "Integer", "Float", "Array",
    "Hash", "Symbol", "Range", "Proc", "Struct", "OpenStruct", "Time", "File", "Dir", "IO",
    "JSON", "YAML", "Regexp", "Exception", "StandardError", "ArgumentError", "RuntimeError",
];

const DART: &[&str] = &[
    "abstract", "as", "assert", "async", "await", "base", "break", "case", "catch", "class",
    "const", "continue", "covariant", "default", "deferred", "do", "dynamic", "else", "enum",
    "export", "extends", "extension", "external", "factory", "false", "final", "finally", "for",
    "Function", "get", "hide", "if", "implements", "import", "in", "interface", "is", "late",
    "library", "mixin", "new", "null", "on", "operator", "part", "required", "rethrow", "return",
    "sealed", "set", "show", "static", "super", "switch", "sync", "this", "throw", "true", "try",
    "typedef", "var", "void", "when", "while", "with", "yield", "String", "int", "double", "num",
    "bool", "List", "Map", "Set", "Iterable", "Future", "Stream", "Object", "dynamic", "print",
    "debugPrint", "map", "where", "forEach", "reduce", "fold", "firstWhere", "any", "every",
    "contains", "length", "isEmpty", "add", "remove", "putIfAbsent", "jsonDecode", "jsonEncode",
    "Widget", "BuildContext", "StatelessWidget", "StatefulWidget", "State", "setState", "runApp",
];

const SCALA: &[&str] = &[
    "abstract", "case", "catch", "class", "def", "do", "else", "enum", "export", "extends",
    "false", "final", "finally", "for", "forSome", "given", "if", "implicit", "import", "lazy",
    "match", "new", "null", "object", "override", "package", "private", "protected", "return",
    "sealed", "super", "then", "this", "throw", "trait", "true", "try", "type", "using", "val",
    "var", "while", "with", "yield", "String", "Int", "Long", "Double", "Float", "Boolean",
    "Char", "Unit", "Any", "AnyRef", "Nothing", "Option", "Some", "None", "Either", "Left",
    "Right", "Try", "Success", "Failure", "Future", "List", "Seq", "Vector", "Map", "Set",
    "Array", "println", "print", "require", "assert", "map", "flatMap", "filter", "foldLeft",
    "foldRight", "foreach", "collect", "groupBy", "sortBy", "mkString", "head", "tail", "isEmpty",
];

const R_LANG: &[&str] = &[
    "break", "else", "FALSE", "for", "function", "if", "Inf", "NA", "NaN", "next", "NULL",
    "repeat", "return", "TRUE", "while", "in", "library", "require", "source", "print", "cat",
    "paste", "paste0", "sprintf", "message", "warning", "stop", "tryCatch", "c", "list", "data.frame",
    "matrix", "array", "factor", "vector", "seq", "seq_len", "rep", "length", "names", "head",
    "tail", "str", "summary", "class", "typeof", "is.na", "as.numeric", "as.character", "apply",
    "lapply", "sapply", "vapply", "mapply", "tapply", "Map", "Reduce", "Filter", "subset", "merge",
    "aggregate", "order", "sort", "unique", "which", "match", "%in%", "mean", "median", "sd", "var",
    "min", "max", "sum", "prod", "range", "quantile", "cor", "lm", "glm", "plot", "ggplot",
    "read.csv", "write.csv", "read.table", "library", "install.packages", "setwd", "getwd",
];

const PERL: &[&str] = &[
    "break", "continue", "do", "else", "elsif", "for", "foreach", "given", "goto", "if",
    "last", "local", "my", "next", "no", "our", "package", "redo", "require", "return",
    "state", "sub", "unless", "until", "use", "when", "while", "print", "printf", "say",
    "warn", "die", "croak", "eval", "scalar", "keys", "values", "each", "push", "pop", "shift",
    "unshift", "splice", "join", "split", "map", "grep", "sort", "reverse", "chomp", "chop",
    "lc", "uc", "lcfirst", "ucfirst", "length", "substr", "index", "rindex", "sprintf", "open",
    "close", "read", "write", "chdir", "mkdir", "unlink", "rename", "stat", "exists", "defined",
    "undef", "ref", "bless", "new", "shift", "wantarray", "STDIN", "STDOUT", "STDERR", "ENV",
];

const ELIXIR: &[&str] = &[
    "after", "alias", "and", "case", "catch", "cond", "def", "defmodule", "defp", "defstruct",
    "defprotocol", "defimpl", "defdelegate", "defguard", "defmacro", "defmacrop", "do", "else",
    "end", "false", "fn", "for", "if", "import", "in", "nil", "not", "or", "quote", "raise",
    "receive", "require", "rescue", "super", "then", "true", "try", "unless", "unquote", "use",
    "when", "with", "Enum", "map", "filter", "reduce", "each", "find", "sort", "group_by",
    "Map", "get", "put", "update", "keys", "values", "merge", "List", "first", "last", "flatten",
    "String", "split", "trim", "upcase", "downcase", "replace", "contains", "length", "IO", "puts",
    "inspect", "gets", "File", "read", "write", "stream", "Path", "join", "Agent", "GenServer",
    "Task", "async", "await", "Supervisor", "start_link", "Application", "Logger", "info", "error",
];

const HASKELL: &[&str] = &[
    "as", "case", "class", "data", "default", "deriving", "do", "else", "foreign", "hiding",
    "if", "import", "in", "infix", "infixl", "infixr", "instance", "let", "module", "newtype",
    "of", "qualified", "then", "type", "where", "Int", "Integer", "Float", "Double", "Char",
    "String", "Bool", "Maybe", "Just", "Nothing", "Either", "Left", "Right", "Ordering", "LT",
    "EQ", "GT", "IO", "pure", "return", "fmap", "mapM", "mapM_", "forM", "forM_", "sequence",
    "sequence_", "traverse", "foldr", "foldl", "foldMap", "concat", "concatMap", "filter", "head",
    "tail", "init", "last", "length", "null", "reverse", "take", "drop", "elem", "notElem", "zip",
    "zipWith", "show", "read", "putStr", "putStrLn", "print", "getLine", "readFile", "writeFile",
];

const ZIG: &[&str] = &[
    "align", "allowzero", "and", "anyframe", "anytype", "asm", "async", "await", "break", "callconv",
    "catch", "comptime", "const", "continue", "defer", "else", "enum", "errdefer", "error", "export",
    "extern", "false", "fn", "for", "if", "inline", "noalias", "noinline", "nosuspend", "null",
    "opaque", "or", "orelse", "packed", "pub", "resume", "return", "struct", "suspend", "switch",
    "test", "threadlocal", "true", "try", "union", "unreachable", "usingnamespace", "var", "volatile",
    "while", "std", "debug", "print", "allocator", "ArrayList", "StringHashMap", "AutoHashMap",
    "HashMap", "Array", "Slice", "Optional", "Error", "anyerror", "@import", "@intCast", "@as",
    "@sizeOf", "@alignOf", "@typeInfo", "@ptrCast", "@memcpy", "@memset",
];

const NIM: &[&str] = &[
    "and", "as", "asm", "bind", "block", "break", "case", "cast", "concept", "const", "continue",
    "converter", "defer", "discard", "distinct", "div", "do", "elif", "else", "end", "enum",
    "except", "export", "finally", "for", "from", "func", "if", "import", "in", "include", "is",
    "isnot", "iterator", "let", "macro", "method", "mixin", "mod", "nil", "not", "notin", "object",
    "of", "or", "out", "proc", "ptr", "raise", "ref", "return", "shl", "shr", "static", "template",
    "try", "tuple", "type", "using", "var", "when", "while", "with", "xor", "yield", "echo", "len",
    "add", "del", "pop", "newSeq", "high", "low", "ord", "chr", "int", "float", "string", "seq",
    "array", "Table", "HashSet", "Option", "some", "none", "isSome", "isNone", "get", "result",
];

const JULIA: &[&str] = &[
    "abstract", "baremodule", "begin", "break", "catch", "const", "continue", "do", "else",
    "elseif", "end", "export", "false", "finally", "for", "function", "global", "if", "import",
    "in", "let", "local", "macro", "module", "quote", "return", "struct", "true", "try", "using",
    "while", "where", "mutable", "primitive", "type", "print", "println", "show", "display",
    "push", "pop", "append", "length", "size", "axes", "eachindex", "enumerate", "zip", "map",
    "filter", "reduce", "foldl", "foldr", "sort", "sortperm", "unique", "findall", "findfirst",
    "sum", "prod", "maximum", "minimum", "extrema", "mean", "median", "std", "var", "range",
    "collect", "Array", "Vector", "Matrix", "Dict", "Set", "Tuple", "NamedTuple", "String",
    "Int", "Float64", "Bool", "Nothing", "Missing", "Union", "DataType", "abstract type",
];

const POWERSHELL: &[&str] = &[
    "begin", "break", "catch", "class", "continue", "data", "do", "dynamicparam", "else",
    "elseif", "end", "enum", "exit", "filter", "finally", "for", "foreach", "from", "function",
    "if", "in", "param", "process", "return", "switch", "throw", "trap", "try", "until", "using",
    "while", "workflow", "Write-Output", "Write-Host", "Write-Error", "Write-Warning", "Write-Verbose",
    "Get-Item", "Get-ChildItem", "Set-Item", "Remove-Item", "Copy-Item", "Move-Item", "New-Item",
    "Test-Path", "Get-Content", "Set-Content", "Add-Content", "Select-Object", "Where-Object",
    "ForEach-Object", "Sort-Object", "Group-Object", "Measure-Object", "ConvertTo-Json",
    "ConvertFrom-Json", "Invoke-WebRequest", "Invoke-RestMethod", "Start-Process", "Get-Process",
    "Stop-Process", "Get-Service", "Start-Service", "Stop-Service", "Get-Command", "Get-Help",
    "Get-Location", "Set-Location", "Get-Date", "Join-Path", "Split-Path", "Resolve-Path",
];

const ASSEMBLY: &[&str] = &[
    "section", "global", "extern", "bits", "default", "align", "db", "dw", "dd", "dq", "equ",
    "times", "byte", "word", "dword", "qword", "resb", "resw", "resd", "resq", "org", "mov",
    "lea", "push", "pop", "add", "sub", "mul", "imul", "div", "idiv", "inc", "dec", "and",
    "or", "xor", "not", "neg", "shl", "shr", "sal", "sar", "rol", "ror", "cmp", "test",
    "jmp", "je", "jne", "jz", "jnz", "jg", "jge", "jl", "jle", "ja", "jb", "call", "ret",
    "leave", "nop", "int", "syscall", "loop", "rep", "movs", "stos", "lods", "scas", "cmps",
    "eax", "ebx", "ecx", "edx", "esi", "edi", "esp", "ebp", "rax", "rbx", "rcx", "rdx",
    "rsi", "rdi", "rsp", "rbp", "r8", "r9", "r10", "r11", "r12", "r13", "r14", "r15",
];

const SOLIDITY: &[&str] = &[
    "abstract", "address", "anonymous", "as", "assembly", "break", "calldata", "catch", "constant",
    "constructor", "continue", "contract", "delete", "do", "else", "emit", "enum", "error", "event",
    "external", "fallback", "false", "for", "function", "if", "immutable", "import", "indexed",
    "interface", "internal", "is", "library", "mapping", "memory", "modifier", "new", "override",
    "payable", "pragma", "private", "public", "pure", "receive", "return", "returns", "revert",
    "storage", "struct", "true", "try", "type", "unchecked", "using", "view", "virtual", "while",
    "uint", "uint256", "int", "int256", "bytes", "bytes32", "string", "bool", "mapping", "msg",
    "sender", "value", "data", "block", "timestamp", "number", "tx", "origin", "require", "assert",
    "keccak256", "abi", "encode", "encodePacked", "decode", "payable", "selfdestruct", "transfer",
    "call", "delegatecall", "staticcall", "balance", "this", "super", "IERC20", "IERC721", "ERC20",
];

const OBJECTIVE_C: &[&str] = &[
    "@autoreleasepool", "@catch", "@class", "@dynamic", "@encode", "@end", "@finally",
    "@implementation", "@interface", "@optional", "@property", "@protocol", "@required",
    "@selector", "@synchronized", "@synthesize", "@throw", "@try", "auto", "break", "case",
    "char", "const", "continue", "default", "do", "double", "else", "enum", "extern", "float",
    "for", "goto", "if", "inline", "int", "long", "nil", "NO", "register", "return", "self",
    "short", "signed", "sizeof", "static", "struct", "super", "switch", "typedef", "union",
    "unsigned", "void", "volatile", "while", "YES", "NSString", "NSArray", "NSDictionary",
    "NSSet", "NSNumber", "NSObject", "NSData", "NSError", "NSURL", "NSDate", "NSLog", "alloc",
    "init", "new", "copy", "mutableCopy", "retain", "release", "autorelease", "description",
    "count", "objectForKey", "setObject", "removeObject", "addObject", "length", "UTF8String",
];

const GROOVY: &[&str] = &[
    "abstract", "as", "assert", "break", "case", "catch", "class", "const", "continue", "def",
    "default", "do", "else", "enum", "extends", "false", "final", "finally", "for", "goto",
    "if", "implements", "import", "in", "instanceof", "interface", "native", "new", "null",
    "package", "private", "protected", "public", "return", "static", "strictfp", "super",
    "switch", "synchronized", "this", "throw", "throws", "trait", "transient", "true", "try",
    "var", "volatile", "while", "String", "Integer", "Long", "Double", "Float", "Boolean",
    "List", "Map", "Set", "Closure", "each", "collect", "findAll", "inject", "println",
    "print", "readLine", "File", "newReader", "text", "with", "withWriter", "json", "JsonSlurper",
    "JsonOutput", "Stage", "node", "sh", "bat", "pipeline", "steps", "echo",
];

/// Whole commands offered on an empty line: compile, run, test and the most
/// common package-manager invocations for every bundled language.
const COMMANDS: &[&str] = &[
    "cargo build --release",
    "cargo run",
    "cargo test",
    "cargo clippy",
    "cargo fmt",
    "rustc main.rs -O -o main",
    "gcc main.c -O2 -Wall -o main",
    "g++ main.cpp -O2 -Wall -o main",
    "clang main.c -O2 -Wall -o main",
    "make",
    "cmake -B build -DCMAKE_BUILD_TYPE=Release",
    "cmake --build build -j",
    "dotnet new console",
    "dotnet build -c Release",
    "dotnet run",
    "dotnet test",
    "dotnet publish -c Release -r linux-x64 --self-contained",
    "csc Program.cs",
    "mono Program.exe",
    "kotlinc Main.kt -include-runtime -d Main.jar",
    "java -jar Main.jar",
    "kotlinc-native Main.kt -o main",
    "./gradlew build",
    "./gradlew run",
    "gradle build",
    "gradle run",
    "lua main.lua",
    "luac -o main.luac main.lua",
    "luarocks install",
    "swift build -c release",
    "swift run",
    "swift test",
    "ruby main.rb",
    "bundle install",
    "bundle exec rspec",
    "rails server",
    "gem install",
    "dart run",
    "dart compile exe bin/main.dart -o main",
    "dart test",
    "dart pub get",
    "flutter run",
    "flutter build linux",
    "sbt compile",
    "sbt run",
    "sbt test",
    "scala-cli run Main.scala",
    "scalac Main.scala",
    "Rscript main.R",
    "R CMD build .",
    "perl main.pl",
    "cpanm --installdeps .",
    "elixir main.exs",
    "mix deps.get",
    "mix compile",
    "mix run",
    "mix test",
    "ghc Main.hs -O2 -o main",
    "runghc Main.hs",
    "cabal build",
    "cabal run",
    "stack build",
    "zig build-exe main.zig -O ReleaseFast",
    "zig build",
    "zig test main.zig",
    "nim c -d:release main.nim",
    "nim r main.nim",
    "nimble build",
    "julia main.jl",
    "julia --project=. -e 'using Pkg; Pkg.instantiate()'",
    "pwsh script.ps1",
    "nasm -f elf64 main.asm -o main.o",
    "ld main.o -o main",
    "as main.s -o main.o",
    "forge build",
    "forge test",
    "npx hardhat compile",
    "npx hardhat test",
    "clang -framework Foundation main.m -o main",
    "groovy main.groovy",
    "groovyc Main.groovy",
    "python3 main.py",
    "python3 -m venv .venv",
    "pip install -r requirements.txt",
    "pytest",
    "python3 -m http.server 8000",
    "node index.js",
    "npm install",
    "npm run dev",
    "npm run build",
    "npm test",
    "npx tsc --noEmit",
    "npx vite",
    "deno run -A main.ts",
    "bun run index.ts",
    "go build -o app .",
    "go run .",
    "go test ./...",
    "go mod tidy",
    "javac Main.java",
    "java Main",
    "mvn package",
    "mvn test",
    "php -S localhost:8000",
    "composer install",
    "php artisan serve",
    "sqlite3 database.db",
    "psql -f schema.sql",
    "mysql -u root -p",
    "sudo apt update",
    "sudo apt install",
    "tar -czf archive.tar.gz folder",
    "chmod +x script.sh",
    "./script.sh",
    "git status",
    "git add .",
    "git commit -m \"message\"",
    "git push",
    "docker build -t app .",
    "docker run --rm -it app",
    "docker compose up",
];

fn lexicon_slices(language: Language) -> &'static [&'static [&'static str]] {
    match language {
        Language::Html => &[HTML_TAGS, HTML_ATTRS, CSS, COMMON],
        Language::Css => &[CSS, HTML_TAGS, COMMON],
        Language::JavaScript => &[JAVASCRIPT, TYPESCRIPT, HTML_ATTRS, COMMON],
        Language::TypeScript => &[JAVASCRIPT, TYPESCRIPT, COMMON],
        Language::Python => &[PYTHON, COMMON],
        Language::Php => &[PHP, COMMON],
        Language::Java => &[JAVA, COMMON],
        Language::Rust => &[RUST, COMMON],
        Language::C => &[C, COMMON],
        Language::Cpp => &[C, CPP, COMMON],
        Language::Go => &[GO, COMMON],
        Language::Sql => &[SQL, COMMON],
        Language::Shell => &[SHELL, COMMON],
        Language::Json => &[JSON, COMMON],
        Language::Markdown => &[MARKDOWN, COMMON],
        Language::Toml => &[TOML, COMMON],
        Language::Yaml => &[YAML, COMMON],
        Language::CSharp => &[CSHARP, COMMON],
        Language::Kotlin => &[KOTLIN, COMMON],
        Language::Lua => &[LUA, COMMON],
        Language::Swift => &[SWIFT, COMMON],
        Language::Ruby => &[RUBY, COMMON],
        Language::Dart => &[DART, COMMON],
        Language::Scala => &[SCALA, COMMON],
        Language::R => &[R_LANG, COMMON],
        Language::Perl => &[PERL, COMMON],
        Language::Elixir => &[ELIXIR, COMMON],
        Language::Haskell => &[HASKELL, COMMON],
        Language::Zig => &[ZIG, COMMON],
        Language::Nim => &[NIM, COMMON],
        Language::Julia => &[JULIA, COMMON],
        Language::PowerShell => &[POWERSHELL, SHELL, COMMON],
        Language::Assembly => &[ASSEMBLY, C, COMMON],
        Language::Solidity => &[SOLIDITY, COMMON],
        Language::ObjectiveC => &[OBJECTIVE_C, C, COMMON],
        Language::Groovy => &[GROOVY, COMMON],
        Language::Plain => &[],
    }
}

/// Every built-in language word, used to seed the spell checker.
pub fn all_language_words() -> impl Iterator<Item = &'static str> {
    [
        COMMON, HTML_TAGS, HTML_ATTRS, CSS, JAVASCRIPT, TYPESCRIPT, PYTHON, PHP, JAVA, RUST, C,
        CPP, GO, SQL, SHELL, JSON, MARKDOWN, TOML, YAML, CSHARP, KOTLIN, LUA, SWIFT, RUBY, DART,
        SCALA, R_LANG, PERL, ELIXIR, HASKELL, ZIG, NIM, JULIA, POWERSHELL, ASSEMBLY, SOLIDITY,
        OBJECTIVE_C, GROOVY,
    ]
    .into_iter()
    .flat_map(|group| group.iter().copied())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_word_at_cursor() {
        let text = "let result = café";
        let cursor = text.chars().count();
        let word = word_at_cursor(text, cursor).unwrap();
        assert_eq!(word.prefix, "café");
        assert_eq!(word.start, text.chars().count() - 4);
    }

    #[test]
    fn ignores_dash_prefix_but_keeps_offset() {
        let text = "  -backg";
        let word = word_at_cursor(text, text.chars().count()).unwrap();
        assert_eq!(word.prefix, "backg");
        assert_eq!(word.start, 3);
    }

    #[test]
    fn offers_html_tags_and_document_words() {
        let text = "<div class=\"card\"></div>";
        let items = completions(Language::Html, "ca", text, text.chars().count(), 8);
        let texts: Vec<_> = items.iter().map(|item| item.text.as_str()).collect();
        assert!(texts.contains(&"card"));
        assert!(texts.contains(&"canvas"));
    }

    #[test]
    fn offers_python_builtins() {
        let items = completions(Language::Python, "pri", "", 0, 8);
        let texts: Vec<_> = items.iter().map(|item| item.text.as_str()).collect();
        assert!(texts.contains(&"print"));
    }

    #[test]
    fn does_not_return_the_typed_word() {
        let items = completions(Language::JavaScript, "const", "const value", 5, 8);
        assert!(items.iter().all(|item| item.text != "const"));
    }

    #[test]
    fn prefix_matches_rank_before_mid_word_matches() {
        let items = completions(Language::Rust, "vec", "", 0, 12);
        assert_eq!(items.first().map(|item| item.text.as_str()), Some("Vec"));
        let items = completions(Language::Rust, "pus", "", 0, 12);
        assert!(items.iter().any(|item| item.text == "push"));
    }

    #[test]
    fn finds_mid_word_matches() {
        let items = completions(Language::JavaScript, "query", "", 0, 12);
        assert!(items.iter().any(|item| item.text == "querySelector"));
    }

    #[test]
    fn lexicons_cover_several_languages() {
        for language in [
            Language::Html,
            Language::Css,
            Language::JavaScript,
            Language::TypeScript,
            Language::Python,
            Language::Php,
            Language::Java,
            Language::Rust,
            Language::C,
            Language::Cpp,
            Language::Go,
            Language::Sql,
            Language::Shell,
            Language::Json,
            Language::Markdown,
            Language::Toml,
            Language::Yaml,
            Language::CSharp,
            Language::Kotlin,
            Language::Lua,
            Language::Swift,
            Language::Ruby,
            Language::Dart,
            Language::Scala,
            Language::R,
            Language::Perl,
            Language::Elixir,
            Language::Haskell,
            Language::Zig,
            Language::Nim,
            Language::Julia,
            Language::PowerShell,
            Language::Assembly,
            Language::Solidity,
            Language::ObjectiveC,
            Language::Groovy,
        ] {
            assert!(
                !completions(language, "s", "", 0, 5).is_empty(),
                "no suggestions for {language:?}"
            );
        }
    }

    #[test]
    fn offers_compile_commands_on_a_fresh_line() {
        let text = "car";
        let items = completions(Language::Rust, "car", text, text.chars().count(), 12);
        let texts: Vec<_> = items.iter().map(|item| item.text.as_str()).collect();
        assert!(texts.iter().any(|text| text.starts_with("cargo build")));
        assert!(texts.iter().all(|text| *text != "car"));
    }

    #[test]
    fn no_commands_mid_sentence() {
        let text = "let car";
        let items = completions(Language::Rust, "car", text, text.chars().count(), 12);
        assert!(items.iter().all(|item| item.detail != "command"));
    }
}

