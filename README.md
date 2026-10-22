# MHXX Attack Damage Display

A Forge plugin that displays the local hunter's attack damage in **Monster Hunter XX (Japanese version 1.5.1)**.

This project is a fork of [Lawrence-al/mhgu-attack-damage-display](https://github.com/Lawrence-al/mhgu-attack-damage-display), which was originally made for MHGU 1.4.0. This fork adapts the plugin for MHXX Japanese v1.5.1 and requires an **MHXX-compatible build of Forge**.

> **Compatibility:** This project targets Monster Hunter XX (Japanese version 1.5.1). Other game versions and regions are not guaranteed to work.

## Features

- Displays damage numbers near the local hunter's hit location.
- Displays a separate number for each valid hit, including multi-hit attacks and phial explosions attributed to the hunter.
- Uses the game's original numeric font.
- Uses the existing Forge-configured text color rather than the original separate hitzone-based highlight and normal colors.
- Shows affinity indicators for critical hits and negative-affinity hits.
- Keeps numbers anchored to their hit locations while updating their screen positions as the camera moves. Numbers do not continuously follow moving monster body parts.

Only damage from the local hunter is displayed. Damage from other hunters, companions, and damage-over-time effects such as poison is not included.

## Requirements

- Monster Hunter XX — Japanese version 1.5.1
- An MHXX-compatible build of Forge
- A compatible game runtime or emulator

Install and verify Forge before installing this plugin. The original MHGU Forge build is not a substitute for the MHXX-compatible build required by this fork.

## Installation

1. Install the MHXX-compatible Forge build and confirm that it loads correctly.
2. Download the latest release from the [Releases](https://github.com/ZellD-0371/MHXX-Damage-Display/releases/latest) page.
3. Extract the package while preserving its directory structure, or copy `attack_damage_display.nro` into the plugin directory of your MHXX mod:

   ```text
   romfs/nativeNX/plugins/attack_damage_display.nro
   ```

4. Make sure there is only one copy of `attack_damage_display.nro` in the plugin directory.
5. Fully close and restart the game, then enter a quest and attack a monster to test the display.

To update, replace the existing NRO with the new version. Your existing `attack_damage_display.ini` settings are preserved. To disable the plugin, remove the NRO and restart the game.

## Configuration

Open **Forge → Plugin UI → Attack Damage Display** to adjust the plugin settings.

- **Save INI** saves the current settings.
- **Revert changes** restores the previously saved settings.
- **Defaults** restores the default values in the current settings editor; save afterward to keep them.

The plugin reads and writes `attack_damage_display.ini` in the current MHXX user's save-data root; it does not create a separate configuration subdirectory. The menu and manual edits use the same file. Restart the game after editing the INI manually.

### Settings

| Setting | Default | Description |
| --- | --- | --- |
| `enabled` | `true` | Enables or disables damage display. |
| `font_size` | `20` | Damage-number size, based on 1080p and scaled for emulator. |
| `outline_width` | `0.6` | Text outline width. Set to `0` to disable the outline. |
| `full_opacity` | `1.0` | Time in seconds that the number remains fully opaque. |
| `lifetime` | `1.3` | Total time in seconds that the number is displayed. |

The total `lifetime` must be greater than `full_opacity`. Text color follows the existing Forge color setting.

## Building from Source

The project can be built separately; an MHGU Forge workspace is not required. Build requirements include Git, Rust nightly with `rust-src`, and devkitPro/devkitARM. On Windows, use the devkitPro MSYS2 Bash environment for `build.sh`. The Rust MSVC toolchain may also require the Visual Studio C++ Build Tools.

Install the Rust nightly components:

```bash
rustup toolchain install nightly --component rust-src
```

From the project root, build the NRO:

```bash
bash "build.sh"
```

The output NRO is written to `dist/attack_damage_display.nro`. The build scripts place their SDK and packaging tools under the project's `target/` directory. The first build needs an internet connection to download dependencies. If the required dependencies are already cached, use:

```bash
bash "build.sh" --offline
```

On Windows, you can also run the PowerShell build script:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\build.ps1
```

Append `-Offline` to the PowerShell command to request an offline build. The default devkitPro path is `C:/devkitPro`; set `DEVKITPRO` in the current process if your installation is elsewhere.

## Testing and Packaging

With Node.js installed, run the project's checks:

```bash
bash "test.sh"
```

The checks cover the default configuration, settings persistence, and NRO format. They do not replace testing in the target game.

After a successful build, Python 3 can create a release package:

```bash
python "tools/package.py"
```

This generates `dist/attack_damage_display.zip`. To install the current build using the included Node.js helper, run:

```bash
node "tools/install.cjs" "path/to/your/mod/romfs/nativeNX/plugins"
```

Packaging and installation do not overwrite or copy your save-data INI, so existing settings are preserved. A game installation is not required for the basic build and configuration checks, but the final plugin should be tested in MHXX Japanese v1.5.1 with the compatible Forge build.

## Credits

- **Original project:** [Lawrence-al/mhgu-attack-damage-display](https://github.com/Lawrence-al/mhgu-attack-damage-display)
- **Forge:** This plugin requires an MHXX-compatible Forge build.

This project is derived from the original project. See the repository's `LICENSE` file for the applicable license terms, and retain the original copyright and license notices as required.

## Disclaimer

This is an unofficial fan-made project. It is not affiliated with, endorsed by, or supported by Capcom.
