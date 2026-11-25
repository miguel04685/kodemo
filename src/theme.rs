use crate::settings::Settings;
use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Stroke,
    TextStyle, Vec2,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub const THEME_NAMES: &[&str] = &[
    "Kodemo Dark",
    "Kodemo Light",
    "Dark+",
    "Light+",
    "Gruvbox Dark",
    "Gruvbox Light",
    "Monokai",
    "Solarized Dark",
    "Solarized Light",
    "Catppuccin",
    "Dracula",
    "Tokyo Night",
    "Kodemo Ocean",
    "AMOLED Black",
    "GTK Native",
    "Qt Native",
    "Custom",
];

pub fn theme_names() -> &'static [&'static str] {
    THEME_NAMES
}

fn c(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// All colors used by Kodemo. Kept flat and serializable so user themes
/// remain simple JSON that can be copied between machines.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ThemePalette {
    pub dark: bool,
    pub bg: Color32,
    pub panel: Color32,
    pub panel_alt: Color32,
    pub editor: Color32,
    pub titlebar: Color32,
    pub statusbar: Color32,
    pub activity: Color32,
    pub activity_fg: Color32,
    pub border: Color32,
    pub border_strong: Color32,
    pub fg: Color32,
    pub fg_muted: Color32,
    pub fg_faint: Color32,
    pub accent: Color32,
    pub accent_fg: Color32,
    pub hover: Color32,
    pub selection: Color32,
    pub active_line: Color32,
    pub gutter: Color32,
    pub input_bg: Color32,
    pub tab_active: Color32,
    pub tab_inactive: Color32,
    pub tab_fg: Color32,
    pub tab_fg_inactive: Color32,
    pub scrollbar: Color32,
    pub scrollbar_hover: Color32,
    pub error: Color32,
    pub warning: Color32,
    pub success: Color32,
    pub info: Color32,
    pub syntax_keyword: Color32,
    pub syntax_string: Color32,
    pub syntax_comment: Color32,
    pub syntax_number: Color32,
    pub syntax_function: Color32,
    pub syntax_type: Color32,
    pub syntax_variable: Color32,
    pub syntax_macro: Color32,
    pub syntax_operator: Color32,
    pub syntax_tag: Color32,
    pub syntax_attribute: Color32,
    pub syntax_property: Color32,
    pub syntax_constant: Color32,
}

impl Default for ThemePalette {
    fn default() -> Self {
        kodemo_dark()
    }
}

pub fn resolve(settings: &Settings) -> ThemePalette {
    if settings.theme == "Custom" {
        if let Some(custom) = &settings.custom_theme {
            return custom.clone();
        }
    }
    builtin(&settings.theme).unwrap_or_else(kodemo_dark)
}

pub fn builtin(name: &str) -> Option<ThemePalette> {
    Some(match name {
        "Kodemo Dark" | "Dark+" => kodemo_dark(),
        "Kodemo Light" | "Light+" => kodemo_light(),
        "Gruvbox Dark" => gruvbox_dark(),
        "Gruvbox Light" => gruvbox_light(),
        "Monokai" => monokai(),
        "Solarized Dark" => solarized_dark(),
        "Solarized Light" => solarized_light(),
        "Catppuccin" => catppuccin(),
        "Dracula" => dracula(),
        "Tokyo Night" => tokyo_night(),
        "Kodemo Ocean" => kodemo_ocean(),
        "AMOLED Black" => amoled_black(),
        "GTK Native" => native_gtk_palette(),
        "Qt Native" => native_qt_palette(),
        _ => return None,
    })
}

fn kodemo_dark() -> ThemePalette {
    ThemePalette {
        dark: true,
        bg: c(0x1e1e1e),
        panel: c(0x252526),
        panel_alt: c(0x2d2d30),
        editor: c(0x1e1e1e),
        titlebar: c(0x181818),
        statusbar: c(0x007acc),
        activity: c(0x333333),
        activity_fg: c(0xffffff),
        border: c(0x2b2b2b),
        border_strong: c(0x454545),
        fg: c(0xcccccc),
        fg_muted: c(0x9d9d9d),
        fg_faint: c(0x6f6f6f),
        accent: c(0x007acc),
        accent_fg: c(0xffffff),
        hover: c(0x2a2d2e),
        selection: c(0x264f78),
        active_line: Color32::from_rgba_premultiplied(255, 255, 255, 10),
        gutter: c(0x858585),
        input_bg: c(0x3c3c3c),
        tab_active: c(0x1e1e1e),
        tab_inactive: c(0x2d2d2d),
        tab_fg: c(0xffffff),
        tab_fg_inactive: c(0x9d9d9d),
        scrollbar: Color32::from_rgba_premultiplied(121, 121, 121, 70),
        scrollbar_hover: Color32::from_rgba_premultiplied(150, 150, 150, 130),
        error: c(0xf14c4c),
        warning: c(0xcca700),
        success: c(0x89d185),
        info: c(0x3794ff),
        syntax_keyword: c(0x569cd6),
        syntax_string: c(0xce9178),
        syntax_comment: c(0x6a9955),
        syntax_number: c(0xb5cea8),
        syntax_function: c(0xdcdcaa),
        syntax_type: c(0x4ec9b0),
        syntax_variable: c(0x9cdcfe),
        syntax_macro: c(0xd7ba7d),
        syntax_operator: c(0xd4d4d4),
        syntax_tag: c(0x569cd6),
        syntax_attribute: c(0x9cdcfe),
        syntax_property: c(0x9cdcfe),
        syntax_constant: c(0x4fc1ff),
    }
}

