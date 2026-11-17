//! Sincronizzazione delle impostazioni di gioco tra istanze.
//!
//! Il launcher tiene un `options.txt` condiviso (`sync/options.txt` nella
//! cartella dati). Prima di avviare un'istanza ne copia le categorie scelte
//! (tasti, video, audio, controlli) nell'`options.txt` dell'istanza; quando
//! il gioco si chiude riporta indietro le stesse categorie. Così cambiare un
//! tasto in un'istanza lo cambia in tutte. Il resto del file (lingua, server
//! recenti, resource pack attivi…) resta di ogni istanza.

mod options;

use std::path::Path;

use hyper_core::{CoreError, SyncConfig, fs};

pub use options::OptionsFile;

/// Gruppo di impostazioni sincronizzabile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Keybinds,
    Video,
    Audio,
    Controls,
}

/// Impostazioni video (le chiavi di `options.txt` di Java Edition).
const VIDEO: &[&str] = &[
    "ao",
    "biomeBlendRadius",
    "bobView",
    "chunkSectionFadeInTime",
    "cloudRange",
    "cutoutLeaves",
    "darknessEffectScale",
    "enableVsync",
    "entityDistanceScaling",
    "entityShadows",
    "fov",
    "fovEffectScale",
    "fullscreen",
    "gamma",
    "glintSpeed",
    "glintStrength",
    "graphicsMode",
    "graphicsPreset",
    "guiScale",
    "improvedTransparency",
    "inactivityFpsLimit",
    "maxAnisotropyBit",
    "maxFps",
    "mipmapLevels",
    "particles",
    "prioritizeChunkUpdates",
    "renderClouds",
    "renderDistance",
    "screenEffectScale",
    "simulationDistance",
    "textureFiltering",
    "vignette",
    "weatherRadius",
];

/// Mouse, movimento e "toggle".
const CONTROLS: &[&str] = &[
    "autoJump",
    "discrete_mouse_scroll",
    "invertXMouse",
    "invertYMouse",
    "mouseSensitivity",
    "mouseWheelSensitivity",
    "rawMouseInput",
    "sprintWindow",
    "toggleAttack",
    "toggleCrouch",
    "toggleSprint",
    "toggleUse",
    "touchscreen",
];

/// Categoria di una chiave di `options.txt` (`None` = resta dell'istanza).
pub fn category_of(key: &str) -> Option<Category> {
    if key.starts_with("key_") {
        Some(Category::Keybinds)
    } else if key.starts_with("soundCategory_")
        || matches!(key, "directionalAudio" | "showSubtitles")
    {
        Some(Category::Audio)
    } else if VIDEO.contains(&key) {
        Some(Category::Video)
    } else if CONTROLS.contains(&key) {
        Some(Category::Controls)
    } else {
        None
    }
}

fn wanted(config: &SyncConfig, key: &str) -> bool {
    match category_of(key) {
        Some(Category::Keybinds) => config.keybinds,
        Some(Category::Video) => config.video,
        Some(Category::Audio) => config.audio,
        Some(Category::Controls) => config.controls,
        None => false,
    }
}

/// Copia in `target` le chiavi di `source` delle categorie attive;
/// restituisce quante sono cambiate.
pub fn merge(source: &OptionsFile, target: &mut OptionsFile, config: &SyncConfig) -> usize {
    let mut changed = 0;
    for (key, value) in source.entries().filter(|(k, _)| wanted(config, k)) {
        if target.get(key) != Some(value) {
            target.set(key, value);
            changed += 1;
        }
    }
    changed
}

fn read(path: &Path) -> Result<Option<OptionsFile>, CoreError> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(OptionsFile::parse(&text))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(CoreError::io(path, e)),
    }
}

/// Chiave con la "data version" del gioco che ha scritto il file: dice a
/// Minecraft se deve convertire le impostazioni da un formato vecchio.
const VERSION_KEY: &str = "version";

fn data_version(options: &OptionsFile) -> u32 {
    options
        .get(VERSION_KEY)
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0)
}

/// `true` se un'istanza di questa versione di gioco può partecipare. Prima
/// della 1.13 i tasti erano salvati come numeri invece che per nome: i due
/// formati non si possono mescolare.
pub fn supports_game(game: &str) -> bool {
    let mut parts = game.split(['.', '-']);
    match parts.next().map(str::parse::<u32>) {
        Some(Ok(1)) => parts
            .next()
            .and_then(|minor| minor.parse::<u32>().ok())
            .is_some_and(|minor| minor >= 13),
        // Numerazione annuale (26.x) e relativi snapshot.
        Some(Ok(_)) => true,
        // Vecchi snapshot ("17w43a") o id sconosciuti: meglio non toccarli.
        _ => false,
    }
}

/// Copia le impostazioni condivise in `options.txt` di un'istanza (prima di
/// avviarla) e restituisce il file risultante, da passare a [`push`] a fine
/// partita. Se l'istanza non ha ancora un `options.txt` lo crea con le
/// impostazioni condivise e la loro `version`: senza, Minecraft le
/// considererebbe di un formato antico e le "convertirebbe" rovinandole.
pub fn pull(
    shared: &Path,
    options: &Path,
    config: &SyncConfig,
) -> Result<Option<OptionsFile>, CoreError> {
    let current = read(options)?;
    let Some(source) = read(shared)? else {
        return Ok(current);
    };
    let mut target = match current {
        Some(target) => target,
        None => {
            let Some(version) = source.get(VERSION_KEY) else {
                return Ok(None);
            };
            let mut fresh = OptionsFile::default();
            fresh.set(VERSION_KEY, version);
            fresh
        }
    };
    if merge(&source, &mut target, config) > 0 || !options.exists() {
        fs::write_atomic(options, target.to_string().as_bytes())?;
    }
    Ok(Some(target))
}

