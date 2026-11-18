use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{CoreError, Paths, fs};

/// Configurazione globale del launcher (`config.toml`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Config {
    /// Memoria massima (`-Xmx`) per le istanze che non la specificano.
    pub default_memory_mb: u32,
    /// Java da usare al posto del runtime Mojang scaricato automaticamente.
    pub java_path: Option<PathBuf>,
    /// Numero massimo di download simultanei.
    pub download_concurrency: usize,
    /// Application (client) ID dell'app Azure usata per il login Microsoft.
    /// Non è un segreto. Sovrascrivibile con `HYPER_MSA_CLIENT_ID`.
    pub msa_client_id: Option<String>,
    /// Colore accento dell'interfaccia (`#rrggbb`).
    pub accent: String,
    /// Riduce a icona la finestra del launcher quando il gioco parte.
    pub minimize_on_launch: bool,
    /// Impostazioni di gioco condivise tra le istanze.
    pub sync: SyncConfig,
    /// Mostra la finestra di console di Java accanto al gioco (Windows).
    pub show_console: bool,
    /// Tema dell'interfaccia: `dark` o `light`.
    pub theme: String,
    /// Lingua dell'interfaccia (`it`, `en`).
    pub language: String,
    /// Configurazione iniziale completata (account, aspetto, lingua).
    pub setup_done: bool,
    /// Aspetto e layout dell'interfaccia.
    pub appearance: AppearanceConfig,
    /// Argomenti JVM per le istanze che non ne hanno di propri.
    pub default_jvm_args: Vec<String>,
    /// Dimensione della finestra di gioco all'avvio (`None` = quella del gioco).
    pub game_width: Option<u32>,
    pub game_height: Option<u32>,
    /// Avvia il gioco a schermo intero.
    pub game_fullscreen: bool,
}

/// Aspetto e layout dell'interfaccia (letti e applicati dalla UI).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct AppearanceConfig {
    /// `solid`, `transparent` (desktop sfocato dietro la finestra) o `image`.
    pub background: String,
    /// Effetto della finestra trasparente: `mica` o `acrylic` (Windows).
    pub effect: String,
    /// Sfocatura dell'immagine di sfondo, in px (0–40).
    pub blur: u32,
    /// Oscuramento dell'immagine di sfondo, in percentuale (0–80).
    pub dim: u32,
    /// Opacità dei pannelli sopra uno sfondo trasparente o immagine (20–100).
    pub panel_opacity: u32,
    /// Scala dell'interfaccia in percentuale (80–130).
    pub ui_scale: u32,
    /// Angoli: `sharp`, `medium`, `round`.
    pub radius: String,
    /// Animazioni: `full`, `reduced`, `off`.
    pub animations: String,
    /// Barra laterale: `left` o `right`.
    pub sidebar_side: String,
    /// Etichette accanto alle icone della barra laterale.
    pub sidebar_labels: bool,
    /// Istanze recenti mostrate nella barra laterale (0–10).
    pub sidebar_instances: u32,
    /// Colonna delle notizie accanto a Home, Libreria e istanze.
    pub show_news: bool,
    /// Contenuti a tutta larghezza invece che centrati.
    pub wide_content: bool,
    /// Widget della Home nell'ordine mostrato; `None` = disposizione
    /// predefinita (diversa da una Home svuotata apposta).
    pub home: Option<Vec<WidgetConfig>>,
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            background: "solid".into(),
            effect: "mica".into(),
            blur: 16,
            dim: 40,
            panel_opacity: 70,
            ui_scale: 100,
            radius: "medium".into(),
            animations: "full".into(),
            sidebar_side: "left".into(),
            sidebar_labels: false,
            sidebar_instances: 5,
            show_news: true,
            wide_content: false,
            home: None,
        }
    }
}

