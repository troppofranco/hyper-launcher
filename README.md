# Hyper Launcher

Launcher di Minecraft moderno e performante, scritto in Rust.

> **Stato:** app grafica (Tauri + Svelte) e CLI (`hyper`) che installano e
> avviano Minecraft vanilla o con **Fabric, Quilt, NeoForge o Forge**, con account offline o
> Microsoft, istanze isolate, aggiornamenti automatici e il catalogo
> **Modrinth** (mod, resource pack, shader, data pack, modpack `.mrpack`).
> Tasti e impostazioni grafiche si possono sincronizzare tra le istanze.
> CurseForge arriverà in una fase successiva (vedi [CLAUDE.md](CLAUDE.md)).

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

L'app: Home con "Gioca subito", modpack in evidenza, i server salvati (con
giocatori online) e il tempo di gioco della settimana; **Personalizzazione** con
anteprima 3D delle skin (per gli account offline la skin si vede in gioco
tramite un resource pack generato dal launcher); immagine personalizzata per
ogni istanza; Libreria, **Esplora**
(catalogo Modrinth con installazione delle dipendenze in un clic), pagina
dell'istanza (Gioca, mod, resource pack e shader con aggiornamenti, mondi,
console, impostazioni, esporta `.mrpack`), account offline o Microsoft dal
menu in alto a sinistra, Impostazioni (con la **sincronizzazione** di tasti,
video, audio e controlli tra le istanze).

NeoForge e Forge si installano eseguendo il loro installer ufficiale
(scaricato dal Maven ufficiale e verificato con SHA1) al primo avvio
dell'istanza; Fabric e Quilt usano i profili delle loro API.

La CLI:

```text
hyper versions                                         # versioni disponibili
hyper account add-offline <nome>                       # account offline
hyper account add-microsoft                            # account Microsoft (vedi sotto)
hyper instance create <nome> --version latest          # istanza vanilla
hyper instance create <nome> --version 26.3 --loader fabric   # o quilt, neoforge, forge
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
| `hyper-installer` | version manifest, Fabric/Quilt/NeoForge/Forge, download paralleli verificati SHA1, asset, native, runtime Java, aggiornamenti |
| `hyper-instances` | istanze isolate, mod/resource pack/shader e mondi, comando di avvio, pipeline di lancio, console |
| `hyper-content` | catalogo Modrinth: ricerca, installazione con dipendenze, aggiornamenti, modpack `.mrpack` |
| `hyper-sync` | sincronizzazione di `options.txt` tra le istanze (tasti, video, audio, controlli) |
| `hyper-cli` | binario `hyper` |
| `hyper-app` + `ui/` | app grafica `hyper-launcher` (Tauri 2 + Svelte 5) |
