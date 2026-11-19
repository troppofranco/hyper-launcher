//! "Avvia con il computer": una voce in
//! `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` che all'accesso a
//! Windows lancia l'host dei server senza finestra. Solo per l'utente
//! corrente, niente permessi di amministratore. Su altri sistemi non fa nulla.

use crate::config::ServerManager;

/// Nome della voce (sovrascrivibile per i test, così non si tocca quella vera).
pub fn value_name() -> String {
    std::env::var("HYPER_AUTOSTART_NAME").unwrap_or_else(|_| "HyperLauncherServers".to_owned())
}

#[cfg(windows)]
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

/// Registra `command` per l'avvio all'accesso.
#[cfg(windows)]
pub fn set(command: &str) -> std::io::Result<()> {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;
    let (key, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(RUN_KEY)?;
    key.set_value(value_name(), &command)
}

/// Toglie la voce (se non c'è, va bene così).
#[cfg(windows)]
pub fn remove() -> std::io::Result<()> {
    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, KEY_SET_VALUE};
    let key = match RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(RUN_KEY, KEY_SET_VALUE)
    {
        Ok(key) => key,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    match key.delete_value(value_name()) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// Comando attualmente registrato, se c'è.
#[cfg(windows)]
pub fn current() -> Option<String> {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(RUN_KEY)
        .ok()?
        .get_value(value_name())
        .ok()
}

#[cfg(not(windows))]
pub fn set(_command: &str) -> std::io::Result<()> {
    Ok(())
}

#[cfg(not(windows))]
pub fn remove() -> std::io::Result<()> {
    Ok(())
}

#[cfg(not(windows))]
pub fn current() -> Option<String> {
    None
}

/// Allinea la voce ai server: c'è solo se almeno uno ha "Avvia con il
/// computer" (e "resta acceso").
pub fn sync(manager: &ServerManager, command: &str) -> std::io::Result<()> {
    if manager.list().iter().any(|c| c.autostart && c.keep_running) {
        set(command)
    } else {
        remove()
    }
}