/// Riporta nel file condiviso le impostazioni che l'utente ha cambiato
/// durante la partita, cioè quelle diverse da `baseline` (il file com'era
/// all'avvio). Così due istanze aperte insieme non si annullano a vicenda
/// le modifiche: chi chiude per ultimo non ricopia i valori vecchi.
pub fn push(
    options: &Path,
    shared: &Path,
    config: &SyncConfig,
    baseline: Option<&OptionsFile>,
) -> Result<usize, CoreError> {
    let Some(source) = read(options)? else {
        return Ok(0);
    };
    let mut target = read(shared)?.unwrap_or_default();
    let mut changed = 0;
    for (key, value) in source.entries().filter(|(k, _)| wanted(config, k)) {
        let edited = baseline.is_none_or(|b| b.get(key) != Some(value));
        if edited && target.get(key) != Some(value) {
            target.set(key, value);
            changed += 1;
        }
    }
    // La versione condivisa è quella del gioco più recente che l'ha scritta.
    if data_version(&source) > data_version(&target) {
        target.set(VERSION_KEY, data_version(&source).to_string());
        changed += 1;
    }
    if changed > 0 {
        // `write_atomic` crea anche la cartella `sync/` se manca.
        fs::write_atomic(shared, target.to_string().as_bytes())?;
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all() -> SyncConfig {
        SyncConfig {
            enabled: true,
            ..SyncConfig::default()
        }
    }

    #[test]
    fn classifies_keys() {
        assert_eq!(category_of("key_key.jump"), Some(Category::Keybinds));
        assert_eq!(category_of("soundCategory_music"), Some(Category::Audio));
        assert_eq!(category_of("renderDistance"), Some(Category::Video));
        assert_eq!(category_of("mouseSensitivity"), Some(Category::Controls));
        for own in [
            "version",
            "lang",
            "resourcePacks",
            "lastServer",
            "tutorialStep",
        ] {
            assert_eq!(category_of(own), None, "{own}");
        }
    }

    #[test]
    fn merges_only_enabled_categories() {
        let source = OptionsFile::parse(
            "key_key.jump:key.keyboard.j\nrenderDistance:8\nsoundCategory_music:0.0\nlang:it_it\n",
        );
        let mut target = OptionsFile::parse("version:5023\nrenderDistance:16\nlang:en_us\n");
        let config = SyncConfig {
            video: false,
            ..all()
        };
        assert_eq!(merge(&source, &mut target, &config), 2);
        assert_eq!(target.get("key_key.jump"), Some("key.keyboard.j"));
        assert_eq!(target.get("soundCategory_music"), Some("0.0"));
        assert_eq!(target.get("renderDistance"), Some("16"), "video is off");
        assert_eq!(target.get("lang"), Some("en_us"), "never synced");
        assert_eq!(target.get("version"), Some("5023"));
    }

    #[test]
    fn push_then_pull_moves_settings_between_instances() {
        let dir = tempfile::tempdir().unwrap();
        let shared = dir.path().join("sync/options.txt");
        let a = dir.path().join("a.txt");
        let b = dir.path().join("b.txt");
        std::fs::write(
            &a,
            "version:5023\nkey_key.drop:key.keyboard.x\nlang:it_it\n",
        )
        .unwrap();

        assert!(
            pull(&shared, &b, &all()).unwrap().is_none(),
            "nothing shared yet"
        );
        assert!(!b.exists());
        // Senza baseline (prima partita) si condivide tutto, versione compresa.
        assert_eq!(push(&a, &shared, &all(), None).unwrap(), 2);
        let pulled = pull(&shared, &b, &all()).unwrap().unwrap();
        // Il file nuovo porta la versione: Minecraft non lo "converte".
        let b_text = std::fs::read_to_string(&b).unwrap();
        assert_eq!(b_text, "version:5023\nkey_key.drop:key.keyboard.x\n");
        assert_eq!(pulled.get("key_key.drop"), Some("key.keyboard.x"));
    }

    #[test]
    fn last_closed_instance_does_not_undo_other_changes() {
        let dir = tempfile::tempdir().unwrap();
        let shared = dir.path().join("sync/options.txt");
        let a = dir.path().join("a.txt");
        let b = dir.path().join("b.txt");
        std::fs::create_dir_all(shared.parent().unwrap()).unwrap();
        std::fs::write(&shared, "version:5023\nkey_key.drop:key.keyboard.q\n").unwrap();
        std::fs::write(&a, "version:5023\n").unwrap();
        std::fs::write(&b, "version:5023\n").unwrap();
        // A e B partono insieme.
        let base_a = pull(&shared, &a, &all()).unwrap();
        let base_b = pull(&shared, &b, &all()).unwrap();
        // In B l'utente cambia il tasto e chiude per primo.
        std::fs::write(&b, "version:5023\nkey_key.drop:key.keyboard.x\n").unwrap();
        assert_eq!(push(&b, &shared, &all(), base_b.as_ref()).unwrap(), 1);
        // A chiude dopo senza aver toccato nulla: non deve rimettere "q".
        assert_eq!(push(&a, &shared, &all(), base_a.as_ref()).unwrap(), 0);
        let shared_text = std::fs::read_to_string(&shared).unwrap();
        assert!(
            shared_text.contains("key_key.drop:key.keyboard.x"),
            "{shared_text}"
        );
    }

    #[test]
    fn only_modern_versions_take_part() {
        for game in ["1.13", "1.20.1", "26.3", "26.4-snapshot-8"] {
            assert!(supports_game(game), "{game}");
        }
        for game in ["1.12.2", "1.8.9", "17w43a", "b1.7.3"] {
            assert!(!supports_game(game), "{game}");
        }
    }
}