fn kodemo_light() -> ThemePalette {
    ThemePalette {
        dark: false,
        bg: c(0xffffff),
        panel: c(0xf3f3f3),
        panel_alt: c(0xe8e8e8),
        editor: c(0xffffff),
        titlebar: c(0xf8f8f8),
        statusbar: c(0x007acc),
        activity: c(0xf8f8f8),
        activity_fg: c(0x424242),
        border: c(0xe5e5e5),
        border_strong: c(0xcecece),
        fg: c(0x333333),
        fg_muted: c(0x717171),
        fg_faint: c(0xa0a0a0),
        accent: c(0x005fb8),
        accent_fg: c(0xffffff),
        hover: c(0xe8e8e8),
        selection: c(0xadd6ff),
        active_line: Color32::from_rgba_premultiplied(0, 0, 0, 8),
        gutter: c(0x939393),
        input_bg: c(0xffffff),
        tab_active: c(0xffffff),
        tab_inactive: c(0xeeeeee),
        tab_fg: c(0x333333),
        tab_fg_inactive: c(0x8b8b8b),
        scrollbar: Color32::from_rgba_premultiplied(100, 100, 100, 50),
        scrollbar_hover: Color32::from_rgba_premultiplied(70, 70, 70, 110),
        error: c(0xcd3131),
        warning: c(0xbf8803),
        success: c(0x14a44d),
        info: c(0x1a85ff),
        syntax_keyword: c(0x0000ff),
        syntax_string: c(0xa31515),
        syntax_comment: c(0x008000),
        syntax_number: c(0x098658),
        syntax_function: c(0x795e26),
        syntax_type: c(0x267f99),
        syntax_variable: c(0x001080),
        syntax_macro: c(0xaf00db),
        syntax_operator: c(0x333333),
        syntax_tag: c(0x800000),
        syntax_attribute: c(0xe50000),
        syntax_property: c(0x001080),
        syntax_constant: c(0x0070c1),
    }
}

fn gruvbox_dark() -> ThemePalette {
    ThemePalette {
        dark: true,
        bg: c(0x282828),
        panel: c(0x32302f),
        panel_alt: c(0x3c3836),
        editor: c(0x282828),
        titlebar: c(0x1d2021),
        statusbar: c(0x3c3836),
        activity: c(0x1d2021),
        activity_fg: c(0xebdbb2),
        border: c(0x3c3836),
        border_strong: c(0x504945),
        fg: c(0xebdbb2),
        fg_muted: c(0xa89984),
        fg_faint: c(0x928374),
        accent: c(0xb8bb26),
        accent_fg: c(0x282828),
        hover: c(0x3c3836),
        selection: c(0x504945),
        active_line: Color32::from_rgba_premultiplied(255, 255, 255, 10),
        gutter: c(0x7c6f64),
        input_bg: c(0x3c3836),
        tab_active: c(0x282828),
        tab_inactive: c(0x32302f),
        tab_fg: c(0xb8bb26),
        tab_fg_inactive: c(0xa89984),
        scrollbar: Color32::from_rgba_premultiplied(168, 153, 132, 70),
        scrollbar_hover: Color32::from_rgba_premultiplied(235, 219, 178, 120),
        error: c(0xfb4934),
        warning: c(0xfabd2f),
        success: c(0xb8bb26),
        info: c(0x83a598),
        syntax_keyword: c(0xfb4934),
        syntax_string: c(0xb8bb26),
        syntax_comment: c(0x928374),
        syntax_number: c(0xd3869b),
        syntax_function: c(0xfabd2f),
        syntax_type: c(0x8ec07c),
        syntax_variable: c(0x83a598),
        syntax_macro: c(0xfe8019),
        syntax_operator: c(0xd5c4a1),
        syntax_tag: c(0x8ec07c),
        syntax_attribute: c(0x83a598),
        syntax_property: c(0x83a598),
        syntax_constant: c(0xd3869b),
    }
}

fn gruvbox_light() -> ThemePalette {
    ThemePalette {
        dark: false,
        bg: c(0xfbf1c7),
        panel: c(0xf2e5bc),
        panel_alt: c(0xebdbb2),
        editor: c(0xfbf1c7),
        titlebar: c(0xf2e5bc),
        statusbar: c(0xebdbb2),
        activity: c(0xf2e5bc),
        activity_fg: c(0x3c3836),
        border: c(0xd5c4a1),
        border_strong: c(0xbdae93),
        fg: c(0x3c3836),
        fg_muted: c(0x665c54),
        fg_faint: c(0x928374),
        accent: c(0xd65d0e),
        accent_fg: c(0xfbf1c7),
        hover: c(0xebdbb2),
        selection: c(0xd5c4a1),
        active_line: Color32::from_rgba_premultiplied(60, 56, 54, 10),
        gutter: c(0x928374),
        input_bg: c(0xfbf1c7),
        tab_active: c(0xfbf1c7),
        tab_inactive: c(0xf2e5bc),
        tab_fg: c(0xaf3a03),
        tab_fg_inactive: c(0x665c54),
        scrollbar: Color32::from_rgba_premultiplied(102, 92, 84, 60),
        scrollbar_hover: Color32::from_rgba_premultiplied(60, 56, 54, 115),
        error: c(0x9d0006),
        warning: c(0xb57614),
        success: c(0x79740e),
        info: c(0x458588),
        syntax_keyword: c(0x9d0006),
        syntax_string: c(0x79740e),
        syntax_comment: c(0x928374),
        syntax_number: c(0x8f3f71),
        syntax_function: c(0xb57614),
        syntax_type: c(0x427b58),
        syntax_variable: c(0x458588),
        syntax_macro: c(0xd65d0e),
        syntax_operator: c(0x3c3836),
        syntax_tag: c(0x427b58),
        syntax_attribute: c(0x458588),
        syntax_property: c(0x458588),
        syntax_constant: c(0x8f3f71),
    }
}

fn solarized_dark() -> ThemePalette {
    ThemePalette {
        dark: true,
        bg: c(0x002b36),
        panel: c(0x073642),
        panel_alt: c(0x0a4050),
        editor: c(0x002b36),
        titlebar: c(0x00252f),
        statusbar: c(0x073642),
        activity: c(0x00252f),
        activity_fg: c(0x93a1a1),
        border: c(0x0d4b5a),
        border_strong: c(0x586e75),
        fg: c(0x93a1a1),
        fg_muted: c(0x839496),
        fg_faint: c(0x586e75),
        accent: c(0x268bd2),
        accent_fg: c(0x002b36),
        hover: c(0x0d4b5a),
        selection: c(0x15506a),
        active_line: Color32::from_rgba_premultiplied(147, 161, 161, 10),
        gutter: c(0x586e75),
        input_bg: c(0x073642),
        tab_active: c(0x002b36),
        tab_inactive: c(0x073642),
        tab_fg: c(0x2aa198),
        tab_fg_inactive: c(0x839496),
        scrollbar: Color32::from_rgba_premultiplied(131, 148, 150, 65),
        scrollbar_hover: Color32::from_rgba_premultiplied(147, 161, 161, 125),
        error: c(0xdc322f),
        warning: c(0xb58900),
        success: c(0x859900),
        info: c(0x2aa198),
        syntax_keyword: c(0x859900),
        syntax_string: c(0x2aa198),
        syntax_comment: c(0x586e75),
        syntax_number: c(0xd33682),
        syntax_function: c(0x268bd2),
        syntax_type: c(0xb58900),
        syntax_variable: c(0x6c71c4),
        syntax_macro: c(0xcb4b16),
        syntax_operator: c(0x93a1a1),
        syntax_tag: c(0x268bd2),
        syntax_attribute: c(0x6c71c4),
        syntax_property: c(0x2aa198),
        syntax_constant: c(0xcb4b16),
    }
}

