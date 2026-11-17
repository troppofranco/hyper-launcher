//! Estrazione delle native libraries con protezione da path traversal.

use std::io::{Read, Seek};
use std::path::Path;

use hyper_core::{CoreError, fs, safe_join};

use crate::{InstallerError, Result};

/// Estrae un jar di native in `dest`, saltando `META-INF/` e i prefissi in
/// `exclude`. Restituisce il numero di file estratti.
pub fn extract_natives(jar: &Path, dest: &Path, exclude: &[String]) -> Result<usize> {
    let file = std::fs::File::open(jar).map_err(|e| CoreError::io(jar, e))?;
    extract_zip(file, jar, dest, exclude)
}

/// Estrae un archivio zip (`origin` serve solo nei messaggi d'errore).
/// Ogni nome di entry è trattato come non fidato e passato a [`safe_join`]:
/// un'entry come `../x`, `C:\x` o `/x` fa fallire l'estrazione prima di
/// scrivere quel file.
pub fn extract_zip<R: Read + Seek>(
    reader: R,
    origin: &Path,
    dest: &Path,
    exclude: &[String],
) -> Result<usize> {
    extract_zip_mapped(reader, origin, dest, |name| {
        let skip =
            name.starts_with("META-INF/") || exclude.iter().any(|ex| name.starts_with(ex.as_str()));
        (!skip).then_some(name)
    })
}

/// Come [`extract_zip`], ma `map` decide per ogni entry (nome con `/`) se
/// estrarla e con quale percorso relativo (`None` = salta). Il percorso
/// restituito passa comunque da [`safe_join`].
pub fn extract_zip_mapped<R: Read + Seek>(
    reader: R,
    origin: &Path,
    dest: &Path,
    map: impl Fn(&str) -> Option<&str>,
) -> Result<usize> {
    let zip_err = |source| InstallerError::Zip {
        path: origin.to_owned(),
        source,
    };
    let mut archive = zip::ZipArchive::new(reader).map_err(zip_err)?;
    fs::create_dir_all(dest)?;

    let mut extracted = 0;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(zip_err)?;
        let name = entry.name().map_err(zip_err)?.replace('\\', "/");
        let Some(relative) = map(&name).filter(|r| !r.is_empty()) else {
            continue;
        };

        let out = safe_join(dest, relative)?;
        if entry.is_dir() {
            fs::create_dir_all(&out)?;
            continue;
        }
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = std::fs::File::create(&out).map_err(|e| CoreError::io(&out, e))?;
        std::io::copy(&mut entry, &mut file).map_err(|e| CoreError::io(&out, e))?;
        extracted += 1;
    }
    Ok(extracted)
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};

    use zip::write::SimpleFileOptions;

    use super::*;

    fn zip_with(entries: &[(&str, &[u8])]) -> Cursor<Vec<u8>> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, data) in entries {
            writer
                .start_file(*name, SimpleFileOptions::default())
                .unwrap();
            writer.write_all(data).unwrap();
        }
        let mut cursor = writer.finish().unwrap();
        cursor.set_position(0);
        cursor
    }

    #[test]
    fn extracts_valid_entries_and_skips_excluded() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("natives");
        let archive = zip_with(&[
            ("lwjgl.dll", b"dll"),
            ("windows/x64/org/lwjgl/glfw.dll", b"glfw"),
            ("META-INF/MANIFEST.MF", b"manifest"),
            ("skip/me.txt", b"x"),
        ]);
        let n = extract_zip(archive, Path::new("test.jar"), &dest, &["skip/".to_owned()]).unwrap();
        assert_eq!(n, 2);
        assert_eq!(std::fs::read(dest.join("lwjgl.dll")).unwrap(), b"dll");
        assert!(dest.join("windows/x64/org/lwjgl/glfw.dll").is_file());
        assert!(!dest.join("META-INF").exists());
        assert!(!dest.join("skip").exists());
    }

    #[test]
    fn rejects_path_traversal_entries() {
        for evil in [
            "../evil.dll",
            "..\\evil.dll",
            "a/../../evil.dll",
            "/abs/evil.dll",
            "C:\\evil.dll",
            "C:/evil.dll",
            "\\\\server\\share\\evil.dll",
        ] {
            let dir = tempfile::tempdir().unwrap();
            let dest = dir.path().join("natives");
            let archive = zip_with(&[(evil, b"pwned")]);
            let result = extract_zip(archive, Path::new("test.jar"), &dest, &[]);
            assert!(
                matches!(result, Err(InstallerError::Core(CoreError::UnsafePath(_)))),
                "entry {evil:?} should be rejected, got {result:?}"
            );
            assert!(!dir.path().join("evil.dll").exists());
            // Nessun file scritto da nessuna parte nella cartella temporanea.
            let written = walk(dir.path());
            assert!(written.is_empty(), "{evil:?} wrote {written:?}");
        }
    }

    fn walk(dir: &Path) -> Vec<std::path::PathBuf> {
        let mut files = Vec::new();
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                files.extend(walk(&path));
            } else {
                files.push(path);
            }
        }
        files
    }
}
