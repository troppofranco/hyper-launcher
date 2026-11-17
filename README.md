# Hyper Launcher

Launcher di Minecraft moderno e performante, scritto in Rust.

> **Stato:** app grafica (Tauri + Svelte) e CLI (`hyper`) che installano e
> avviano Minecraft vanilla o con **Fabric**, con account offline o
> Microsoft, istanze isolate, aggiornamenti automatici e il catalogo
> **Modrinth** (mod, resource pack, shader, data pack, modpack `.mrpack`).
> CurseForge, Quilt/Forge/NeoForge e la sync delle impostazioni arriveranno
> nelle fasi successive (vedi [CLAUDE.md](CLAUDE.md)).

## Installazione (Windows)

Scarica `HyperLauncher-Setup.exe` dall'[ultima release](../../releases/latest)
e avvialo. Installa per l'utente corrente (nessun permesso di amministratore)
l'app **Hyper Launcher** (menu Start e, se vuoi, desktop) e la CLI `hyper`,
che finisce nel PATH. Il setup non è firmato: se Windows SmartScreen lo
blocca, scegli "Ulteriori informazioni" → "Esegui comunque".

**Aggiornamenti automatici:** ogni push su `main` pubblica una nuova release
`build-<N>`. L'app mostra "Aggiorna e riavvia" quando ce n'è una più nuova;
la CLI controlla a ogni avvio e si aggiorna da sola (`hyper update` forza il
controllo, `HYPER_NO_UPDATE=1` lo disattiva). Il setup scaricato viene sempre
verificato con SHA1.

## Uso

L'app: Home con le istanze recenti e il tempo di gioco, Libreria, **Esplora**
(catalogo Modrinth con installazione delle dipendenze in un clic), pagina
dell'istanza (Gioca, mod, resource pack e shader con aggiornamenti, mondi,
console, impostazioni, esporta `.mrpack`), account offline o Microsoft dal
menu in alto a sinistra, Impostazioni.

La CLI:

```text
hyper versions                                         # versioni disponibili
hyper account add-offline <nome>                       # account offline
hyper account add-microsoft                            # account Microsoft (vedi sotto)
hyper instance create <nome> --version latest          # istanza vanilla
hyper instance create <nome> --version 26.3 --loader fabric
hyper launch <nome>                                    # installa (se serve) e avvia
hyper launch <nome> --dry-run                          # mostra il comando senza avviare
hyper content search sodium --instance <nome>         # cerca su Modrinth (alias: hyper mod)
hyper content add <nome> iris --kind mod               # installa con le dipendenze
hyper content update <nome>                            # aggiorna le mod
hyper pack install fabulously-optimized                # modpack Modrinth → nuova istanza
hyper pack import file.mrpack / export <nome> file.mrpack
```

Java non serve: il launcher scarica il runtime ufficiale Mojang adatto alla
versione. I dati stanno in `%APPDATA%\HyperLauncher` (sovrascrivibile con
`HYPER_LAUNCHER_HOME`).

**Account Microsoft:** serve il client ID di un'app Azure ("Personal Microsoft
accounts only", "Allow public client flows" = Yes) approvata da Mojang per la
Minecraft API, da impostare nelle Impostazioni dell'app, come `msa_client_id`
in `config.toml` o in `HYPER_MSA_CLIENT_ID`. I token restano solo nel keyring
di sistema.

## Sviluppo

```text
npm --prefix ui ci && npm --prefix ui run build   # interfaccia (incorporata nell'app)
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p hyper-app                            # app grafica
cargo run -p hyper-cli -- versions                # CLI
npm --prefix ui run dev                           # UI nel browser con backend simulato
```

| Crate / cartella | Ruolo |
|---|---|
| `hyper-core` | percorsi, configurazione, filesystem sicuro (anti path traversal) |
| `hyper-auth` | account offline e Microsoft, token nel keyring |
| `hyper-installer` | version manifest, Fabric, download paralleli verificati SHA1, asset, native, runtime Java, aggiornamenti |
| `hyper-instances` | istanze isolate, mod/resource pack/shader e mondi, comando di avvio, pipeline di lancio, console |
| `hyper-content` | catalogo Modrinth: ricerca, installazione con dipendenze, aggiornamenti, modpack `.mrpack` |
| `hyper-sync` | parser di `options.txt` (scheletro) |
| `hyper-cli` | binario `hyper` |
| `hyper-app` + `ui/` | app grafica `hyper-launcher` (Tauri 2 + Svelte 5) |