fn catppuccin() -> ThemePalette {
    ThemePalette {
        dark: true,
        bg: c(0x1e1e2e),
        panel: c(0x181825),
        panel_alt: c(0x313244),
        editor: c(0x1e1e2e),
        titlebar: c(0x11111b),
        statusbar: c(0x181825),
        activity: c(0x11111b),
        activity_fg: c(0xcba6f7),
        border: c(0x313244),
        border_strong: c(0x45475a),
        fg: c(0xcdd6f4),
        fg_muted: c(0xa6adc8),
        fg_faint: c(0x6c7086),
        accent: c(0xcba6f7),
        accent_fg: c(0x11111b),
        hover: c(0x313244),
        selection: c(0x45475a),
        active_line: Color32::from_rgba_premultiplied(205, 214, 244, 10),
        gutter: c(0x6c7086),
        input_bg: c(0x313244),
        tab_active: c(0x1e1e2e),
        tab_inactive: c(0x181825),
        tab_fg: c(0xcba6f7),
        tab_fg_inactive: c(0xa6adc8),
        scrollbar: Color32::from_rgba_premultiplied(108, 112, 134, 70),
        scrollbar_hover: Color32::from_rgba_premultiplied(205, 214, 244, 120),
        error: c(0xf38ba8),
        warning: c(0xf9e2af),
        success: c(0xa6e3a1),
        info: c(0x89b4fa),
        syntax_keyword: c(0xcba6f7),
        syntax_string: c(0xa6e3a1),
        syntax_comment: c(0x6c7086),
        syntax_number: c(0xfab387),
        syntax_function: c(0x89b4fa),
        syntax_type: c(0xf9e2af),
        syntax_variable: c(0xcdd6f4),
        syntax_macro: c(0xf38ba8),
        syntax_operator: c(0x94e2d5),
        syntax_tag: c(0xf38ba8),
        syntax_attribute: c(0xf9e2af),
        syntax_property: c(0x89b4fa),
        syntax_constant: c(0xfab387),
    }
}

fn dracula() -> ThemePalette {
    ThemePalette {
        dark: true,
        bg: c(0x282a36),
        panel: c(0x21222c),
        panel_alt: c(0x343746),
        editor: c(0x282a36),
        titlebar: c(0x191a21),
        statusbar: c(0x21222c),
        activity: c(0x191a21),
        activity_fg: c(0xbd93f9),
        border: c(0x343746),
        border_strong: c(0x44475a),
        fg: c(0xf8f8f2),
        fg_muted: c(0xbfc0c7),
        fg_faint: c(0x6272a4),
        accent: c(0xbd93f9),
        accent_fg: c(0x282a36),
        hover: c(0x343746),
        selection: c(0x44475a),
        active_line: Color32::from_rgba_premultiplied(248, 248, 242, 10),
        gutter: c(0x6272a4),
        input_bg: c(0x343746),
        tab_active: c(0x282a36),
        tab_inactive: c(0x21222c),
        tab_fg: c(0xbd93f9),
        tab_fg_inactive: c(0xbfc0c7),
        scrollbar: Color32::from_rgba_premultiplied(98, 114, 164, 80),
        scrollbar_hover: Color32::from_rgba_premultiplied(189, 147, 249, 130),
        error: c(0xff5555),
        warning: c(0xf1fa8c),
        success: c(0x50fa7b),
        info: c(0x8be9fd),
        syntax_keyword: c(0xff79c6),
        syntax_string: c(0xf1fa8c),
        syntax_comment: c(0x6272a4),
        syntax_number: c(0xbd93f9),
        syntax_function: c(0x50fa7b),
        syntax_type: c(0x8be9fd),
        syntax_variable: c(0xf8f8f2),
        syntax_macro: c(0xffb86c),
        syntax_operator: c(0xff79c6),
        syntax_tag: c(0xff79c6),
        syntax_attribute: c(0x50fa7b),
        syntax_property: c(0x8be9fd),
        syntax_constant: c(0xbd93f9),
    }
}

fn tokyo_night() -> ThemePalette {
    ThemePalette {
        dark: true,
        bg: c(0x1a1b26),
        panel: c(0x16161e),
        panel_alt: c(0x292e42),
        editor: c(0x1a1b26),
        titlebar: c(0x13131a),
        statusbar: c(0x16161e),
        activity: c(0x13131a),
        activity_fg: c(0x7aa2f7),
        border: c(0x292e42),
        border_strong: c(0x3b4261),
        fg: c(0xc0caf5),
        fg_muted: c(0xa9b1d6),
        fg_faint: c(0x565f89),
        accent: c(0x7aa2f7),
        accent_fg: c(0x1a1b26),
        hover: c(0x292e42),
        selection: c(0x33467c),
        active_line: Color32::from_rgba_premultiplied(192, 202, 245, 10),
        gutter: c(0x3b4261),
        input_bg: c(0x292e42),
        tab_active: c(0x1a1b26),
        tab_inactive: c(0x16161e),
        tab_fg: c(0x7aa2f7),
        tab_fg_inactive: c(0xa9b1d6),
        scrollbar: Color32::from_rgba_premultiplied(59, 66, 97, 80),
        scrollbar_hover: Color32::from_rgba_premultiplied(122, 162, 247, 130),
        error: c(0xf7768e),
        warning: c(0xe0af68),
        success: c(0x9ece6a),
        info: c(0x7dcfff),
        syntax_keyword: c(0xbb9af7),
        syntax_string: c(0x9ece6a),
        syntax_comment: c(0x565f89),
        syntax_number: c(0xff9e64),
        syntax_function: c(0x7aa2f7),
        syntax_type: c(0x2ac3de),
        syntax_variable: c(0xc0caf5),
        syntax_macro: c(0xff9e64),
        syntax_operator: c(0x89ddff),
        syntax_tag: c(0xf7768e),
        syntax_attribute: c(0xe0af68),
        syntax_property: c(0x7dcfff),
        syntax_constant: c(0xff9e64),
    }
}

