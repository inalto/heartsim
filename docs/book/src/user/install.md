# Installing heartsim

Installers for each release are attached to the
[GitHub releases page](https://github.com/inalto/heartsim/releases). Each one bundles the default heart model and the
rhythm library; nothing else is downloaded.

| System | File | Notes |
|---|---|---|
| Windows 10/11 (x64) | `heartsim_<version>_x64_en-US.msi` or `…-setup.exe` | Installs to *Program Files* and adds a Start-menu entry. |
| macOS 11+ (Intel and Apple silicon) | `HeartSim_<version>_universal.dmg` | Open the image and drag *HeartSim* to *Applications*. |
| Debian / Ubuntu 22.04+ | `heartsim_<version>_amd64.deb` | `sudo apt install ./heartsim_<version>_amd64.deb` |
| Other Linux | `heartsim_<version>_amd64.AppImage` | `chmod +x` it and run it; no installation needed. It uses the system's `libasound2` (ALSA), `libgl1` and `libvulkan1`, which every desktop distribution has. |

Check the installation from a terminal with `heartsim --version`.

## Update check

At start the desktop app asks `api.github.com` once whether a newer release exists. The request carries the app version in
its user agent (`heartsim/0.7.0`) and nothing else: no account, no machine identifier, nothing stored. When a newer release
exists, a thin bar under the menu bar and the About window name the version with a link to its release page; the app never
downloads or installs anything by itself. Turn the check off with *Check for a newer release at start* in **Settings**: the
app then makes no request at all. Without a network the check fails silently.

## Graphics requirements

heartsim draws the heart with the GPU through Vulkan, Metal or DirectX 12, and falls back to OpenGL ES 3 where those are
missing. Any integrated GPU from the last ten years is enough. On a Linux machine without a GPU, install Mesa's software
Vulkan driver (*lavapipe*, package `mesa-vulkan-drivers`): heartsim then runs, more slowly.

## Building from source

You need the Rust toolchain from [rustup.rs](https://rustup.rs); the repository pins the version in `rust-toolchain.toml`.
On Linux also install `libxkbcommon-dev libwayland-dev libasound2-dev libudev-dev` (Debian/Ubuntu names).

```bash
git clone https://github.com/inalto/heartsim
```

```bash
cd heartsim && cargo run --release -p heartsim-app
```

The first build takes several minutes. The command-line tool is built with `cargo build --release -p heartsim-cli`
(see [Command line](cli.md)).
