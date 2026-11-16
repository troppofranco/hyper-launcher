//! Valori segreti (token): mai stampabili, azzerati in memoria al drop.

use std::fmt;

use serde::Deserialize;
use zeroize::Zeroize;

/// Un token o altro segreto. `Debug` e `Display` mostrano `<redacted>` e il
/// tipo non implementa `Serialize`, quindi non può finire per sbaglio in log,
/// file TOML o messaggi d'errore. Il contenuto si legge solo con
/// [`Secret::expose`], da usare nel punto esatto in cui serve.
#[derive(Clone, PartialEq, Eq, Deserialize)]
#[serde(transparent)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

impl fmt::Display for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn never_printed() {
        #[derive(Debug, Deserialize)]
        #[allow(dead_code)]
        struct Response {
            access_token: Secret,
        }
        let response: Response =
            serde_json::from_str(r#"{"access_token": "eyJ-super-secret"}"#).unwrap();
        assert_eq!(response.access_token.expose(), "eyJ-super-secret");
        let shown = format!("{response:?} {}", response.access_token);
        assert!(!shown.contains("super-secret"), "{shown}");
        assert!(shown.contains("<redacted>"));
    }
}
