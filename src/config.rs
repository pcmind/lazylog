use ratatui::style::Color;
use regex::Regex;
use serde::Deserialize;
use std::path::PathBuf;
use std::str::FromStr;
use std::fs;

#[derive(Debug, Deserialize, Clone)]
pub struct HighlighterConfig {
    pub pattern: String,
    #[serde(default)]
    pub is_regex: bool,
    pub fg: Option<String>,
    pub bg: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TransformerConfig {
    pub pattern: String,
    #[serde(default)]
    pub is_regex: bool,
    pub command: String,
}

#[derive(Debug, Clone)]
pub struct Highlighter {
    pub regex: Option<Regex>,
    pub substring: Option<String>,
    pub fg: Option<Color>,
    pub bg: Option<Color>,
}

#[derive(Debug, Clone)]
pub struct Transformer {
    pub regex: Option<Regex>,
    pub substring: Option<String>,
    pub command: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct ConfigData {
    #[serde(default)]
    pub highlighter: Vec<HighlighterConfig>,
    #[serde(default)]
    pub transformer: Vec<TransformerConfig>,
}

#[derive(Debug, Default, Clone)]
pub struct Config {
    pub highlighters: Vec<Highlighter>,
    pub transformers: Vec<Transformer>,
}

impl Config {
    pub fn load() -> Self {
        let config_dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        let config_path = config_dir.join("lazylog").join("config.toml");

        if !config_path.exists() {
            // Create default file
            if let Some(parent) = config_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let default_toml = r#"[[highlighter]]
pattern = "ERROR"
is_regex = false
fg = "Red"

[[highlighter]]
pattern = "WARN"
is_regex = false
fg = "Yellow"

# Example transformer: pretty-print JSON when viewing line details (Space)
# [[transformer]]
# pattern = "\\{.*\\}"
# is_regex = true
# command = "jq ."
"#;
            let _ = fs::write(&config_path, default_toml);
        }

        let Ok(contents) = fs::read_to_string(&config_path) else {
            return Config::default();
        };

        let Ok(data) = toml::from_str::<ConfigData>(&contents) else {
            return Config::default();
        };

        let mut highlighters = Vec::new();
        for h in data.highlighter {
            let mut regex = None;
            let mut substring = None;

            if h.is_regex {
                regex = Regex::new(&h.pattern).ok();
            } else {
                substring = Some(h.pattern.clone());
            }

            let fg = h.fg.as_deref().and_then(|c| Color::from_str(c).ok());
            let bg = h.bg.as_deref().and_then(|c| Color::from_str(c).ok());

            highlighters.push(Highlighter {
                regex,
                substring,
                fg,
                bg,
            });
        }

        let mut transformers = Vec::new();
        for t in data.transformer {
            let mut regex = None;
            let mut substring = None;

            if t.is_regex {
                regex = Regex::new(&t.pattern).ok();
            } else {
                substring = Some(t.pattern.clone());
            }

            transformers.push(Transformer {
                regex,
                substring,
                command: t.command,
            });
        }

        Config { 
            highlighters,
            transformers,
        }
    }
}
