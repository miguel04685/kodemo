//! Phosphor icon font glyphs.
//!
//! The icon set ships as `assets/fonts/Phosphor.ttf` (MIT, see
//! `Phosphor-LICENSE.txt`) and is registered as the `phosphor` font family in
//! `theme::install_fonts`. Keeping the icons in an external font means the set
//! can be updated by dropping in a new file, without touching drawing code.

use eframe::egui::{Color32, FontFamily, FontId, Painter, Pos2, Rect, Stroke, Vec2};

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Files,
    Search,
    Git,
    Extensions,
    Sparkle,
    Settings,
    Account,
    Preview,
    Close,
    Plus,
    Folder,
    FolderOpen,
    File,
    ChevronRight,
    ChevronDown,
    Refresh,
    Collapse,
    Trash,
    Edit,
    More,
    Save,
    Play,
    Stop,
    Sidebar,
    Panel,
    Error,
    Warning,
    Info,
    Check,
    Branch,
    Sync,
    Bell,
    Globe,
    External,
    Terminal,
    Command,
    ArrowLeft,
    Code,
    WordWrap,
    FileHtml,
    FileCss,
    FileJs,
    FileTs,
    FileJsx,
    FileTsx,
    FileImage,
    FileAudio,
    FileVideo,
    FilePdf,
    FileZip,
    FileDoc,
    FileXls,
    FilePpt,
    FileMd,
    FileText,
    FileCode,
    FilePy,
    FileSql,
    FileVue,
    FileIni,
    FileC,
    FileCpp,
    FileRs,
    FileCSharp,
    FileCsv,
    FilePng,
    FileJpg,
    FileSvg,
    FileTxt,
    FileLock,
    FileArchive,
    BracketsCurly,
    BracketsAngle,
    Cpu,
    Memory,
}

impl Icon {
    /// Unicode private-use codepoint used by Phosphor 2.x.
    fn glyph(self) -> char {
        let code = match self {
            Icon::Files => 0xe710,
            Icon::Search => 0xe30c,
            Icon::Git | Icon::Branch => 0xe278,
            Icon::Extensions => 0xe464,
            Icon::Sparkle => 0xe6a2,
            Icon::Settings => 0xe272,
            Icon::Account => 0xe4c4,
            Icon::Preview | Icon::Globe => 0xe0f4,
            Icon::Close => 0xe4f6,
            Icon::Plus => 0xe3d4,
            Icon::Folder => 0xe24a,
            Icon::FolderOpen => 0xe256,
            Icon::File => 0xe230,
            Icon::ChevronRight => 0xe13a,
            Icon::ChevronDown => 0xe136,
            Icon::Refresh | Icon::Sync => 0xe094,
            Icon::Collapse => 0xe2f0,
            Icon::Trash => 0xe4a6,
            Icon::Edit => 0xe3b4,
            Icon::More => 0xe1fe,
            Icon::Save => 0xe248,
            Icon::Play => 0xe3d0,
            Icon::Stop => 0xe46c,
            Icon::Sidebar => 0xeab6,
            Icon::Panel => 0xe870,
            Icon::Error => 0xe4f8,
            Icon::Warning => 0xe4e0,
            Icon::Info => 0xe2ce,
            Icon::Check => 0xe182,
            Icon::Bell => 0xe0ce,
            Icon::External => 0xe5de,
            Icon::Terminal => 0xeae8,
            Icon::Command => 0xe1c4,
            Icon::ArrowLeft => 0xe058,
            Icon::Code => 0xe1bc,
            Icon::WordWrap => 0xe484,
            Icon::FileHtml => 0xeb38,
            Icon::FileCss => 0xeb34,
            Icon::FileJs => 0xeb24,
            Icon::FileTs => 0xeb26,
            Icon::FileJsx => 0xeb3a,
            Icon::FileTsx => 0xeb3c,
            Icon::FileImage => 0xea24,
            // The speaker icon reads more clearly than a generic audio file.
            Icon::FileAudio => 0xe44a,
            Icon::FileVideo => 0xea22,
            Icon::FilePdf => 0xe702,
            Icon::FileZip => 0xe958,
            Icon::FileDoc => 0xeb1e,
            Icon::FileXls => 0xeb22,
            Icon::FilePpt => 0xeb20,
            Icon::FileMd => 0xed50,
            Icon::FileText => 0xe23a,
            Icon::FileCode => 0xe914,
            Icon::FilePy => 0xeb2c,
            Icon::FileSql => 0xed4e,
            Icon::FileVue => 0xeb3e,
            Icon::FileIni => 0xeb33,
            Icon::FileC => 0xeb32,
            Icon::FileCpp => 0xeb2e,
            Icon::FileRs => 0xeb28,
            Icon::FileCSharp => 0xeb30,
            Icon::FileCsv => 0xeb1c,
            Icon::FilePng => 0xeb18,
            Icon::FileJpg => 0xeb1a,
            Icon::FileSvg => 0xed08,
            Icon::FileTxt => 0xeb35,
            Icon::FileLock => 0xe95c,
            Icon::FileArchive => 0xeb2a,
            Icon::BracketsCurly => 0xe860,
            Icon::BracketsAngle => 0xe862,
            Icon::Cpu => 0xe610,
            Icon::Memory => 0xe9c4,
        };
        char::from_u32(code).unwrap_or('\u{e4f6}')
    }
}

