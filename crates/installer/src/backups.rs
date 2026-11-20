//! Backup a zip di cartelle (mondi di server e istanze), condivisi da
//! `hyper-server` e `hyper-instances`.
//!
//! Ogni backup è `<cartella backup>/<secondi unix>-<auto|manual>.zip` e
//! contiene le cartelle salvate come prime voci del percorso (es. `world/…`,
//! `world_nether/…`). Il ripristino estrae in una cartella temporanea (ogni
//! voce passa da `safe_join`) e solo alla fine scambia le cartelle: quella
//! sostituita resta come `<nome>.prima-del-ripristino`.

use std::io::Write;
use std::path::{Path, PathBuf};

use hyper_core::{fs, safe_component};
use serde::{Deserialize, Serialize};
use zip::write::SimpleFileOptions;

use crate::{InstallerError, Result};

/// Suffisso della cartella sostituita da un ripristino (resta finché non ne
/// arriva un altro).
pub const PREVIOUS_SUFFIX: &str = ".prima-del-ripristino";
/// File che Minecraft tiene aperto: non si legge e non serve nei backup.
const SKIP: &[&str] = &["session.lock"];
/// Cartella temporanea del ripristino, dentro la destinazione.
const RESTORE_TMP: &str = ".hyper-ripristino-in-corso";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    /// Nome del file (es. `1795100000-auto.zip`).
    pub file: String,
    /// Secondi Unix della creazione.
    pub created: u64,
    pub size_bytes: u64,
    /// Fatto dagli automatismi (altrimenti a mano).
    pub auto: bool,
}

fn zip_err(path: &Path) -> impl Fn(zip::result::ZipError) -> InstallerError + '_ {
    move |source| InstallerError::Zip {
        path: path.to_owned(),
        source,
    }
}

fn parse_name(file: &str) -> Option<(u64, bool)> {
    let stem = file.strip_suffix(".zip")?;
    let (secs, kind) = stem.split_once('-')?;
    let auto = match kind {
        "auto" => true,
        "manual" => false,
        _ => return None,
    };
    Some((secs.parse().ok()?, auto))
}

/// Backup presenti in `dir`, dal più recente.
pub fn list(dir: &Path) -> Vec<BackupInfo> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut list: Vec<BackupInfo> = entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter_map(|e| {
            let file = e.file_name().into_string().ok()?;
            let (created, auto) = parse_name(&file)?;
            Some(BackupInfo {
                size_bytes: e.metadata().ok()?.len(),
                file,
                created,
                auto,
            })
        })
        .collect();
    list.sort_by(|a, b| b.created.cmp(&a.created).then(b.file.cmp(&a.file)));
    list
}

/// Il file di un backup in `dir` (solo nomi validi, niente percorsi).
fn backup_file(dir: &Path, file: &str) -> Result<PathBuf> {
    let name = safe_component(file)?;
    if parse_name(name).is_none() {
        return Err(hyper_core::CoreError::UnsafePath(name.to_owned()).into());
    }
    Ok(dir.join(name))
}

/// Zippa le cartelle `sources` (`(cartella, nome nello zip)`) in un nuovo
/// backup in `dir`; quelle che non esistono si saltano. Scrittura atomica.
pub fn create(dir: &Path, sources: &[(PathBuf, String)], auto: bool) -> Result<BackupInfo> {
    fs::create_dir_all(dir)?;
    let mut created = hyper_core::unix_now();
    // Due backup nello stesso secondo non si sovrascrivono.
    let kind = if auto { "auto" } else { "manual" };
    while dir.join(format!("{created}-{kind}.zip")).exists() {
        created += 1;
    }
    let file = format!("{created}-{kind}.zip");
    let out = dir.join(&file);
    let tmp = fs::sibling_with_suffix(&out, ".part");
    let result = (|| -> Result<()> {
        let handle = std::fs::File::create(&tmp).map_err(|e| hyper_core::CoreError::io(&tmp, e))?;
        let mut zip = zip::ZipWriter::new(std::io::BufWriter::new(handle));
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .large_file(true);
        for (src, prefix) in sources {
            safe_component(prefix)?;
            if !src.is_dir() {
                continue;
            }
            let mut files = Vec::new();
            walk(src, prefix, &mut files)?;
            for (name, path) in files {
                zip.start_file(&name, options).map_err(zip_err(&tmp))?;
                let mut reader =
                    std::fs::File::open(&path).map_err(|e| hyper_core::CoreError::io(&path, e))?;
                std::io::copy(&mut reader, &mut zip)
                    .map_err(|e| hyper_core::CoreError::io(&path, e))?;
            }
        }
        let mut inner = zip.finish().map_err(zip_err(&tmp))?;
        inner
            .flush()
            .map_err(|e| hyper_core::CoreError::io(&tmp, e))?;
        Ok(())
    })();
    if let Err(e) = result {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    fs::rename_replacing(&tmp, &out)?;
    let size_bytes = std::fs::metadata(&out).map_or(0, |m| m.len());
    Ok(BackupInfo {
        file,
        created,
        size_bytes,
        auto,
    })
}

/// File da salvare sotto `dir` (percorsi nello zip con `/`), senza seguire
/// i collegamenti.
fn walk(dir: &Path, prefix: &str, out: &mut Vec<(String, PathBuf)>) -> Result<()> {
    let entries = std::fs::read_dir(dir).map_err(|e| hyper_core::CoreError::io(dir, e))?;
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        let path = entry.path();
        let zip_name = format!("{prefix}/{name}");
        if kind.is_dir() {
            walk(&path, &zip_name, out)?;
        } else if kind.is_file() && !SKIP.contains(&name.as_str()) {
            out.push((zip_name, path));
        }
    }
    Ok(())
}

