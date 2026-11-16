# Hyper Launcher

Launcher di Minecraft moderno e performante, scritto in Rust.

> **Stato:** core headless. Oggi c'è una CLI (`hyper`) che scarica, verifica e
> avvia Minecraft vanilla con account offline o Microsoft. UI (Tauri +
> Svelte), mod loader, installer di mod e sync delle impostazioni arriveranno
> nelle fasi successive (vedi [CLAUDE.md](CLAUDE.md)).

## Installazione (Windows)

Scarica `HyperLauncher-Setup.exe` dall'[ultima release](../../releases/latest)
e avvialo. Installa `hyper` per l'utente corrente (nessun permesso di
amministratore) e lo aggiunge al PATH. Il setup non è firmato: se Windows
SmartScreen lo blocca, scegli "Ulteriori informazioni" → "Esegui comunque".

**Aggiornamenti automatici:** ogni push su `main` pubblica una nuova release
`build-<N>`. A ogni avvio `hyper` controlla l'ultima release e, se è più
nuova, scarica il setup, ne verifica lo SHA1 e lo installa da solo; poi basta
rieseguire il comando. `hyper update` forza il controllo, `HYPER_NO_UPDATE=1`
lo disattiva.

## Uso

```text
hyper versions                                 # versioni disponibili
hyper account add-offline <nome>               # account offline
hyper account add-microsoft                    # account Microsoft (vedi sotto)
hyper instance create <nome> --version latest  # istanza isolata
hyper launch <nome>                            # installa (se serve) e avvia
hyper launch <nome> --dry-run                  # mostra il comando senza avviare
```

Java non serve: il launcher scarica il runtime ufficiale Mojang adatto alla
versione. I dati stanno in `%APPDATA%\HyperLauncher` (sovrascrivibile con
`HYPER_LAUNCHER_HOME`).

**Account Microsoft:** serve il client ID di un'app Azure ("Personal Microsoft
accounts only", "Allow public client flows" = Yes) approvata da Mojang per la
Minecraft API, da impostare come `msa_client_id` in `config.toml` o in
`HYPER_MSA_CLIENT_ID`. I token restano solo nel keyring di sistema.

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