fn amoled_black() -> ThemePalette {
    ThemePalette {
        dark: true,
        bg: Color32::BLACK,
        panel: Color32::from_rgb(5, 5, 5),
        panel_alt: c(0x0c0c0c),
        editor: Color32::BLACK,
        titlebar: Color32::BLACK,
        statusbar: c(0x050505),
        activity: Color32::BLACK,
        activity_fg: c(0x8ab4ff),
        border: c(0x171717),
        border_strong: c(0x2b2b2b),
        fg: c(0xf2f2f2),
        fg_muted: c(0xa3a3a3),
        fg_faint: c(0x666666),
        accent: c(0x4d9fff),
        accent_fg: Color32::BLACK,
        hover: c(0x111111),
        selection: c(0x123a63),
        active_line: Color32::from_rgba_premultiplied(255, 255, 255, 8),
        gutter: c(0x595959),
        input_bg: c(0x0c0c0c),
        tab_active: Color32::BLACK,
        tab_inactive: c(0x050505),
        tab_fg: c(0x7cb3ff),
        tab_fg_inactive: c(0xa3a3a3),
        scrollbar: Color32::from_rgba_premultiplied(255, 255, 255, 55),
        scrollbar_hover: Color32::from_rgba_premultiplied(255, 255, 255, 120),
        error: c(0xff6b6b),
        warning: c(0xffcc66),
        success: c(0x66e08a),
        info: c(0x6cb6ff),
        syntax_keyword: c(0x82aaff),
        syntax_string: c(0x8ce99a),
        syntax_comment: c(0x5f6b7a),
        syntax_number: c(0xffb86c),
        syntax_function: c(0xffd479),
        syntax_type: c(0x5ce1e6),
        syntax_variable: c(0xd6e2ff),
        syntax_macro: c(0xff9e64),
        syntax_operator: c(0xc9d1d9),
        syntax_tag: c(0xff7b72),
        syntax_attribute: c(0xffd479),
        syntax_property: c(0x7dcfff),
        syntax_constant: c(0xffb86c),
    }
}

fn monokai() -> ThemePalette {
    ThemePalette {
        dark: true,
        bg: c(0x272822),
        panel: c(0x1e1f1c),
        panel_alt: c(0x34352f),
        editor: c(0x272822),
        titlebar: c(0x1e1f1c),
        statusbar: c(0x414339),
        activity: c(0x1e1f1c),
        activity_fg: c(0xf8f8f2),
        border: c(0x414339),
        border_strong: c(0x55564d),
        fg: c(0xf8f8f2),
        fg_muted: c(0xa8a8a2),
        fg_faint: c(0x75715e),
        accent: c(0xf92672),
        accent_fg: c(0xffffff),
        hover: c(0x3e3d32),
        selection: c(0x49483e),
        active_line: Color32::from_rgba_premultiplied(255, 255, 255, 10),
        gutter: c(0x90908a),
        input_bg: c(0x3e3d32),
        tab_active: c(0x272822),
        tab_inactive: c(0x1e1f1c),
        tab_fg: c(0xf8f8f2),
        tab_fg_inactive: c(0xa8a8a2),
        scrollbar: Color32::from_rgba_premultiplied(168, 168, 162, 65),
        scrollbar_hover: Color32::from_rgba_premultiplied(248, 248, 242, 120),
        error: c(0xf92672),
        warning: c(0xe6db74),
        success: c(0xa6e22e),
        info: c(0x66d9ef),
        syntax_keyword: c(0xf92672),
        syntax_string: c(0xe6db74),
        syntax_comment: c(0x75715e),
        syntax_number: c(0xae81ff),
        syntax_function: c(0xa6e22e),
        syntax_type: c(0x66d9ef),
        syntax_variable: c(0xfd971f),
        syntax_macro: c(0xae81ff),
        syntax_operator: c(0xf92672),
        syntax_tag: c(0xf92672),
        syntax_attribute: c(0xa6e22e),
        syntax_property: c(0x66d9ef),
        syntax_constant: c(0xae81ff),
    }
}

fn solarized_light() -> ThemePalette {
    ThemePalette {
        dark: false,
        bg: c(0xfdf6e3),
        panel: c(0xeee8d5),
        panel_alt: c(0xe4ddc8),
        editor: c(0xfdf6e3),
        titlebar: c(0xeee8d5),
        statusbar: c(0xeee8d5),
        activity: c(0xeee8d5),
        activity_fg: c(0x586e75),
        border: c(0xddd6c1),
        border_strong: c(0xc9c2aa),
        fg: c(0x657b83),
        fg_muted: c(0x839496),
        fg_faint: c(0x93a1a1),
        accent: c(0x268bd2),
        accent_fg: c(0xffffff),
        hover: c(0xe4ddc8),
        selection: c(0xdfe5c7),
        active_line: Color32::from_rgba_premultiplied(0, 0, 0, 8),
        gutter: c(0x93a1a1),
        input_bg: c(0xfdf6e3),
        tab_active: c(0xfdf6e3),
        tab_inactive: c(0xeee8d5),
        tab_fg: c(0x586e75),
        tab_fg_inactive: c(0x93a1a1),
        scrollbar: Color32::from_rgba_premultiplied(88, 110, 117, 45),
        scrollbar_hover: Color32::from_rgba_premultiplied(88, 110, 117, 100),
        error: c(0xdc322f),
        warning: c(0xb58900),
        success: c(0x859900),
        info: c(0x268bd2),
        syntax_keyword: c(0x859900),
        syntax_string: c(0x2aa198),
        syntax_comment: c(0x93a1a1),
        syntax_number: c(0xd33682),
        syntax_function: c(0x268bd2),
        syntax_type: c(0xb58900),
        syntax_variable: c(0x268bd2),
        syntax_macro: c(0xcb4b16),
        syntax_operator: c(0x657b83),
        syntax_tag: c(0x268bd2),
        syntax_attribute: c(0x6c71c4),
        syntax_property: c(0x2aa198),
        syntax_constant: c(0xcb4b16),
    }
}

