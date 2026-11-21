//! Scambio delle cartelle tra installazioni, con diario.
//!
//! Stato in `<dati launcher>/bedrock/state.json`: quale installazione occupa
//! le cartelle del gioco e l'eventuale scambio in corso. Ogni passo è un
//! `rename` (atomico) e il diario annota quanti passi sono finiti; dopo
//! un'interruzione [`recover`] riparte dal primo non annotato. Se il crash
//! arriva tra un rename e la sua annotazione, ripetere quel passo non fa
//! nulla: la sorgente non c'è più.

use std::path::{Path, PathBuf};

use hyper_core::fs;
use serde::{Deserialize, Serialize};

use crate::{BedrockError, DataDirs, FOLDERS, Result};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct State {
    /// Nome dell'installazione i cui dati sono nelle cartelle del gioco.
    pub active: Option<String>,
    /// Scambio iniziato e non ancora finito.
    #[serde(default)]
    pub pending: Option<Pending>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Pending {
    pub from: Option<String>,
    pub to: String,
    /// `<istanza uscente>/bedrock` (dove vanno i dati del gioco).
    pub from_storage: Option<PathBuf>,
    /// `<istanza entrante>/bedrock` (da dove arrivano).
    pub to_storage: PathBuf,
    /// Passi già fatti: per ogni cartella di [`FOLDERS`] due passi, prima
    /// fuori i dati dell'uscente, poi dentro quelli dell'entrante.
    #[serde(default)]
    pub done: usize,
}

fn state_file(launcher_root: &Path) -> PathBuf {
    launcher_root.join("bedrock").join("state.json")
}

impl State {
    pub fn load(launcher_root: &Path) -> Result<Self> {
        let path = state_file(launcher_root);
        match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|e| BedrockError::Invalid(format!("{}: {e}", path.display()))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(hyper_core::CoreError::io(&path, e).into()),
        }
    }

    fn save(&self, launcher_root: &Path) -> Result<()> {
        let json = serde_json::to_vec_pretty(self).expect("lo stato si serializza sempre");
        fs::write_atomic(&state_file(launcher_root), &json)?;
        Ok(())
    }
}

/// Segna `name` come installazione attiva senza spostare nulla: serve alla
/// prima installazione Bedrock, che "adotta" ciò che c'è già nel gioco, e
/// quando l'attiva viene rinominata.
pub fn set_active(launcher_root: &Path, name: Option<&str>) -> Result<()> {
    let mut state = State::load(launcher_root)?;
    state.active = name.map(str::to_owned);
    state.save(launcher_root)
}

/// Porta i dati di `to` nelle cartelle del gioco, rimettendo quelli
/// dell'attuale attiva in `from_storage`. Il gioco deve essere chiuso.
pub fn swap(
    launcher_root: &Path,
    dirs: &DataDirs,
    from_storage: Option<PathBuf>,
    to: &str,
    to_storage: PathBuf,
) -> Result<()> {
    let mut state = State::load(launcher_root)?;
    if state.pending.is_some() {
        recover(launcher_root, dirs)?;
        state = State::load(launcher_root)?;
    }
    if state.active.as_deref() == Some(to) {
        return Ok(());
    }
    if crate::running() {
        return Err(BedrockError::GameRunning);
    }
    // Dati del profilo Xbox da sistemare ma nessuna cartella del profilo:
    // fermarsi prima di toccare qualunque cosa.
    if dirs.user.is_none()
        && FOLDERS.iter().any(|(scope, name)| {
            *scope == crate::Scope::User && stored(&to_storage, *scope, name).exists()
        })
    {
        return Err(BedrockError::NoProfile);
    }
    state.pending = Some(Pending {
        from: state.active.clone(),
        to: to.to_owned(),
        from_storage,
        to_storage,
        done: 0,
    });
    state.save(launcher_root)?;
    run(launcher_root, dirs, state)
}

/// Completa uno scambio interrotto (da chiamare all'avvio del launcher).
pub fn recover(launcher_root: &Path, dirs: &DataDirs) -> Result<()> {
    let state = State::load(launcher_root)?;
    if state.pending.is_none() {
        return Ok(());
    }
    run(launcher_root, dirs, state)
}

fn stored(storage: &Path, scope: crate::Scope, name: &str) -> PathBuf {
    storage.join(scope.dir_name()).join(name)
}

