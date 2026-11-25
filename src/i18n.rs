use crate::settings::Lang;

/// Tiny inline translation helper. Keeping both strings next to the call site
/// makes it hard for a translation to silently go missing from the UI.
pub fn t(lang: Lang, en: &'static str, pt: &'static str) -> &'static str {
    match lang {
        Lang::En => en,
        Lang::Pt => pt,
    }
}

pub fn theme_name(lang: Lang, name: &str) -> String {
    let pt = lang.is_pt();
    match name {
        "Kodemo Dark" => if pt { "Kodemo Escuro" } else { "Kodemo Dark" },
        "Kodemo Light" => if pt { "Kodemo Claro" } else { "Kodemo Light" },
        "Gruvbox Dark" => "Gruvbox Dark",
        "Gruvbox Light" => if pt { "Gruvbox Claro" } else { "Gruvbox Light" },
        "Monokai" => "Monokai",
        "Solarized Dark" => if pt { "Solarized Escuro" } else { "Solarized Dark" },
        "Solarized Light" => if pt { "Solarized Claro" } else { "Solarized Light" },
        "Catppuccin" => "Catppuccin",
        "Dracula" => "Dracula",
        "Tokyo Night" => "Tokyo Night",
        "Kodemo Ocean" => if pt { "Kodemo Oceano" } else { "Kodemo Ocean" },
        "AMOLED Black" => if pt { "AMOLED Preto" } else { "AMOLED Black" },
        "GTK Native" => if pt { "GTK Nativo" } else { "GTK Native" },
        "Qt Native" => if pt { "Qt Nativo" } else { "Qt Native" },
        "Custom" => if pt { "Personalizado" } else { "Custom" },
        _ => name,
    }
    .to_owned()
}