fn kodemo_ocean() -> ThemePalette {
    ThemePalette {
        dark: true,
        bg: c(0x0b1220),
        panel: c(0x101a2c),
        panel_alt: c(0x152238),
        editor: c(0x0b1220),
        titlebar: c(0x08101c),
        statusbar: c(0x0e7490),
        activity: c(0x081522),
        activity_fg: c(0xa5f3fc),
        border: c(0x17304a),
        border_strong: c(0x244764),
        fg: c(0xd6e6f2),
        fg_muted: c(0x7f9db3),
        fg_faint: c(0x537086),
        accent: c(0x22d3ee),
        accent_fg: c(0x04141c),
        hover: c(0x152c44),
        selection: c(0x164e63),
        active_line: Color32::from_rgba_premultiplied(34, 211, 238, 10),
        gutter: c(0x5f7f94),
        input_bg: c(0x132236),
        tab_active: c(0x0b1220),
        tab_inactive: c(0x101a2c),
        tab_fg: c(0x67e8f9),
        tab_fg_inactive: c(0x7f9db3),
        scrollbar: Color32::from_rgba_premultiplied(125, 211, 252, 55),
        scrollbar_hover: Color32::from_rgba_premultiplied(125, 211, 252, 120),
        error: c(0xfb7185),
        warning: c(0xfbbf24),
        success: c(0x34d399),
        info: c(0x38bdf8),
        syntax_keyword: c(0x67e8f9),
        syntax_string: c(0xa7f3d0),
        syntax_comment: c(0x52798d),
        syntax_number: c(0xf0abfc),
        syntax_function: c(0xfde68a),
        syntax_type: c(0x7dd3fc),
        syntax_variable: c(0xc4b5fd),
        syntax_macro: c(0xfbbf24),
        syntax_operator: c(0x93c5fd),
        syntax_tag: c(0x67e8f9),
        syntax_attribute: c(0xfbbf24),
        syntax_property: c(0x7dd3fc),
        syntax_constant: c(0xf0abfc),
    }
}

struct NativeChrome {
    bg: Color32,
    base: Color32,
    titlebar: Color32,
    fg: Color32,
    accent: Color32,
    accent_fg: Color32,
    border: Color32,
    error: Color32,
    warning: Color32,
    success: Color32,
    dark: bool,
}

fn blend(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let channel = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
    Color32::from_rgb(
        channel(a.r(), b.r()),
        channel(a.g(), b.g()),
        channel(a.b(), b.b()),
    )
}

fn contrast_text(background: Color32) -> Color32 {
    let luma = 0.2126 * background.r() as f32
        + 0.7152 * background.g() as f32
        + 0.0722 * background.b() as f32;
    if luma > 140.0 {
        Color32::BLACK
    } else {
        Color32::WHITE
    }
}

fn looks_dark(color: Color32) -> bool {
    let luma = 0.2126 * color.r() as f32
        + 0.7152 * color.g() as f32
        + 0.0722 * color.b() as f32;
    luma < 118.0
}

fn native_palette(chrome: NativeChrome) -> ThemePalette {
    let template = if chrome.dark {
        kodemo_dark()
    } else {
        kodemo_light()
    };
    let hover = blend(chrome.bg, chrome.fg, 0.09);
    let panel_alt = blend(chrome.bg, chrome.fg, if chrome.dark { 0.07 } else { 0.055 });
    let muted = blend(chrome.fg, chrome.bg, 0.34);
    let faint = blend(chrome.fg, chrome.bg, 0.52);
    ThemePalette {
        dark: chrome.dark,
        bg: chrome.bg,
        panel: chrome.bg,
        panel_alt,
        editor: chrome.base,
        titlebar: chrome.titlebar,
        statusbar: chrome.bg,
        activity: chrome.titlebar,
        activity_fg: chrome.accent,
        border: chrome.border,
        border_strong: blend(chrome.bg, chrome.fg, 0.22),
        fg: chrome.fg,
        fg_muted: muted,
        fg_faint: faint,
        accent: chrome.accent,
        accent_fg: chrome.accent_fg,
        hover,
        selection: blend(chrome.bg, chrome.accent, 0.34),
        active_line: alpha(chrome.fg, 9),
        gutter: blend(chrome.fg, chrome.bg, 0.48),
        input_bg: chrome.base,
        tab_active: chrome.bg,
        tab_inactive: chrome.titlebar,
        tab_fg: chrome.accent,
        tab_fg_inactive: muted,
        scrollbar: alpha(chrome.fg, 58),
        scrollbar_hover: alpha(chrome.fg, 120),
        error: chrome.error,
        warning: chrome.warning,
        success: chrome.success,
        info: chrome.accent,
        syntax_keyword: chrome.accent,
        syntax_string: template.syntax_string,
        syntax_comment: blend(template.syntax_comment, chrome.bg, 0.1),
        syntax_number: template.syntax_number,
        syntax_function: blend(chrome.accent, chrome.fg, 0.25),
        syntax_type: template.syntax_type,
        syntax_variable: template.syntax_variable,
        syntax_macro: template.syntax_macro,
        syntax_operator: chrome.fg,
        syntax_tag: chrome.accent,
        syntax_attribute: template.syntax_attribute,
        syntax_property: template.syntax_property,
        syntax_constant: template.syntax_constant,
    }
}

fn command_line(program: &str, args: &[&str]) -> Option<String> {
    let output = std::process::Command::new(program)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    (!text.is_empty()).then_some(text)
}

fn gtk_theme_name() -> Option<String> {
    if let Ok(theme) = std::env::var("GTK_THEME") {
        let theme = theme.split(':').next().unwrap_or(&theme).trim();
        if !theme.is_empty() {
            return Some(theme.to_owned());
        }
    }
    for (program, args) in [
        ("xfconf-query", &["-c", "xsettings", "-p", "/Net/ThemeName"][..]),
        ("gsettings", &["get", "org.gnome.desktop.interface", "gtk-theme"][..]),
    ] {
        if let Some(value) = command_line(program, args) {
            let value = value.trim_matches('\'').trim_matches('"').trim();
            if !value.is_empty() && value != "''" {
                return Some(value.to_owned());
            }
        }
    }
    for path in gtk_settings_paths() {
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            if key.trim() == "gtk-theme-name" {
                let value = value.trim().trim_matches('"');
                if !value.is_empty() {
                    return Some(value.to_owned());
                }
            }
        }
    }
    None
}

fn gtk_settings_paths() -> Vec<std::path::PathBuf> {
    let mut paths = Vec::new();
    if let Some(config) = std::env::var_os("XDG_CONFIG_HOME") {
        paths.push(std::path::PathBuf::from(config).join("gtk-3.0/settings.ini"));
    }
    if let Some(home) = std::env::var_os("HOME") {
        paths.push(std::path::PathBuf::from(&home).join(".config/gtk-3.0/settings.ini"));
        paths.push(std::path::PathBuf::from(&home).join(".config/gtk-4.0/settings.ini"));
    }
    paths
}

