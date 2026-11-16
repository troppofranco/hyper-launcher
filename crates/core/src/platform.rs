//! Informazioni sulla piattaforma corrente nel vocabolario usato da Mojang.

/// Nome del sistema operativo come compare nelle `rules` delle version JSON.
pub fn mojang_os() -> &'static str {
    match std::env::consts::OS {
        "windows" => "windows",
        "macos" => "osx",
        _ => "linux",
    }
}

/// Architettura come compare nelle `rules` (`"arch": "x86"`).
pub fn mojang_arch() -> &'static str {
    match std::env::consts::ARCH {
        "aarch64" => "arm64",
        other => other,
    }
}

/// Valore di `${arch}` nei classifier delle native legacy (`natives-windows-${arch}`).
pub fn arch_bits() -> &'static str {
    if cfg!(target_pointer_width = "64") {
        "64"
    } else {
        "32"
    }
}

/// Separatore del classpath Java.
pub fn classpath_separator() -> &'static str {
    if cfg!(windows) { ";" } else { ":" }
}
