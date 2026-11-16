# Hyper Launcher

Launcher di Minecraft moderno e performante, scritto in Rust.

> **Stato:** fase 1 — core headless. Oggi c'è una CLI (`hyper`) che scarica,
> verifica e avvia Minecraft vanilla con account offline. Login Microsoft, UI
> (Tauri + Svelte), installer di mod e sync delle impostazioni arriveranno
> nelle fasi successive (vedi [CLAUDE.md](CLAUDE.md)).

## Installazione (Windows)

Scarica `HyperLauncher-Setup.exe` dall'[ultima release](../../releases/latest)
e avvialo. Installa `hyper` per l'utente corrente (nessun permesso di
amministratore) e lo aggiunge al PATH. Il setup non è firmato: se Windows
SmartScreen lo blocca, scegli "Ulteriori informazioni" → "Esegui comunque".

Ogni push su `main` ricompila il setup e aggiorna la release della versione
corrente.

## Uso

```text
hyper versions                                 # versioni disponibili
hyper account add-offline <nome>               # account offline
hyper instance create <nome> --version latest  # istanza isolata
hyper launch <nome>                            # installa (se serve) e avvia
hyper launch <nome> --dry-run                  # mostra il comando senza avviare
```

Java non serve: il launcher scarica il runtime ufficiale Mojang adatto alla
versione. I dati stanno in `%APPDATA%\HyperLauncher` (sovrascrivibile con
`HYPER_LAUNCHER_HOME`).

## Sviluppo

```text
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p hyper-cli -- versions
```

| Crate | Ruolo |
|---|---|
| `hyper-core` | percorsi, configurazione, filesystem sicuro (anti path traversal) |
| `hyper-auth` | account (per ora offline) |
| `hyper-installer` | version manifest, download paralleli verificati SHA1, asset, native, runtime Java |
| `hyper-instances` | istanze isolate, comando di avvio, pipeline di lancio |
| `hyper-sync` | parser di `options.txt` (scheletro) |
| `hyper-cli` | binario `hyper` |