/// Un widget della Home.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WidgetConfig {
    /// Tipo di widget (`greeting`, `quick`, `servers`, …).
    pub kind: String,
    /// Larghezza: `small`, `medium`, `large`.
    pub size: String,
    /// Opzioni numeriche del widget (es. quanti server mostrare).
    #[serde(default)]
    pub options: std::collections::BTreeMap<String, u32>,
}

const BACKGROUNDS: &[&str] = &["solid", "transparent", "image"];
const EFFECTS: &[&str] = &["mica", "acrylic"];
const RADII: &[&str] = &["sharp", "medium", "round"];
const ANIMATIONS: &[&str] = &["full", "reduced", "off"];
const SIDES: &[&str] = &["left", "right"];
const WIDGET_SIZES: &[&str] = &["small", "medium", "large"];
const MAX_WIDGETS: usize = 32;

fn widget_ok(widget: &WidgetConfig) -> bool {
    !widget.kind.is_empty()
        && widget.kind.len() <= 32
        && widget
            .kind
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        && widget.options.len() <= 8
        && WIDGET_SIZES.contains(&widget.size.as_str())
}

impl AppearanceConfig {
    /// Campi testuali con i valori ammessi.
    fn choices(&mut self) -> [(&'static str, &mut String, &'static [&'static str]); 5] {
        [
            ("background", &mut self.background, BACKGROUNDS),
            ("window effect", &mut self.effect, EFFECTS),
            ("corner style", &mut self.radius, RADII),
            ("animation level", &mut self.animations, ANIMATIONS),
            ("sidebar side", &mut self.sidebar_side, SIDES),
        ]
    }

    /// Campi numerici con il loro intervallo.
    fn ranges(&mut self) -> [(&'static str, &mut u32, u32, u32); 5] {
        [
            ("background blur", &mut self.blur, 0, 40),
            ("background dim", &mut self.dim, 0, 80),
            ("panel opacity", &mut self.panel_opacity, 20, 100),
            ("interface scale", &mut self.ui_scale, 80, 130),
            ("sidebar instances", &mut self.sidebar_instances, 0, 10),
        ]
    }

    /// Valori ammessi (arrivati dalla UI o scritti a mano nel file).
    fn validate(&self) -> Result<(), CoreError> {
        let mut copy = self.clone();
        for (field, value, allowed) in copy.choices() {
            if !allowed.contains(&value.as_str()) {
                return Err(CoreError::InvalidConfig(format!(
                    "unknown {field} {value:?}"
                )));
            }
        }
        for (field, value, min, max) in copy.ranges() {
            if !(min..=max).contains(value) {
                return Err(CoreError::InvalidConfig(format!(
                    "{field} must be between {min} and {max}"
                )));
            }
        }
        if let Some(home) = &self.home {
            if home.len() > MAX_WIDGETS {
                return Err(CoreError::InvalidConfig("too many home widgets".into()));
            }
            let mut kinds = std::collections::HashSet::new();
            for widget in home {
                if !widget_ok(widget) || !kinds.insert(widget.kind.as_str()) {
                    return Err(CoreError::InvalidConfig(format!(
                        "invalid or repeated home widget {:?}",
                        widget.kind
                    )));
                }
            }
        }
        Ok(())
    }

    /// Rende validi i valori di un file modificato a mano: numeri riportati
    /// nell'intervallo, scelte sconosciute ai predefiniti, widget non validi
    /// o ripetuti tolti. Così il resto della configurazione non va perso.
    fn repair(&mut self) {
        let defaults = AppearanceConfig::default();
        let fallback = [
            defaults.background,
            defaults.effect,
            defaults.radius,
            defaults.animations,
            defaults.sidebar_side,
        ];
        for ((_, value, allowed), default) in self.choices().into_iter().zip(fallback) {
            if !allowed.contains(&value.as_str()) {
                *value = default;
            }
        }
        for (_, value, min, max) in self.ranges() {
            *value = (*value).clamp(min, max);
        }
        if let Some(home) = &mut self.home {
            let mut kinds = std::collections::HashSet::new();
            home.retain(|w| widget_ok(w) && kinds.insert(w.kind.clone()));
            home.truncate(MAX_WIDGETS);
        }
    }
}

/// Quali impostazioni di `options.txt` condividere tra le istanze.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct SyncConfig {
    /// Spenta finché l'utente non la attiva: cambia le impostazioni di
    /// tutte le istanze.
    pub enabled: bool,
    pub keybinds: bool,
    pub video: bool,
    pub audio: bool,
    pub controls: bool,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            keybinds: true,
            video: true,
            audio: true,
            controls: true,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_memory_mb: 4096,
            java_path: None,
            download_concurrency: 16,
            msa_client_id: None,
            accent: "#6e5ae6".into(),
            minimize_on_launch: true,
            sync: SyncConfig::default(),
            show_console: false,
            theme: "dark".into(),
            language: "it".into(),
            setup_done: false,
            appearance: AppearanceConfig::default(),
            default_jvm_args: Vec::new(),
            game_width: None,
            game_height: None,
            game_fullscreen: false,
        }
    }
}