fn theme_dirs() -> Vec<std::path::PathBuf> {
    let mut dirs = Vec::new();
    if let Some(data) = std::env::var_os("XDG_DATA_HOME") {
        dirs.push(std::path::PathBuf::from(data).join("themes"));
    }
    if let Some(home) = std::env::var_os("HOME") {
        dirs.push(std::path::PathBuf::from(&home).join(".themes"));
        dirs.push(std::path::PathBuf::from(&home).join(".local/share/themes"));
    }
    dirs.push(std::path::PathBuf::from("/usr/share/themes"));
    dirs
}

fn parse_css_color(value: &str) -> Option<Color32> {
    let value = value.trim().trim_end_matches(';').trim();
    if let Some(hex) = value.strip_prefix('#') {
        let digits = hex.chars().collect::<Vec<_>>();
        let pair = |index: usize| u8::from_str_radix(&digits[index..index + 2].iter().collect::<String>(), 16).ok();
        return match digits.len() {
            3 => {
                let r = u8::from_str_radix(&digits[0].to_string(), 16).ok()? * 17;
                let g = u8::from_str_radix(&digits[1].to_string(), 16).ok()? * 17;
                let b = u8::from_str_radix(&digits[2].to_string(), 16).ok()? * 17;
                Some(Color32::from_rgb(r, g, b))
            }
            6 => Some(Color32::from_rgb(pair(0)?, pair(2)?, pair(4)?)),
            8 => Some(Color32::from_rgba_unmultiplied(pair(0)?, pair(2)?, pair(4)?, pair(6)?)),
            _ => None,
        };
    }
    // KDE/Qt palette files commonly store colors as bare `R,G,B` triplets.
    if !value.contains('(') {
        let channels: Vec<&str> = value.split(',').map(str::trim).collect();
        if channels.len() >= 3 {
            if let (Ok(r), Ok(g), Ok(b)) = (
                channels[0].parse::<u8>(),
                channels[1].parse::<u8>(),
                channels[2].parse::<u8>(),
            ) {
                return Some(Color32::from_rgb(r, g, b));
            }
        }
        return None;
    }
    let open = value.find('(')?;
    let close = value.rfind(')')?;
    let name = value[..open].trim().to_ascii_lowercase();
    let parts: Vec<&str> = value[open + 1..close]
        .split(',')
        .map(str::trim)
        .collect();
    if parts.len() < 3 {
        return None;
    }
    let channel = |part: &str| part.trim_end_matches('%').parse::<f32>().ok();
    let (r, g, b) = (channel(parts[0])?, channel(parts[1])?, channel(parts[2])?);
    let alpha = parts
        .get(3)
        .and_then(|a| a.trim_end_matches('%').parse::<f32>().ok())
        .map(|a| if a <= 1.0 { a * 255.0 } else { a })
        .unwrap_or(255.0);
    if name == "rgb" || name == "rgba" {
        Some(Color32::from_rgba_unmultiplied(
            r.round() as u8,
            g.round() as u8,
            b.round() as u8,
            alpha.round().clamp(0.0, 255.0) as u8,
        ))
    } else {
        None
    }
}

fn collect_gtk_defines(path: &std::path::Path, seen: &mut std::collections::HashSet<std::path::PathBuf>, out: &mut std::collections::HashMap<String, Color32>) {
    let path = path.to_path_buf();
    if !seen.insert(path.clone()) {
        return;
    }
    let Ok(css) = std::fs::read_to_string(&path) else {
        return;
    };
    for line in css.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("@define-color") {
            let mut parts = rest.split_whitespace();
            if let (Some(name), Some(value)) = (parts.next(), parts.next()) {
                if let Some(color) = parse_css_color(value) {
                    out.entry(name.to_owned()).or_insert(color);
                }
            }
        } else if let Some(rest) = line.strip_prefix("@import") {
            if let Some(start) = rest.find('"') {
                if let Some(end) = rest[start + 1..].find('"') {
                    let import = &rest[start + 1..start + 1 + end];
                    let import_path = path.parent().unwrap_or_else(|| std::path::Path::new(".")).join(import);
                    collect_gtk_defines(&import_path, seen, out);
                }
            }
        }
    }
}

fn gtk_defines(theme: &str, dark: bool) -> std::collections::HashMap<String, Color32> {
    let mut out = std::collections::HashMap::new();
    let mut seen = std::collections::HashSet::new();
    for base in theme_dirs() {
        let dir = base.join(theme);
        let gtk3 = dir.join("gtk-3.0");
        if !gtk3.is_dir() {
            continue;
        }
        let preferred = if dark {
            gtk3.join("gtk-dark.css")
        } else {
            gtk3.join("gtk.css")
        };
        let primary = if preferred.is_file() {
            preferred
        } else {
            gtk3.join("gtk.css")
        };
        collect_gtk_defines(&primary, &mut seen, &mut out);
        if out.contains_key("theme_bg_color") {
            break;
        }
        if let Ok(entries) = std::fs::read_dir(&gtk3) {
            for entry in entries.flatten() {
                if entry.path().extension().and_then(|e| e.to_str()) == Some("css") {
                    collect_gtk_defines(&entry.path(), &mut seen, &mut out);
                }
            }
        }
        if !out.is_empty() {
            break;
        }
    }
    out
}