/// Tiene solo i `keep` backup più recenti in `dir`.
pub fn prune(dir: &Path, keep: usize) -> Result<usize> {
    let mut removed = 0;
    for old in list(dir).into_iter().skip(keep.max(1)) {
        std::fs::remove_file(dir.join(&old.file)).map_err(|e| hyper_core::CoreError::io(dir, e))?;
        removed += 1;
    }
    Ok(removed)
}

pub fn delete(dir: &Path, file: &str) -> Result<()> {
    let path = backup_file(dir, file)?;
    std::fs::remove_file(&path).map_err(|e| hyper_core::CoreError::io(&path, e))?;
    Ok(())
}

/// Ripristina il backup `file` dentro `dest`: solo le cartelle `allowed`
/// (es. i mondi del server). Ognuna sostituisce quella attuale, che resta
/// come `<nome>.prima-del-ripristino`. Restituisce le cartelle ripristinate.
pub fn restore(dir: &Path, file: &str, dest: &Path, allowed: &[String]) -> Result<Vec<String>> {
    let path = backup_file(dir, file)?;
    let tmp = dest.join(RESTORE_TMP);
    // Una cartella con lo stesso nome di quella temporanea non si ripristina.
    let allowed: Vec<String> = allowed
        .iter()
        .filter(|a| *a != RESTORE_TMP)
        .cloned()
        .collect();
    fs::remove_dir_all_if_exists(&tmp)?;
    let reader = std::fs::File::open(&path).map_err(|e| hyper_core::CoreError::io(&path, e))?;
    // Solo le voci sotto una cartella ammessa; ogni percorso passa da
    // `safe_join` dentro `extract_zip_mapped`.
    let extracted = crate::natives::extract_zip_mapped(reader, &path, &tmp, |name| {
        let top = name.split('/').next()?;
        allowed.iter().any(|a| a == top).then_some(name)
    });
    if let Err(e) = extracted {
        let _ = fs::remove_dir_all_if_exists(&tmp);
        return Err(e);
    }
    let mut restored = Vec::new();
    for name in &allowed {
        let from = tmp.join(name);
        if !from.is_dir() {
            continue;
        }
        let current = dest.join(name);
        let previous = dest.join(format!("{name}{PREVIOUS_SUFFIX}"));
        if current.exists() {
            fs::remove_dir_all_if_exists(&previous)?;
            std::fs::rename(&current, &previous)
                .map_err(|e| hyper_core::CoreError::io(&current, e))?;
        }
        std::fs::rename(&from, &current).map_err(|e| hyper_core::CoreError::io(&from, e))?;
        restored.push(name.clone());
    }
    fs::remove_dir_all_if_exists(&tmp)?;
    Ok(restored)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world(root: &Path, name: &str, content: &str) {
        std::fs::create_dir_all(root.join(name).join("region")).unwrap();
        std::fs::write(root.join(name).join("level.dat"), content).unwrap();
        std::fs::write(root.join(name).join("region").join("r.0.0.mca"), "chunk").unwrap();
        std::fs::write(root.join(name).join("session.lock"), "lock").unwrap();
    }

    #[test]
    fn backup_restore_and_prune() {
        let tmp = tempfile::tempdir().unwrap();
        let server = tmp.path().join("srv");
        let backups = tmp.path().join("backups");
        world(&server, "world", "v1");
        let sources = vec![
            (server.join("world"), "world".to_owned()),
            (server.join("world_nether"), "world_nether".to_owned()),
        ];
        let first = create(&backups, &sources, false).unwrap();
        assert!(!first.auto && first.size_bytes > 0);

        // Il mondo cambia, poi si torna al backup.
        std::fs::write(server.join("world").join("level.dat"), "v2").unwrap();
        let allowed = vec!["world".to_owned(), "world_nether".to_owned()];
        let restored = restore(&backups, &first.file, &server, &allowed).unwrap();
        assert_eq!(restored, ["world"]);
        assert_eq!(
            std::fs::read_to_string(server.join("world/level.dat")).unwrap(),
            "v1"
        );
        assert_eq!(
            std::fs::read_to_string(server.join(format!("world{PREVIOUS_SUFFIX}/level.dat")))
                .unwrap(),
            "v2"
        );
        assert!(
            !server.join("world/session.lock").exists(),
            "lock not saved"
        );
        assert!(!server.join(RESTORE_TMP).exists());

        for _ in 0..3 {
            create(&backups, &sources, true).unwrap();
        }
        assert_eq!(list(&backups).len(), 4);
        assert_eq!(prune(&backups, 2).unwrap(), 2);
        assert_eq!(list(&backups).len(), 2);
    }

    #[test]
    fn refuses_bad_names_and_evil_zips() {
        let tmp = tempfile::tempdir().unwrap();
        let backups = tmp.path().join("backups");
        std::fs::create_dir_all(&backups).unwrap();
        assert!(delete(&backups, "../x-manual.zip").is_err());
        assert!(delete(&backups, "notes.txt").is_err());

        // Uno zip con `../` non esce dalla cartella di destinazione.
        let evil = backups.join("1-manual.zip");
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&evil).unwrap());
        zip.start_file("world/../../evil.txt", SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"x").unwrap();
        zip.finish().unwrap();
        let dest = tmp.path().join("srv");
        std::fs::create_dir_all(&dest).unwrap();
        assert!(restore(&backups, "1-manual.zip", &dest, &["world".to_owned()]).is_err());
        assert!(!tmp.path().join("evil.txt").exists());
    }
}