fn run(launcher_root: &Path, dirs: &DataDirs, mut state: State) -> Result<()> {
    let mut pending = state
        .pending
        .clone()
        .expect("chiamato solo con uno scambio in corso");
    let steps = FOLDERS.iter().flat_map(|f| [(f, true), (f, false)]);
    for (step, ((scope, name), outgoing)) in steps.enumerate().skip(pending.done) {
        if let Some(game) = dirs.game_folder(*scope, name) {
            if outgoing {
                // Fuori i dati dell'uscente. Senza installazione attiva il
                // contenuto del gioco non è di nessuno e non si tocca: se
                // c'è, l'entrante non può prendere il suo posto (`Conflict`).
                if let Some(storage) = &pending.from_storage {
                    move_dir(&game, &stored(storage, *scope, name))?;
                }
            } else {
                // Dentro quelli dell'entrante.
                move_dir(&stored(&pending.to_storage, *scope, name), &game)?;
            }
        }
        pending.done = step + 1;
        state.pending = Some(pending.clone());
        state.save(launcher_root)?;
    }
    state.active = Some(pending.to);
    state.pending = None;
    state.save(launcher_root)
}

/// Sposta una cartella (o un file); non fa nulla se `src` non c'è (mai
/// esistito o già spostato). Una cartella di destinazione vuota si
/// sostituisce; una piena, o un file già presente, ferma tutto.
fn move_dir(src: &Path, dst: &Path) -> Result<()> {
    if !src.exists() {
        return Ok(());
    }
    if dst.is_file() {
        return Err(BedrockError::Conflict(dst.to_owned()));
    }
    if dst.exists() {
        let empty = std::fs::read_dir(dst)
            .map_err(|e| hyper_core::CoreError::io(dst, e))?
            .next()
            .is_none();
        if !empty {
            return Err(BedrockError::Conflict(dst.to_owned()));
        }
        std::fs::remove_dir(dst).map_err(|e| hyper_core::CoreError::io(dst, e))?;
    }
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    std::fs::rename(src, dst).map_err(|e| {
        // ERROR_NOT_SAME_DEVICE: il rename tra dischi non è possibile.
        if e.raw_os_error() == Some(17) {
            BedrockError::CrossDevice
        } else {
            hyper_core::CoreError::io(src, e).into()
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Scope;

    struct Setup {
        _tmp: tempfile::TempDir,
        root: PathBuf,
        dirs: DataDirs,
        a: PathBuf,
        b: PathBuf,
    }

    fn setup() -> Setup {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("launcher");
        let dirs = DataDirs {
            shared: tmp.path().join("game/Shared/com.mojang"),
            user: Some(tmp.path().join("game/123/com.mojang")),
        };
        let a = root.join("instances/A/bedrock");
        let b = root.join("instances/B/bedrock");
        Setup {
            root,
            dirs,
            a,
            b,
            _tmp: tmp,
        }
    }

    fn write(path: PathBuf, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn read(path: PathBuf) -> String {
        std::fs::read_to_string(path).unwrap()
    }

    #[test]
    fn swaps_folders_back_and_forth() {
        let s = setup();
        let game_world = |d: &DataDirs| {
            d.user
                .clone()
                .unwrap()
                .join("minecraftWorlds/w1/levelname.txt")
        };
        // A adotta il contenuto del gioco.
        write(game_world(&s.dirs), "mondo di A");
        write(
            s.dirs.shared.join("resource_packs/p/manifest.json"),
            "pack di A",
        );
        set_active(&s.root, Some("A")).unwrap();
        // B ha già un suo mondo salvato.
        write(
            s.b.join("user/minecraftWorlds/w1/levelname.txt"),
            "mondo di B",
        );

        swap(&s.root, &s.dirs, Some(s.a.clone()), "B", s.b.clone()).unwrap();
        assert_eq!(read(game_world(&s.dirs)), "mondo di B");
        assert!(!s.dirs.shared.join("resource_packs").exists());
        assert_eq!(
            read(s.a.join("user/minecraftWorlds/w1/levelname.txt")),
            "mondo di A"
        );
        assert_eq!(
            read(s.a.join("shared/resource_packs/p/manifest.json")),
            "pack di A"
        );
        assert_eq!(State::load(&s.root).unwrap().active.as_deref(), Some("B"));

        swap(&s.root, &s.dirs, Some(s.b.clone()), "A", s.a.clone()).unwrap();
        assert_eq!(read(game_world(&s.dirs)), "mondo di A");
        assert_eq!(
            read(s.dirs.shared.join("resource_packs/p/manifest.json")),
            "pack di A"
        );
        assert_eq!(
            read(s.b.join("user/minecraftWorlds/w1/levelname.txt")),
            "mondo di B"
        );
    }

    #[test]
    fn interrupted_swap_is_completed() {
        let s = setup();
        write(
            s.dirs.user.clone().unwrap().join("minecraftpe/options.txt"),
            "A",
        );
        write(s.dirs.shared.join("behavior_packs/x/manifest.json"), "A");
        // File interno del gioco: non appartiene a nessuna installazione.
        write(
            s.dirs
                .user
                .clone()
                .unwrap()
                .join("minecraftpe/clientId.txt"),
            "id",
        );
        set_active(&s.root, Some("A")).unwrap();
        write(s.b.join("user/minecraftpe/options.txt"), "B");
        // Simula un crash dopo il primo passo: diario scritto, una sola
        // cartella spostata.
        let state = State {
            active: Some("A".into()),
            pending: Some(Pending {
                from: Some("A".into()),
                to: "B".into(),
                from_storage: Some(s.a.clone()),
                to_storage: s.b.clone(),
                done: 0,
            }),
        };
        state.save(&s.root).unwrap();
        move_dir(
            &s.dirs.game_folder(Scope::Shared, "behavior_packs").unwrap(),
            &s.a.join("shared/behavior_packs"),
        )
        .unwrap();

        recover(&s.root, &s.dirs).unwrap();
        let state = State::load(&s.root).unwrap();
        assert_eq!(state.active.as_deref(), Some("B"));
        assert!(state.pending.is_none());
        assert_eq!(
            read(s.dirs.user.clone().unwrap().join("minecraftpe/options.txt")),
            "B"
        );
        assert_eq!(read(s.a.join("user/minecraftpe/options.txt")), "A");
        assert_eq!(read(s.a.join("shared/behavior_packs/x/manifest.json")), "A");
        assert_eq!(
            read(
                s.dirs
                    .user
                    .clone()
                    .unwrap()
                    .join("minecraftpe/clientId.txt")
            ),
            "id"
        );
        assert!(!s.a.join("user/minecraftpe/clientId.txt").exists());
    }

    #[test]
    fn completed_steps_are_not_repeated() {
        // Crash dopo aver scambiato del tutto la prima cartella
        // (resource_packs), con il diario aggiornato (2) o no (1): riprendere
        // non deve rimandare indietro i dati di B già entrati nel gioco.
        for done in [1, 2] {
            let s = setup();
            let game_rp = s.dirs.game_folder(Scope::Shared, "resource_packs").unwrap();
            write(s.a.join("shared/resource_packs/pa/manifest.json"), "A");
            write(game_rp.join("pb/manifest.json"), "B");
            write(
                s.dirs
                    .user
                    .clone()
                    .unwrap()
                    .join("minecraftWorlds/w/level.dat"),
                "A",
            );
            write(s.b.join("user/minecraftWorlds/w/level.dat"), "B");
            State {
                active: Some("A".into()),
                pending: Some(Pending {
                    from: Some("A".into()),
                    to: "B".into(),
                    from_storage: Some(s.a.clone()),
                    to_storage: s.b.clone(),
                    done,
                }),
            }
            .save(&s.root)
            .unwrap();

            recover(&s.root, &s.dirs).unwrap();
            assert_eq!(read(game_rp.join("pb/manifest.json")), "B", "done={done}");
            assert_eq!(
                read(s.a.join("shared/resource_packs/pa/manifest.json")),
                "A"
            );
            assert!(
                !s.a.join("shared/resource_packs/pb").exists(),
                "done={done}"
            );
            let world = s
                .dirs
                .user
                .clone()
                .unwrap()
                .join("minecraftWorlds/w/level.dat");
            assert_eq!(read(world), "B");
            assert_eq!(read(s.a.join("user/minecraftWorlds/w/level.dat")), "A");
            assert_eq!(State::load(&s.root).unwrap().active.as_deref(), Some("B"));
        }
    }

    #[test]
    fn never_overwrites_data() {
        let s = setup();
        write(s.dirs.shared.join("skin_packs/s/manifest.json"), "gioco");
        set_active(&s.root, Some("A")).unwrap();
        // Dati rimasti per errore nella cartella di A: lo scambio si ferma.
        write(s.a.join("shared/skin_packs/old/manifest.json"), "vecchio");
        let err = swap(&s.root, &s.dirs, Some(s.a.clone()), "B", s.b.clone()).unwrap_err();
        assert!(matches!(err, BedrockError::Conflict(_)), "{err}");
        assert_eq!(
            read(s.dirs.shared.join("skin_packs/s/manifest.json")),
            "gioco"
        );
        assert_eq!(
            read(s.a.join("shared/skin_packs/old/manifest.json")),
            "vecchio"
        );
    }

    #[test]
    fn needs_a_profile_folder_for_profile_data() {
        let mut s = setup();
        s.dirs.user = None;
        set_active(&s.root, Some("A")).unwrap();
        write(s.b.join("user/minecraftWorlds/w/levelname.txt"), "B");
        let err = swap(&s.root, &s.dirs, Some(s.a.clone()), "B", s.b.clone()).unwrap_err();
        assert!(matches!(err, BedrockError::NoProfile), "{err}");
        assert!(State::load(&s.root).unwrap().pending.is_none());
    }
}
