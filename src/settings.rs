//! The choices and the game in progress, kept between sessions in `Checkers3D.ini` in the settings folder.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::PathBuf;

use crate::platform;
use crate::strings::Language;
use crate::themes::THEME_COUNT;

const NAME: &str = "Checkers3D";

/// Where the window was when it was neither maximized nor full screen.
#[derive(Clone, Copy)]
pub struct WindowPlacement {
    pub left: i32,
    pub top: i32,
    pub width: i32,
    pub height: i32,
}

pub struct Settings {
    pub theme: i32,
    pub level: i32,
    pub mode: i32,
    pub variant: i32, // the rules: Russian, English or giveaway
    pub sound: bool,
    pub panel: bool,
    pub language: Language,
    pub window: Option<WindowPlacement>,
    pub maximized: bool,
    pub game: String, // see draughts::Game::serialize
}

fn file_path() -> PathBuf {
    platform::config_folder(NAME).join(format!("{NAME}.ini"))
}

impl Settings {
    pub fn load() -> Settings {
        // The lines "Key=Value" of the program's section.
        let text = std::fs::read_to_string(file_path()).unwrap_or_default();
        let mut values = HashMap::new();
        let mut in_section = false;
        for line in text.lines().map(str::trim) {
            if let Some(section) = line.strip_prefix('[') {
                in_section = section.trim_end_matches(']').eq_ignore_ascii_case(NAME);
            } else if let (true, Some((key, value))) = (in_section, line.split_once('=')) {
                values.insert(key.trim(), value.trim());
            }
        }
        let int = |key: &str, fallback: i32| values.get(key).and_then(|value| value.parse().ok()).unwrap_or(fallback);

        let language = match int("Language", -1) {
            0 => Language::English,
            1 => Language::Russian,
            _ if platform::prefers_russian() => Language::Russian,
            _ => Language::English,
        };
        let (width, height) = (int("WindowWidth", 0), int("WindowHeight", 0));
        let has_window = width > 0 && height > 0;
        Settings {
            theme: int("Theme", 0).clamp(0, THEME_COUNT as i32 - 1),
            level: int("Level", 3).clamp(1, 5),
            mode: int("Mode", 0).clamp(0, 3),
            variant: int("Rules", 0).clamp(0, 2),
            sound: int("Sound", 1) != 0,
            panel: int("Panel", 1) != 0,
            language,
            window: has_window.then(|| WindowPlacement { left: int("WindowLeft", 0), top: int("WindowTop", 0), width, height }),
            maximized: has_window && int("Maximized", 0) != 0,
            game: values.get("Game").copied().unwrap_or_default().to_owned(),
        }
    }

    pub fn save(&self) {
        let mut text = format!("[{NAME}]\n");
        let mut line = |key: &str, value: i32| writeln!(text, "{key}={value}").expect("writing to a string does not fail");
        line("Theme", self.theme);
        line("Level", self.level);
        line("Mode", self.mode);
        line("Rules", self.variant);
        line("Sound", i32::from(self.sound));
        line("Panel", i32::from(self.panel));
        line("Language", i32::from(self.language == Language::Russian));
        if let Some(window) = self.window {
            line("WindowLeft", window.left);
            line("WindowTop", window.top);
            line("WindowWidth", window.width);
            line("WindowHeight", window.height);
            line("Maximized", i32::from(self.maximized));
        }
        text += &format!("Game={}\n", self.game);
        // A game that cannot be saved is still played.
        let _ = std::fs::write(file_path(), text);
    }
}
