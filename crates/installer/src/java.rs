//! Runtime Java ufficiali Mojang (gli stessi usati dal launcher vanilla).

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::version::Download;
use crate::{DownloadTask, InstallerError, Result};

pub const JAVA_RUNTIME_INDEX_URL: &str = "https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";

/// Componente usato quando una version JSON molto vecchia non indica `javaVersion`.
pub const LEGACY_COMPONENT: &str = "jre-legacy";

/// piattaforma → componente → release disponibili.
pub type RuntimeIndex = HashMap<String, HashMap<String, Vec<RuntimeRelease>>>;

#[derive(Debug, Clone, Deserialize)]
pub struct RuntimeRelease {
    pub manifest: Download,
    pub version: RuntimeVersion,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RuntimeVersion {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RuntimeManifest {
    pub files: BTreeMap<String, RuntimeFile>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum RuntimeFile {
    Directory,
    File {
        #[serde(default)]
        executable: bool,
        downloads: RuntimeFileDownloads,
    },
    Link {
        target: String,
    },
}

#[derive(Debug, Clone, Deserialize)]
pub struct RuntimeFileDownloads {
    pub raw: Download,
}

/// File di un runtime da scaricare, con l'elenco di quelli eseguibili.
#[derive(Debug, Clone, Default)]
pub struct RuntimeFiles {
    pub tasks: Vec<DownloadTask>,
    pub executables: Vec<PathBuf>,
}

/// Chiave della piattaforma corrente nell'indice dei runtime Mojang.
pub fn current_platform() -> Option<&'static str> {
    Some(match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => "windows-x64",
        ("windows", "x86") => "windows-x86",
        ("windows", "aarch64") => "windows-arm64",
        ("linux", "x86_64") => "linux",
        ("linux", "x86") => "linux-i386",
        ("macos", "x86_64") => "mac-os",
        ("macos", "aarch64") => "mac-os-arm64",
        _ => return None,
    })
}

/// Eseguibile `java` dentro la cartella di un runtime Mojang installato.
pub fn java_executable(runtime_dir: &Path) -> PathBuf {
    runtime_dir.join(if cfg!(windows) {
        "bin/java.exe"
    } else if cfg!(target_os = "macos") {
        "jre.bundle/Contents/Home/bin/java"
    } else {
        "bin/java"
    })
}

/// Release di un componente per la piattaforma indicata.
pub fn find_release<'a>(
    index: &'a RuntimeIndex,
    platform: &str,
    component: &str,
) -> Result<&'a RuntimeRelease> {
    index
        .get(platform)
        .and_then(|components| components.get(component))
        .and_then(|releases| releases.first())
        .ok_or_else(|| InstallerError::JavaRuntimeUnavailable {
            component: component.to_owned(),
            platform: platform.to_owned(),
        })
}

impl RuntimeManifest {
    /// File da scaricare, con percorsi validati rispetto a `dest`. I link
    /// simbolici (presenti solo su Linux/macOS) non sono ancora gestiti.
    pub fn files(&self, dest: &Path) -> Result<RuntimeFiles> {
        let mut files = RuntimeFiles::default();
        for (path, file) in &self.files {
            match file {
                RuntimeFile::File {
                    executable,
                    downloads,
                } => {
                    let task = downloads.raw.task_in(dest, path)?;
                    if *executable {
                        files.executables.push(task.path.clone());
                    }
                    files.tasks.push(task);
                }
                RuntimeFile::Directory => {}
                RuntimeFile::Link { target } => {
                    tracing::debug!(%path, %target, "skipping symlink in Java runtime");
                }
            }
        }
        Ok(files)
    }
}

/// Imposta il bit di esecuzione (no-op su Windows).
pub fn mark_executable(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(path)
            .map_err(|e| hyper_core::CoreError::io(path, e))?
            .permissions();
        perms.set_mode(perms.mode() | 0o755);
        std::fs::set_permissions(path, perms).map_err(|e| hyper_core::CoreError::io(path, e))?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_runtime_manifest_and_rejects_traversal() {
        let manifest: RuntimeManifest = serde_json::from_str(
            r#"{"files": {
                "bin": {"type": "directory"},
                "bin/java.exe": {"type": "file", "executable": true, "downloads": {
                    "raw": {"sha1": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "size": 1, "url": "https://example.invalid/java.exe"}}},
                "legal/LICENSE": {"type": "link", "target": "../x"}
            }}"#,
        )
        .unwrap();
        let files = manifest.files(Path::new("rt")).unwrap();
        let java = Path::new("rt").join("bin").join("java.exe");
        assert_eq!(files.tasks.len(), 1);
        assert_eq!(files.tasks[0].path, java);
        assert_eq!(files.executables, [java]);

        let evil: RuntimeManifest = serde_json::from_str(
            r#"{"files": {"../../evil.exe": {"type": "file", "downloads": {
                "raw": {"sha1": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "size": 1, "url": "https://example.invalid/e"}}}}}"#,
        )
        .unwrap();
        assert!(evil.files(Path::new("rt")).is_err());
    }

    #[test]
    fn finds_release_for_platform() {
        let index: RuntimeIndex = serde_json::from_str(
            r#"{"windows-x64": {"java-runtime-delta": [{"manifest":
                {"sha1": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "size": 1, "url": "https://example.invalid/m.json"},
                "version": {"name": "21.0.7", "released": "2025-01-01T00:00:00+00:00"}}],
               "jre-legacy": []}}"#,
        )
        .unwrap();
        assert_eq!(
            find_release(&index, "windows-x64", "java-runtime-delta")
                .unwrap()
                .version
                .name,
            "21.0.7"
        );
        assert!(find_release(&index, "windows-x64", "jre-legacy").is_err());
        assert!(find_release(&index, "linux", "java-runtime-delta").is_err());
    }
}
