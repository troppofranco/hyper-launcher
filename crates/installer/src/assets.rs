//! Asset index e oggetti (cache condivisa `assets/objects/ab/<hash>`).

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

use crate::{DownloadTask, Sha1Hex};

pub const RESOURCES_URL: &str = "https://resources.download.minecraft.net";

#[derive(Debug, Clone, Deserialize)]
pub struct AssetIndex {
    pub objects: HashMap<String, AssetObject>,
    #[serde(default, rename = "virtual")]
    pub is_virtual: bool,
    #[serde(default)]
    pub map_to_resources: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetObject {
    /// Già validato come 40 caratteri hex: sicuro da usare come percorso.
    pub hash: Sha1Hex,
    pub size: u64,
}

impl AssetIndex {
    /// Un task per ogni oggetto (i duplicati vengono rimossi dal downloader).
    pub fn object_tasks(&self, objects_dir: &Path) -> Vec<DownloadTask> {
        self.objects
            .values()
            .map(|object| {
                let hash = object.hash.as_str();
                let prefix = &hash[..2];
                DownloadTask::new(
                    format!("{RESOURCES_URL}/{prefix}/{hash}"),
                    objects_dir.join(prefix).join(hash),
                    object.hash.clone(),
                    Some(object.size),
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_object_tasks() {
        let index: AssetIndex = serde_json::from_str(
            r#"{"objects": {
                "minecraft/sounds/a.ogg": {"hash": "0123456789abcdef0123456789abcdef01234567", "size": 3},
                "icons/icon.png": {"hash": "fedcba9876543210fedcba9876543210fedcba98", "size": 4}
            }}"#,
        )
        .unwrap();
        let tasks = index.object_tasks(Path::new("objects"));
        let task = tasks.iter().find(|t| t.size == Some(3)).unwrap();
        assert_eq!(
            task.url,
            "https://resources.download.minecraft.net/01/0123456789abcdef0123456789abcdef01234567"
        );
        assert_eq!(
            task.path,
            Path::new("objects")
                .join("01")
                .join("0123456789abcdef0123456789abcdef01234567")
        );
    }

    #[test]
    fn rejects_hash_that_is_a_path() {
        let result = serde_json::from_str::<AssetIndex>(
            r#"{"objects": {"x": {"hash": "../../../../../../../../../../windows/x", "size": 1}}}"#,
        );
        assert!(result.is_err());
    }
}