fn native_gtk_palette() -> ThemePalette {
    let theme = gtk_theme_name().unwrap_or_default();
    let prefers_dark = command_line(
        "gsettings",
        &["get", "org.gnome.desktop.interface", "color-scheme"],
    )
    .map(|value| value.contains("dark"))
    .unwrap_or(false)
        || theme.to_ascii_lowercase().contains("dark")
        || theme.to_ascii_lowercase().contains("night");
    let defs = gtk_defines(&theme, prefers_dark);
    let bg = defs
        .get("theme_bg_color")
        .copied()
        .unwrap_or(if prefers_dark { c(0x242424) } else { c(0xf6f5f4) });
    let base = defs.get("theme_base_color").copied().unwrap_or(bg);
    let fg = defs
        .get("theme_fg_color")
        .or_else(|| defs.get("theme_text_color"))
        .copied()
        .unwrap_or(if looks_dark(bg) { c(0xeeeeee) } else { c(0x2e3436) });
    let accent = defs
        .get("theme_selected_bg_color")
        .copied()
        .unwrap_or(c(0x3584e4));
    let accent_fg = defs
        .get("theme_selected_fg_color")
        .copied()
        .unwrap_or_else(|| contrast_text(accent));
    let border = defs
        .get("borders")
        .copied()
        .unwrap_or_else(|| blend(bg, fg, 0.18));
    let titlebar = defs
        .get("wm_focused_bg")
        .or_else(|| defs.get("theme_bg_color"))
        .copied()
        .unwrap_or(bg);
    let dark = looks_dark(bg);
    native_palette(NativeChrome {
        bg,
        base,
        titlebar,
        fg,
        accent,
        accent_fg,
        border,
        error: defs.get("error_color").copied().unwrap_or(c(0xe01b24)),
        warning: defs.get("warning_color").copied().unwrap_or(c(0xe5a50a)),
        success: defs.get("success_color").copied().unwrap_or(c(0x2ec27e)),
        dark,
    })
}

fn parse_ini(path: &std::path::Path) -> std::collections::HashMap<String, std::collections::HashMap<String, String>> {
    let mut sections: std::collections::HashMap<String, std::collections::HashMap<String, String>> =
        std::collections::HashMap::new();
    let Ok(text) = std::fs::read_to_string(path) else {
        return sections;
    };
    let mut section = String::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].trim().to_owned();
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            sections
                .entry(section.clone())
                .or_default()
                .insert(key.trim().to_owned(), value.trim().to_owned());
        }
    }
    sections
}

fn qt_config_path() -> Option<std::path::PathBuf> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| std::path::PathBuf::from(home).join(".config")))?;
    for name in ["qt6ct/qt6ct.conf", "qt5ct/qt5ct.conf"] {
        let path = config.join(name);
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

fn qt_scheme_path(config: &std::collections::HashMap<String, std::collections::HashMap<String, String>>) -> Option<std::path::PathBuf> {
    let value = config.get("Appearance")?.get("color_scheme_path")?;
    let path = std::path::PathBuf::from(value);
    path.is_file().then_some(path)
}

fn ini_color(
    sections: &std::collections::HashMap<String, std::collections::HashMap<String, String>>,
    section: &str,
    key: &str,
) -> Option<Color32> {
    sections.get(section)?.get(key).and_then(|v| parse_css_color(v))
}

fn native_qt_palette() -> ThemePalette {
    let config_path = qt_config_path();
    let config = config_path
        .as_deref()
        .map(parse_ini)
        .unwrap_or_default();
    let style = config
        .get("Appearance")
        .and_then(|s| s.get("style"))
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();
    let scheme = qt_scheme_path(&config).map(|path| parse_ini(&path)).unwrap_or_default();
    let kde = std::env::var_os("HOME")
        .map(|home| std::path::PathBuf::from(home).join(".config/kdeglobals"))
        .filter(|path| path.is_file())
        .map(|path| parse_ini(&path))
        .unwrap_or_default();

    let window_bg = ini_color(&scheme, "Colors:Window", "BackgroundNormal")
        .or_else(|| ini_color(&kde, "Colors:Window", "BackgroundNormal"));
    let window_fg = ini_color(&scheme, "Colors:Window", "ForegroundNormal")
        .or_else(|| ini_color(&kde, "Colors:Window", "ForegroundNormal"));
    let base = ini_color(&scheme, "Colors:View", "BackgroundNormal")
        .or_else(|| ini_color(&scheme, "Colors:Base", "BackgroundNormal"))
        .or_else(|| ini_color(&kde, "Colors:View", "BackgroundNormal"));
    let accent = ini_color(&scheme, "Colors:Selection", "BackgroundNormal")
        .or_else(|| ini_color(&kde, "Colors:Selection", "BackgroundNormal"));
    let accent_fg = ini_color(&scheme, "Colors:Selection", "ForegroundNormal")
        .or_else(|| ini_color(&kde, "Colors:Selection", "ForegroundNormal"))
        .unwrap_or_else(|| accent.map(contrast_text).unwrap_or(Color32::WHITE));

    let style_dark = style.contains("dark") || style.contains("night");
    let mut dark = window_bg.map(looks_dark).unwrap_or(style_dark);
    if style_dark {
        dark = true;
    }
    // A light Qt color scheme can still be paired with a dark Kvantum style.
    // In that case use the Breeze Dark window colors rather than a bright sheet.
    let scheme_light = window_bg.map(|color| !looks_dark(color)).unwrap_or(false);
    let (fallback_bg, fallback_fg, fallback_base) = if dark {
        (c(0x1b1d20), c(0xeff0f1), c(0x232629))
    } else {
        (c(0xeff0f1), c(0x232629), c(0xffffff))
    };
    let bg = if scheme_light && dark {
        fallback_bg
    } else {
        window_bg.unwrap_or(fallback_bg)
    };
    let fg = if scheme_light && dark {
        fallback_fg
    } else {
        window_fg.unwrap_or(fallback_fg)
    };
    let base = if scheme_light && dark {
        fallback_base
    } else {
        base.unwrap_or(fallback_base)
    };
    let accent = accent.unwrap_or(c(0x3daee9));
    native_palette(NativeChrome {
        bg,
        base,
        titlebar: blend(bg, Color32::BLACK, if dark { 0.18 } else { 0.04 }),
        fg,
        accent,
        accent_fg,
        border: blend(bg, fg, 0.18),
        error: c(0xda4453),
        warning: c(0xf67400),
        success: c(0x27ae60),
        dark: looks_dark(bg),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_themes_are_registered_and_distinct() {
        for name in [
            "Gruvbox Light",
            "Solarized Dark",
            "Catppuccin",
            "Dracula",
            "Tokyo Night",
            "AMOLED Black",
            "GTK Native",
            "Qt Native",
        ] {
            assert!(theme_names().contains(&name), "missing theme {name}");
            assert!(builtin(name).is_some(), "theme {name} does not resolve");
        }
        assert_eq!(builtin("AMOLED Black").unwrap().bg, Color32::BLACK);
        assert!(!builtin("Gruvbox Light").unwrap().dark);
        assert!(builtin("Solarized Dark").unwrap().dark);
    }

    #[test]
    fn parses_gtk_qt_and_kde_colors() {
        assert_eq!(parse_css_color("#32302f"), Some(c(0x32302f)));
        assert_eq!(
            parse_css_color("rgba(50, 48, 47, 0.87)"),
            Some(Color32::from_rgba_unmultiplied(50, 48, 47, 222))
        );
        assert_eq!(parse_css_color("49,54,59"), Some(c(0x31363b)));
    }
}

pub fn install_fonts(ctx: &egui::Context, ui_size: f32, mono_size: f32) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "kodemo-ui".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../assets/fonts/Inter-Regular.ttf"
        ))),
    );
    fonts.font_data.insert(
        "kodemo-mono".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../assets/fonts/JetBrainsMono-Regular.ttf"
        ))),
    );
    // Space Grotesk Bold gives the product name its own voice. It is bundled
    // as a small latin subset so the binary stays light.
    fonts.font_data.insert(
        "kodemo-display".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../assets/fonts/SpaceGrotesk-Bold.ttf"
        ))),
    );
    // Phosphor Icons (MIT) is an external icon font: Kodemo draws every UI
    // glyph from it instead of bundling hand-drawn approximations.
    fonts.font_data.insert(
        "kodemo-phosphor".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../assets/fonts/Phosphor.ttf"
        ))),
    );
    fonts.families.insert(
        FontFamily::Name("phosphor".into()),
        vec!["kodemo-phosphor".to_owned()],
    );
    fonts.families.insert(
        FontFamily::Name("kodemo-display".into()),
        vec!["kodemo-display".to_owned(), "kodemo-ui".to_owned()],
    );
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "kodemo-ui".to_owned());
    fonts
        .families
        .entry(FontFamily::Monospace)
        .or_default()
        .insert(0, "kodemo-mono".to_owned());
    ctx.set_fonts(fonts);

    let mut style = (*ctx.style()).clone();
    style.text_styles = [
        (TextStyle::Small, FontId::new(11.5, FontFamily::Proportional)),
        (TextStyle::Body, FontId::new(ui_size, FontFamily::Proportional)),
        (TextStyle::Button, FontId::new(ui_size, FontFamily::Proportional)),
        (TextStyle::Heading, FontId::new((ui_size + 6.0).max(18.0), FontFamily::Proportional)),
        (TextStyle::Monospace, FontId::new(mono_size, FontFamily::Monospace)),
    ]
    .into();
    ctx.set_style(style);
}

