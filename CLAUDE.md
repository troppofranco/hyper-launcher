# Hyper Launcher

Launcher di Minecraft moderno e performante, scritto prevalentemente in Rust.

## Obiettivi
- Estetica pulita e moderna, altamente personalizzabile (temi, layout)
- Più installazioni/istanze isolate, con cache condivisa di librerie e assets
- Installer integrato di mod, resource pack, data pack, shader (Modrinth + CurseForge, import/export `.mrpack`)
- Sync di keybinds e impostazioni grafiche tra profili (parsing di `options.txt`)
- Più account (Microsoft OAuth → Xbox → XSTS → Minecraft services, più account offline)
- Loader: Fabric, Quilt, NeoForge, Forge; gestione automatica Java (Adoptium/Azul)

## Stack (proposto)
- UI: Tauri 2 + frontend Svelte/SolidJS (da confermare; alternative: Slint, iced)
- Core: workspace Cargo con crate separati (`core`, `auth`, `installer`, `instances`, `sync`, UI)
- Librerie: tokio, reqwest, serde/serde_json/toml, sqlx (SQLite) o redb, keyring, zip, tracing, thiserror/anyhow, notify
- Download paralleli con ripresa e verifica SHA1/SHA256

## Ordine di sviluppo
1. Core headless: version manifest, download, avvio vanilla da CLI
2. Login Microsoft
3. Istanze multiple + gestione Java
4. Loader (Fabric per primo)
5. UI e design system
6. Browser mod/pack
7. Sync impostazioni, temi, extra

## Skill da usare
- `security-review`: su login/token, download, estrazione zip (path traversal), salvataggio credenziali
- `code-review` e `simplify`: a fine di ogni modulo
- `run`: per verificare le modifiche nell'app reale, non solo nei test
- `artifact-design`: per mockup HTML della UI prima di implementarla
- Modalità plan per decisioni di architettura prima di scrivere codice

## Convenzioni
- Token e credenziali solo nel keyring di sistema, mai in chiaro su disco né nei log
- Errori tipizzati nei crate libreria (`thiserror`), `anyhow` solo nei binari
- Ogni file scaricato viene verificato con hash prima dell'uso
- Percorsi di estrazione validati contro path traversal
- `cargo fmt` e `cargo clippy` puliti prima di considerare un task finito
