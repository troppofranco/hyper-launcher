# Hyper Launcher

[![Latest release](https://img.shields.io/github/v/release/troppofranco/hyper-launcher?label=release)](https://github.com/troppofranco/hyper-launcher/releases/latest)
![Platform](https://img.shields.io/badge/platform-Windows-0078d4)
![Built with Rust](https://img.shields.io/badge/built%20with-Rust-b7410e)

A fast, modern Minecraft: Java Edition launcher written in Rust, with a desktop app built on Tauri 2 and Svelte 5.

Hyper Launcher keeps every installation isolated, while sharing libraries and assets between them. It installs mod loaders and content in one click and keeps your settings in sync across instances.

## Features

- **Isolated instances.** Each instance has its own worlds, mods and settings, plus an optional custom image. Libraries, assets and Java runtimes are downloaded once and shared.
- **Every major loader.** Vanilla, Fabric, Quilt, NeoForge and Forge, with the matching Java runtime downloaded automatically from Mojang.
- **Built-in content browser.** Search the Modrinth catalogue for mods, resource packs, shaders, data packs and modpacks:
  - one-click install that resolves required dependencies;
  - recognition of files you added by hand;
  - per-file and bulk updates.
- **Modpacks.** Install from the catalogue, or import and export `.mrpack` files. A modpack downloaded from Modrinth uses its project icon as the instance image.
- **Settings sync.** Optionally share keybinds, video, audio and control settings (`options.txt`) between instances. Only the values you change during a session are written back.
- **Customisation.**
  - Skin library with a live 3D preview. For offline accounts, the skin is applied in game through a generated resource pack.
  - Your skin's head is used as your profile picture.
  - Light and dark themes, accent colours, English and Italian interface.
- **Home dashboard.** Quick launch, featured modpacks, your saved servers with live player counts, a weekly playtime chart and Minecraft news.
- **Accounts.** Offline accounts and Microsoft sign-in (see [Microsoft accounts](#microsoft-accounts)). Tokens are stored only in the system keyring.
- **Automatic updates.** The app offers "Update and restart" as soon as a new build is published, and the CLI updates itself.
- **Command-line interface.** Everything the app does is also available from the `hyper` CLI.

## Installation (Windows)

1. Download `HyperLauncher-Setup.exe` from the [latest release](https://github.com/troppofranco/hyper-launcher/releases/latest).
2. Run it. It installs for the current user only, so no administrator rights are needed. It adds:
   - the **Hyper Launcher** app (Start menu, and optionally the desktop);
   - the `hyper` CLI, added to your `PATH`.

The installer is not code-signed yet. If Windows SmartScreen blocks it, choose **More info → Run anyway**.

Java does not need to be installed: the launcher downloads the official runtime each Minecraft version requires. Data is stored in `%APPDATA%\HyperLauncher`; set `HYPER_LAUNCHER_HOME` to use another folder.

On the first launch, a short setup asks for your language, your first account, and your theme and accent colour.

## Command-line usage

```text
hyper versions                                        # list Minecraft versions
hyper account add-offline <name>                      # add an offline account
hyper account add-microsoft                           # add a Microsoft account
hyper instance create <name> --version latest         # vanilla instance
hyper instance create <name> --version 26.3 --loader fabric   # or quilt, neoforge, forge
hyper launch <name>                                   # install if needed, then launch
hyper launch <name> --dry-run                         # print the launch command only
hyper content search sodium --instance <name>         # search Modrinth (alias: hyper mod)
hyper content add <name> iris --kind mod              # install with dependencies
hyper content update <name>                           # update installed mods
hyper pack install fabulously-optimized               # Modrinth modpack → new instance
hyper pack import pack.mrpack                         # import a modpack file
hyper pack export <name> pack.mrpack                  # export an instance
hyper update                                          # check for launcher updates now
```

Set `HYPER_NO_UPDATE=1` to turn off the CLI's automatic update check.

## Updates

Every commit to `main` publishes a numbered release (`build-<N>`) with the installer and its SHA-1 checksum. The app and the CLI check for newer builds at startup. The downloaded installer is always verified against its checksum before it runs.

## Microsoft accounts

Microsoft sign-in requires the client ID of an Azure application with these settings:

- supported account types: *Personal Microsoft accounts only*;
- *Allow public client flows* set to **Yes**;
- approved by Mojang for the Minecraft services API.

Set the client ID in **Settings → Microsoft account**, as `msa_client_id` in `config.toml`, or in the `HYPER_MSA_CLIENT_ID` environment variable. Until then, use offline accounts.

## Security

- Every downloaded file is verified against its published hash before use.
- Archive extraction (natives, modpacks, overrides) is validated against path traversal.
- Modpack downloads are restricted to the hosts allowed by the Modrinth specification.
- Account tokens never touch the disk or the logs: they live in the operating system's keyring.
- Forge and NeoForge are installed with their official installers, downloaded from the official Maven repositories and verified before running.

## Building from source

Requirements: a recent stable Rust toolchain, Node.js and npm, plus the [Tauri prerequisites](https://tauri.app/start/prerequisites/) for Windows.

```text
npm --prefix ui ci && npm --prefix ui run build   # build the interface (embedded in the app)
cargo run -p hyper-app                            # desktop app
cargo run -p hyper-cli -- versions                # CLI
npm --prefix ui run dev                           # interface in the browser, with a mock backend
```

Before submitting changes:

```text
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm --prefix ui run check
```

### Project layout

| Crate / folder | Responsibility |
|---|---|
| `hyper-core` | Paths, configuration, safe filesystem helpers (path traversal protection) |
| `hyper-auth` | Offline and Microsoft accounts, keyring-backed token storage |
| `hyper-installer` | Version manifest, Fabric/Quilt/NeoForge/Forge, verified parallel downloads, assets, natives, Java runtimes, self-update |
| `hyper-instances` | Isolated instances, content folders and worlds, launch command and pipeline, console, skins, saved servers |
| `hyper-content` | Modrinth catalogue: search, dependency-aware installs, updates, `.mrpack` import and export |
| `hyper-sync` | `options.txt` synchronisation between instances |
| `hyper-cli` | The `hyper` command-line binary |
| `hyper-app` + `ui/` | The `hyper-launcher` desktop app (Tauri 2 + Svelte 5) |

## License

Licensed under either of MIT or Apache-2.0, at your option.

Hyper Launcher is not an official Minecraft product and is not approved by or associated with Mojang or Microsoft.