pub fn apply_style(ctx: &egui::Context, settings: &Settings, p: &ThemePalette) {
    let mut style = (*ctx.style()).clone();
    let v = &mut style.visuals;
    v.dark_mode = p.dark;
    v.override_text_color = Some(p.fg);
    v.panel_fill = p.panel;
    v.window_fill = p.panel;
    v.extreme_bg_color = p.editor;
    v.faint_bg_color = p.panel_alt;
    v.code_bg_color = p.panel_alt;
    v.window_stroke = Stroke::new(1.0_f32, p.border_strong);
    v.window_corner_radius = CornerRadius::same(10);
    v.menu_corner_radius = CornerRadius::same(8);
    v.window_shadow = egui::epaint::Shadow {
        offset: [0, 10],
        blur: 28,
        spread: 0,
        color: Color32::from_black_alpha(96),
    };
    v.popup_shadow = egui::epaint::Shadow {
        offset: [0, 6],
        blur: 18,
        spread: 0,
        color: Color32::from_black_alpha(72),
    };
    v.window_highlight_topmost = true;
    v.hyperlink_color = p.accent;
    v.warn_fg_color = p.warning;
    v.error_fg_color = p.error;
    v.selection.bg_fill = p.selection;
    v.selection.stroke = Stroke::new(1.0_f32, p.fg);
    v.text_cursor.stroke = Stroke::new(1.6_f32, p.accent);
    v.text_cursor.preview = true;
    v.text_cursor.blink = true;
    v.button_frame = true;
    v.interact_cursor = Some(egui::CursorIcon::PointingHand);

    v.widgets.noninteractive.bg_fill = p.panel;
    v.widgets.noninteractive.weak_bg_fill = p.panel_alt;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, p.border);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, p.fg);
    v.widgets.noninteractive.corner_radius = CornerRadius::same(6);

    v.widgets.inactive.bg_fill = p.panel_alt;
    v.widgets.inactive.weak_bg_fill = p.panel_alt;
    v.widgets.inactive.bg_stroke = Stroke::new(0.0_f32, Color32::TRANSPARENT);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, p.fg);
    v.widgets.inactive.corner_radius = CornerRadius::same(6);

    v.widgets.hovered.bg_fill = p.hover;
    v.widgets.hovered.weak_bg_fill = p.hover;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, p.border_strong);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, p.fg);
    v.widgets.hovered.corner_radius = CornerRadius::same(6);

    v.widgets.active.bg_fill = p.panel_alt;
    v.widgets.active.weak_bg_fill = p.panel_alt;
    v.widgets.active.bg_stroke = Stroke::new(1.0_f32, p.accent);
    v.widgets.active.fg_stroke = Stroke::new(1.0_f32, p.fg);
    v.widgets.active.corner_radius = CornerRadius::same(6);

    v.widgets.open = v.widgets.hovered.clone();
    style.spacing.item_spacing = Vec2::new(7.0, 5.0);
    style.spacing.button_padding = Vec2::new(10.0, 5.0);
    style.spacing.window_margin = Margin::same(10);
    style.spacing.menu_margin = Margin::symmetric(8, 6);
    style.spacing.indent = 14.0;
    style.spacing.interact_size.y = 24.0;
    style.spacing.scroll.bar_width = 10.0;
    style.spacing.scroll.floating = true;
    style.spacing.scroll.bar_inner_margin = 2.0;
    style.spacing.scroll.bar_outer_margin = 0.0;
    style.spacing.scroll.floating_width = 4.0;
    style.spacing.scroll.dormant_background_opacity = 0.0;
    style.animation_time = if settings.animations { 0.16 } else { 0.0 };
    style.visuals.widgets.noninteractive.expansion = 0.0;
    ctx.set_style(style);
}

/// Font used for the Kodemo wordmark on the start page and in About.
pub fn display_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("kodemo-display".into()))
}

pub fn alpha(color: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_premultiplied(
        ((color.r() as u16 * alpha as u16) / 255) as u8,
        ((color.g() as u16 * alpha as u16) / 255) as u8,
        ((color.b() as u16 * alpha as u16) / 255) as u8,
        alpha,
    )
}