/// Sotto questa soglia la JVM non riesce nemmeno ad avviare il gioco.
pub const MIN_MEMORY_MB: u32 = 512;

impl Config {
    /// Controlla i valori prima di salvarli (anche se scritti a mano).
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.default_memory_mb < MIN_MEMORY_MB {
            return Err(CoreError::InvalidConfig(format!(
                "default memory must be at least {MIN_MEMORY_MB} MB"
            )));
        }
        let hex = self.accent.strip_prefix('#').unwrap_or_default();
        if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(CoreError::InvalidConfig(format!(
                "accent color {:?} is not #rrggbb",
                self.accent
            )));
        }
        if !Self::THEMES.contains(&self.theme.as_str()) {
            return Err(CoreError::InvalidConfig(format!(
                "unknown theme {:?}",
                self.theme
            )));
        }
        if !Self::LANGUAGES.contains(&self.language.as_str()) {
            return Err(CoreError::InvalidConfig(format!(
                "unknown language {:?}",
                self.language
            )));
        }
        let size_ok = |v: Option<u32>| v.is_none_or(|v| (320..=7680).contains(&v));
        if !size_ok(self.game_width) || !size_ok(self.game_height) {
            return Err(CoreError::InvalidConfig(
                "game window size must be between 320 and 7680 pixels".into(),
            ));
        }
        if self.default_jvm_args.len() > 64 {
            return Err(CoreError::InvalidConfig("too many JVM arguments".into()));
        }
        if !Self::CONCURRENCY.contains(&self.download_concurrency) {
            return Err(CoreError::InvalidConfig(
                "parallel downloads must be between 1 and 64".into(),
            ));
        }
        self.appearance.validate()
    }

    /// Download in parallelo ammessi.
    pub const CONCURRENCY: std::ops::RangeInclusive<usize> = 1..=64;

    /// Rende valido un file scritto a mano (o da una versione più vecchia):
    /// ogni campo fuori regola torna al predefinito o nell'intervallo, gli
    /// altri restano. Senza, ogni salvataggio fallirebbe per sempre.
    fn repair(&mut self) {
        let defaults = Config::default();
        self.default_memory_mb = self.default_memory_mb.max(MIN_MEMORY_MB);
        self.download_concurrency = self
            .download_concurrency
            .clamp(*Self::CONCURRENCY.start(), *Self::CONCURRENCY.end());
        let hex = self.accent.strip_prefix('#').unwrap_or_default();
        if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            self.accent = defaults.accent;
        }
        if !Self::THEMES.contains(&self.theme.as_str()) {
            self.theme = defaults.theme;
        }
        if !Self::LANGUAGES.contains(&self.language.as_str()) {
            self.language = defaults.language;
        }
        let size_ok = |v: Option<u32>| v.is_none_or(|v| (320..=7680).contains(&v));
        if !size_ok(self.game_width) || !size_ok(self.game_height) {
            self.game_width = None;
            self.game_height = None;
        }
        self.default_jvm_args.truncate(64);
        self.appearance.repair();
    }

    /// Temi e lingue che l'interfaccia conosce.
    pub const THEMES: [&str; 2] = ["dark", "light"];
    pub const LANGUAGES: [&str; 2] = ["it", "en"];

    pub const MSA_CLIENT_ID_ENV: &str = "HYPER_MSA_CLIENT_ID";

    /// Client ID Microsoft effettivo: variabile d'ambiente, poi config.
    pub fn effective_msa_client_id(&self) -> Option<String> {
        std::env::var(Self::MSA_CLIENT_ID_ENV)
            .ok()
            .or_else(|| self.msa_client_id.clone())
            .map(|id| id.trim().to_owned())
            .filter(|id| !id.is_empty())
    }

    pub fn load(paths: &Paths) -> Result<Self, CoreError> {
        let mut config: Self = fs::read_toml_or_default(&paths.config_file())?;
        config.repair();
        Ok(config)
    }

    pub fn save(&self, paths: &Paths) -> Result<(), CoreError> {
        self.validate()?;
        fs::write_toml(&paths.config_file(), self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_gives_defaults_and_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        let mut config = Config::load(&paths).unwrap();
        assert_eq!(config, Config::default());

        config.default_memory_mb = 6144;
        config.save(&paths).unwrap();
        assert_eq!(Config::load(&paths).unwrap(), config);
    }

    #[test]
    fn appearance_is_validated() {
        let mut config = Config::default();
        config.validate().unwrap();
        config.appearance.background = "video".into();
        assert!(config.validate().is_err());
        config.appearance = AppearanceConfig::default();
        config.appearance.blur = 99;
        assert!(config.validate().is_err());
        config.appearance = AppearanceConfig::default();
        let widget = |kind: &str, size: &str| WidgetConfig {
            kind: kind.into(),
            size: size.into(),
            options: Default::default(),
        };
        config.appearance.home = Some(vec![widget("servers", "huge")]);
        assert!(config.validate().is_err());
        config.appearance.home = Some(vec![
            widget("servers", "medium"),
            widget("servers", "small"),
        ]);
        assert!(config.validate().is_err(), "repeated widgets");
        config.appearance.home = Some(vec![widget("servers", "medium")]);
        config.validate().unwrap();
        // Una Home svuotata resta vuota (non torna quella predefinita).
        config.appearance.home = Some(Vec::new());
        config.validate().unwrap();
        config.game_width = Some(10);
        assert!(config.validate().is_err());
    }

    #[test]
    fn hand_edited_file_is_repaired_on_load() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        std::fs::write(
            paths.config_file(),
            "download_concurrency = 500
theme = \"neon\"
game_width = 5

[appearance]
uiScale = 0
background = \"video\"
home = [{ kind = \"quick\", size = \"large\" }, { kind = \"quick\", size = \"small\" }, { kind = \"x y\", size = \"large\" }]
",
        )
        .unwrap();
        let config = Config::load(&paths).unwrap();
        config.validate().unwrap();
        assert_eq!(config.download_concurrency, 64);
        assert_eq!(config.theme, "dark");
        assert_eq!(config.game_width, None);
        assert_eq!(config.appearance.ui_scale, 80);
        assert_eq!(config.appearance.background, "solid");
        let home = config.appearance.home.unwrap();
        assert_eq!(home.len(), 1);
        assert_eq!(home[0].size, "large");
    }

    #[test]
    fn partial_file_fills_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        std::fs::write(paths.config_file(), "default_memory_mb = 2048\n").unwrap();
        let config = Config::load(&paths).unwrap();
        assert_eq!(config.default_memory_mb, 2048);
        assert_eq!(config.download_concurrency, 16);
    }
}
