//! Version manifest Mojang (`version_manifest_v2.json`).

use serde::Deserialize;

use crate::Sha1Hex;

pub const VERSION_MANIFEST_URL: &str =
    "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

#[derive(Debug, Clone, Deserialize)]
pub struct VersionManifest {
    pub latest: Latest,
    pub versions: Vec<ManifestVersion>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Latest {
    pub release: String,
    pub snapshot: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestVersion {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: VersionKind,
    pub url: String,
    pub release_time: String,
    pub sha1: Sha1Hex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionKind {
    Release,
    Snapshot,
    OldBeta,
    OldAlpha,
    #[serde(other)]
    Unknown,
}

impl VersionKind {
    /// Valore per `${version_type}`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Release => "release",
            Self::Snapshot => "snapshot",
            Self::OldBeta => "old_beta",
            Self::OldAlpha => "old_alpha",
            Self::Unknown => "unknown",
        }
    }
}

impl VersionManifest {
    pub fn find(&self, id: &str) -> Option<&ManifestVersion> {
        self.versions.iter().find(|v| v.id == id)
    }

    /// Risolve un id o gli alias `latest` / `latest-snapshot`.
    pub fn resolve(&self, id_or_alias: &str) -> Option<&ManifestVersion> {
        match id_or_alias {
            "latest" | "latest-release" => self.find(&self.latest.release),
            "latest-snapshot" => self.find(&self.latest.snapshot),
            id => self.find(id),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
        "latest": {"release": "1.21.4", "snapshot": "25w02a"},
        "versions": [
            {"id": "25w02a", "type": "snapshot",
             "url": "https://piston-meta.mojang.com/v1/packages/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/25w02a.json",
             "time": "2025-01-08T12:00:00+00:00", "releaseTime": "2025-01-08T12:00:00+00:00",
             "sha1": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "complianceLevel": 1},
            {"id": "1.21.4", "type": "release",
             "url": "https://piston-meta.mojang.com/v1/packages/bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb/1.21.4.json",
             "time": "2024-12-03T10:00:00+00:00", "releaseTime": "2024-12-03T10:00:00+00:00",
             "sha1": "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB", "complianceLevel": 1},
            {"id": "a1.0.4", "type": "old_alpha",
             "url": "https://example.invalid/a1.0.4.json",
             "time": "2010-07-09T00:00:00+00:00", "releaseTime": "2010-07-09T00:00:00+00:00",
             "sha1": "cccccccccccccccccccccccccccccccccccccccc", "complianceLevel": 0},
            {"id": "future", "type": "pending",
             "url": "https://example.invalid/future.json",
             "time": "2030-01-01T00:00:00+00:00", "releaseTime": "2030-01-01T00:00:00+00:00",
             "sha1": "dddddddddddddddddddddddddddddddddddddddd", "complianceLevel": 1}
        ]
    }"#;

    #[test]
    fn parses_manifest_and_resolves_aliases() {
        let manifest: VersionManifest = serde_json::from_str(SAMPLE).unwrap();
        assert_eq!(manifest.versions.len(), 4);
        assert_eq!(manifest.resolve("latest").unwrap().id, "1.21.4");
        assert_eq!(manifest.resolve("latest-snapshot").unwrap().id, "25w02a");
        assert_eq!(
            manifest.resolve("a1.0.4").unwrap().kind,
            VersionKind::OldAlpha
        );
        assert_eq!(manifest.find("future").unwrap().kind, VersionKind::Unknown);
        assert!(manifest.resolve("0.0.0").is_none());
        // sha1 normalizzato in minuscolo
        assert_eq!(
            manifest.find("1.21.4").unwrap().sha1.as_str(),
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
        );
    }

    #[test]
    fn rejects_malformed_sha1() {
        let bad = SAMPLE.replace(
            "cccccccccccccccccccccccccccccccccccccccc",
            "../../not-a-hash",
        );
        assert!(serde_json::from_str::<VersionManifest>(&bad).is_err());
    }
}