/// Pick an icon that says what a file actually is, so a `.png` never looks
/// like a `.rs` in the explorer or the file picker.
pub fn for_path(path: &std::path::Path) -> Icon {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if name == ".env" || name.starts_with(".env.") || name.ends_with(".lock") {
        return Icon::FileLock;
    }
    if name == ".gitignore" || name == ".gitattributes" || name == ".dockerignore" {
        return Icon::FileIni;
    }
    if name == "dockerfile" || name == "makefile" || name == "cmakelists.txt" {
        return Icon::Terminal;
    }
    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match ext.as_str() {
        "html" | "htm" | "xhtml" => Icon::FileHtml,
        "css" | "scss" | "sass" | "less" => Icon::FileCss,
        "js" | "mjs" | "cjs" => Icon::FileJs,
        "jsx" => Icon::FileJsx,
        "ts" => Icon::FileTs,
        "tsx" => Icon::FileTsx,
        "png" => Icon::FilePng,
        "jpg" | "jpeg" => Icon::FileJpg,
        "svg" => Icon::FileSvg,
        "gif" | "webp" | "bmp" | "ico" | "avif" | "tif" | "tiff" => Icon::FileImage,
        "wav" | "mp3" | "flac" | "ogg" | "m4a" | "aac" | "opus" | "wma" => Icon::FileAudio,
        "mp4" | "mov" | "webm" | "mkv" | "avi" | "m4v" => Icon::FileVideo,
        "pdf" => Icon::FilePdf,
        "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" | "zst" => Icon::FileArchive,
        "doc" | "docx" | "odt" | "rtf" => Icon::FileDoc,
        "xls" | "xlsx" | "ods" => Icon::FileXls,
        "csv" | "tsv" => Icon::FileCsv,
        "ppt" | "pptx" | "odp" => Icon::FilePpt,
        "md" | "markdown" => Icon::FileMd,
        "txt" => Icon::FileTxt,
        "log" => Icon::FileText,
        "py" | "pyw" => Icon::FilePy,
        "rs" => Icon::FileRs,
        "cs" | "csx" => Icon::FileCSharp,
        "go"
        | "java"
        | "rb"
        | "php"
        | "lua"
        | "kt"
        | "kts"
        | "swift"
        | "dart"
        | "scala"
        | "sc"
        | "pl"
        | "pm"
        | "ex"
        | "exs"
        | "hs"
        | "zig"
        | "nim"
        | "jl"
        | "sol"
        | "groovy"
        | "r" => Icon::FileCode,
        "json" | "jsonc" | "json5" | "webmanifest" => Icon::BracketsCurly,
        "xml" | "xsl" | "xslt" | "xsd" | "plist" => Icon::BracketsAngle,
        "sql" => Icon::FileSql,
        "vue" => Icon::FileVue,
        "ini" | "conf" | "cfg" | "toml" | "yaml" | "yml" => Icon::FileIni,
        "c" | "h" => Icon::FileC,
        "cpp" | "cc" | "cxx" | "hpp" | "hh" => Icon::FileCpp,
        "sh" | "bash" | "zsh" | "fish" | "bat" | "cmd" | "ps1" | "psm1" | "psd1" => {
            Icon::Terminal
        }
        "asm" | "s" => Icon::FileCode,
        "lock" => Icon::FileLock,
        _ => Icon::File,
    }
}

pub fn draw(painter: &Painter, rect: Rect, icon: Icon, color: Color32, _width: f32) {
    let size = rect.width().min(rect.height()) * 1.12;
    painter.text(
        rect.center(),
        eframe::egui::Align2::CENTER_CENTER,
        icon.glyph(),
        FontId::new(size, FontFamily::Name("phosphor".into())),
        color,
    );
}

/// The brand mark stays a vector `</>` so the logo keeps its exact shape.
pub fn draw_logo_mark(painter: &Painter, rect: Rect, accent: Color32) {
    let s = rect.width().min(rect.height());
    let o = rect.center() - Vec2::splat(s * 0.5);
    let p = |x: f32, y: f32| o + Vec2::new(x * s, y * s);
    let stroke = Stroke::new((s * 0.12).max(1.6), accent);
    painter.line_segment([p(0.38, 0.22), p(0.13, 0.5)], stroke);
    painter.line_segment([p(0.13, 0.5), p(0.38, 0.78)], stroke);
    painter.line_segment([p(0.62, 0.22), p(0.87, 0.5)], stroke);
    painter.line_segment([p(0.87, 0.5), p(0.62, 0.78)], stroke);
    painter.line_segment([p(0.59, 0.12), p(0.41, 0.88)], stroke);
}

#[allow(dead_code)]
fn midpoint(a: Pos2, b: Pos2) -> Pos2 {
    Pos2::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn file_icons_follow_the_extension() {
        assert_eq!(for_path(Path::new("index.html")), Icon::FileHtml);
        assert_eq!(for_path(Path::new("styles.css")), Icon::FileCss);
        assert_eq!(for_path(Path::new("app.js")), Icon::FileJs);
        assert_eq!(for_path(Path::new("snippets.py")), Icon::FilePy);
        assert_eq!(for_path(Path::new("photo.png")), Icon::FilePng);
        assert_eq!(for_path(Path::new("photo.gif")), Icon::FileImage);
        assert_eq!(for_path(Path::new("sound.wav")), Icon::FileAudio);
        assert_eq!(for_path(Path::new("README.md")), Icon::FileMd);
        assert_eq!(for_path(Path::new("lib.rs")), Icon::FileRs);
        assert_eq!(for_path(Path::new("data.json")), Icon::BracketsCurly);
        assert_eq!(for_path(Path::new("feed.xml")), Icon::BracketsAngle);
        assert_eq!(for_path(Path::new("archive.tar")), Icon::FileArchive);
        assert_eq!(for_path(Path::new(".env")), Icon::FileLock);
    }
}
