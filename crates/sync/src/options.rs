use std::fmt;

/// Contenuto di `options.txt`: righe `chiave:valore` in ordine. Le righe che
/// non seguono il formato sono conservate così come sono, per un round-trip
/// senza perdite.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OptionsFile {
    lines: Vec<Line>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Line {
    Entry { key: String, value: String },
    Raw(String),
}

impl OptionsFile {
    pub fn parse(text: &str) -> Self {
        let lines = text
            .lines()
            .map(|line| match line.split_once(':') {
                Some((key, value)) if !key.is_empty() => Line::Entry {
                    key: key.to_owned(),
                    value: value.to_owned(),
                },
                _ => Line::Raw(line.to_owned()),
            })
            .collect();
        Self { lines }
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries().find(|(k, _)| *k == key).map(|(_, v)| v)
    }

    /// Imposta una chiave, aggiungendola in fondo se manca.
    pub fn set(&mut self, key: &str, value: impl Into<String>) {
        let value = value.into();
        for line in &mut self.lines {
            if let Line::Entry { key: k, value: v } = line
                && k == key
            {
                *v = value;
                return;
            }
        }
        self.lines.push(Line::Entry {
            key: key.to_owned(),
            value,
        });
    }

    pub fn entries(&self) -> impl Iterator<Item = (&str, &str)> {
        self.lines.iter().filter_map(|line| match line {
            Line::Entry { key, value } => Some((key.as_str(), value.as_str())),
            Line::Raw(_) => None,
        })
    }

    /// Solo i keybind (`key_*`).
    pub fn keybinds(&self) -> impl Iterator<Item = (&str, &str)> {
        self.entries().filter(|(k, _)| k.starts_with("key_"))
    }
}

impl fmt::Display for OptionsFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for line in &self.lines {
            match line {
                Line::Entry { key, value } => writeln!(f, "{key}:{value}")?,
                Line::Raw(raw) => writeln!(f, "{raw}")?,
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "version:4189\nfov:0.0\nkey_key.attack:key.mouse.left\nresourcePacks:[\"vanilla\"]\nweird line\nkey_key.jump:key.keyboard.space\n";

    #[test]
    fn round_trip_is_lossless() {
        assert_eq!(OptionsFile::parse(SAMPLE).to_string(), SAMPLE);
    }

    #[test]
    fn get_set_and_keybinds() {
        let mut options = OptionsFile::parse(SAMPLE);
        assert_eq!(options.get("fov"), Some("0.0"));
        assert_eq!(options.get("key_key.attack"), Some("key.mouse.left"));
        assert_eq!(options.keybinds().count(), 2);

        options.set("fov", "0.5");
        options.set("renderDistance", "16");
        assert_eq!(options.get("fov"), Some("0.5"));
        assert!(options.to_string().ends_with("renderDistance:16\n"));
    }
}
