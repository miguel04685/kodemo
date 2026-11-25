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
    _cursor: usize,
    limit: usize,
) -> Vec<CompletionItem> {
    if prefix.is_empty() || limit == 0 {
        return Vec::new();
    }
    let lower = prefix.to_lowercase();
    let case_sensitive = prefix.chars().any(|ch| ch.is_uppercase());
    let mut seen = HashSet::new();
    // (keyword, match position, length, item)
    let mut items: Vec<(bool, usize, usize, CompletionItem)> = Vec::new();

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
            false,
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
                true,
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

    // Prefix matches come before mid-word matches, then shorter words, then
    // language words, then alphabetically earlier words. This keeps the first
    // row stable while identifiers already used in the document surface early.
    items.sort_by(|a, b| {
        a.1.cmp(&b.1)
            .then_with(|| a.2.cmp(&b.2))
            .then_with(|| b.0.cmp(&a.0))
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
        Language::Plain => &[],
    }
}

/// Every built-in language word, used to seed the spell checker.
pub fn all_language_words() -> impl Iterator<Item = &'static str> {
    [
        COMMON, HTML_TAGS, HTML_ATTRS, CSS, JAVASCRIPT, TYPESCRIPT, PYTHON, PHP, JAVA, RUST, C,
        CPP, GO, SQL, SHELL, JSON, MARKDOWN, TOML, YAML,
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
        let items = completions(Language::Html, "ca", text, 0, 8);
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
        ] {
            assert!(
                !completions(language, "s", "", 0, 5).is_empty(),
                "no suggestions for {language:?}"
            );
        }
    }
}

