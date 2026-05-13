# Krypt OS

> Cryptography-first. Compartmentalization by design. Lightweight by discipline.

[![CI](https://github.com/sudoshaco/krypt-os/actions/workflows/build-iso.yml/badge.svg)](https://github.com/sudoshaco/krypt-os/actions/workflows/build-iso.yml)
[![Rust](https://img.shields.io/badge/rust-stable-orange.svg)](https://www.rust-lang.org)
[![License: GPL-3.0](https://img.shields.io/badge/License-GPL--3.0-blue.svg)](LICENSE)
[![Arch Linux](https://img.shields.io/badge/base-Arch%20Linux-1793d1?logo=arch-linux)](https://archlinux.org)
[![Xen](https://img.shields.io/badge/hypervisor-Xen%20Type--1-green)](https://xenproject.org)

---

Krypt ist eine Linux-Distribution für Menschen die keine Kompromisse eingehen.

Das Sicherheitsmodell von QubesOS. Die Ästhetik von Omarchy. Der Footprint von Alpine.

---

## Warum Krypt?

| Problem | Beispiel |
|---|---|
| Sicher aber hässlich und schwer | QubesOS (XFCE, 16GB+ RAM Minimum) |
| Schön aber kein echtes Sicherheitsmodell | Tails, Whonix (kein Hypervisor) |
| Leichtgewichtig aber unsicher | Alpine, Arch vanilla |

Krypt löst alle drei gleichzeitig.

---

## Features

- **Xen Typ-1 Hypervisor** — kein Shared-Kernel zwischen VMs
- **LUKS2 + Argon2id** — Full-Disk-Encryption auf Produktion-Niveau
- **USB Kill-Switch** — Stick raus = alle VMs sofort eingefroren
- **Trust-Level-System** — `black > green > yellow > orange > red` pro VM
- **krypt-daemon** (Rust) — VM-Lifecycle + Policy Engine, ersetzt qubesd
- **Wayland-native GUI** — AppVM-Fenster via `wl_shm` mit Trust-Border
- **Hyprland + Catppuccin Mocha** — modernes Tiling-WM, Krypt-Violet Akzent
- **TUI-Installer** — Python + textual, Disk→LUKS2→Xen→USB→VMs in einem Durchgang
- **Neovim-Config** — lazy.nvim, LSP für Rust + Python, gleiche Qualität wie Omarchy

---

## Screenshots

> *Screenshots folgen bei erster Beta-Release.*

```
┌─────────────────────────────────────────────────────────┐
│  [black] vault: KeePassXC         [green] work: Neovim  │
│  ░░░░░░░░░░░░░░░░░░░░░░░░         ░░░░░░░░░░░░░░░░░░░░  │
│  Border: Krypt-Violet #9d4edd     Border: Green #a6e3a1  │
│                                                          │
│  [yellow] browser: Firefox                               │
│  ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░                          │
│  Border: Yellow #f9e2af                                  │
├─────────────────────────────────────────────────────────┤
│  ■ work  ■ browser  □ vault  ■ personal    12:34  🔋 87% │
└─────────────────────────────────────────────────────────┘
```

---

## Architektur

```
HARDWARE (VT-d/AMD-Vi · TPM2 · Secure Boot)
        │
XEN HYPERVISOR (Typ-1)
        │
dom0 ── krypt-daemon (Rust)
   │        Policy Engine · VM-Lifecycle · USB-Kill-Switch
   │
   ├── sys-net      Netzwerk-Isolation
   ├── sys-firewall Traffic-Filterung
   ├── sys-usb      USB-Isolation
   ├── sys-gui      Arch + Hyprland + krypt-gui-protocol
   └── AppVMs       Alpine-Templates
           work │ browser │ hacking │ personal │ vault
```

---

## Eigenentwicklungen

| Komponente | Sprache | Zweck |
|---|---|---|
| `krypt-daemon` | Rust | VM-Lifecycle, Policy Engine, USB-Kill-Switch |
| `krypt-stick` | Rust | USB Auth-Stick Setup + LUKS2-Schlüsselverwaltung |
| `krypt-gui-protocol` | Rust | Wayland-native AppVM-Fenster (wl_shm 60fps) |
| `krypt-installer` | Python | TUI-Installer mit Full-Disk-Encryption |
| Waybar-Module | Python | VM-Status, Trust-Level via IPC |

---

## Quick Start

### Voraussetzungen

- x86_64 CPU mit VT-d (Intel) oder AMD-Vi
- Mindestens 8 GB RAM, 120 GB SSD
- USB-Stick ≥ 8 GB für Installation + USB Kill-Switch

### ISO herunterladen (empfohlen)

```bash
# Von GitHub Releases (sobald verfügbar):
wget https://github.com/sudoshaco/krypt-os/releases/latest/download/krypt-os-<version>.iso
sha256sum -c krypt-os-<version>.sha256

# Auf USB schreiben:
dd if=krypt-os-<version>.iso of=/dev/sdX bs=4M status=progress oflag=sync
```

### Selbst bauen

```bash
git clone https://github.com/sudoshaco/krypt-os
cd krypt-os

# Rust-Tests + Clippy
cargo test
cargo clippy -- -D warnings

# ISO bauen (Arch Linux mit archiso-Paket erforderlich)
sudo ./build/build.sh

# ISO liegt dann unter dist/
```

### Installation

→ **[docs/install.md](docs/install.md)** — vollständige Schritt-für-Schritt-Anleitung

Oder interaktiv mit dem TUI-Installer (auf dem Live-ISO):

```bash
sudo krypt-install
```

---

## USB Kill-Switch

Der physische Schlüssel zur Existenz des Systems.

```
Stick drin   →  System läuft
Stick raus   →  Sofort. Alles. Eingefroren.
Kein Stick   →  Kein Boot. Punkt.
```

- Normaler USB-Stick — sieht aus wie jeder andere
- 64-Byte-Zufallsschlüssel in LUKS2-Keyslot 1
- Konfigurierbare Panic-Level (Lock / Suspend / Nuke)
- Backup-Sticks via `krypt-stick backup add /dev/sdX`
- **Verlorener Stick ohne Backup: keine Wiederherstellung möglich**

→ Details: [docs/usb-kill-switch.md](docs/usb-kill-switch.md)

---

## Roadmap

| Phase | Status | Inhalt |
|---|---|---|
| 1–5 — Fundament | ✅ | Cargo-Workspace, USB Kill-Switch, Policy Engine, IPC |
| 6–8 — GUI + Design | ✅ | wayland-client 0.31, Dotfiles, Design-System |
| 9 — IPC + ADRs | ✅ | VmStart IPC, ADR-011 xdg_toplevel |
| 10 — 60fps + TUI | ✅ | wl_shm Pipeline, Installer, Neovim-Config |
| 11 — Final Polish | 🔄 | ISO-Build, CI-Gate, README |
| 12 — Xen Grant Table | ⬜ | Echte VM-Pixel, wl_frame_callback |
| Beta | ⬜ | Erstes öffentliches ISO |

---

## Contributing

Krypt OS ist in früher Entwicklung. Contributions sind willkommen, aber bitte erst ein Issue öffnen.

### Setup

```bash
git clone https://github.com/sudoshaco/krypt-os
cd krypt-os

# Rust (stable)
rustup default stable

# System-Dependencies (Arch)
sudo pacman -S wayland wayland-protocols pkg-config

# Bauen und testen
cargo build
cargo test
cargo clippy -- -D warnings
```

### Coding Standards

**Rust:**
- `cargo clippy -- -D warnings` muss sauber sein
- Kein `unwrap()` in Produktionscode — `?` oder explizites `match`
- Kein `unsafe` außer für Xen/Wayland-FFI (dann mit Kommentar)
- Tests für neue Logik

**Python:**
- Type hints überall (`from __future__ import annotations`)
- `subprocess.run` mit `timeout=` und `capture_output=True`
- Kein `bare except`

### Commit-Nachrichten

```
feat: [komponente] kurze beschreibung
fix:  [komponente] was wurde behoben
docs: was wurde dokumentiert
```

### Pull Requests

1. Fork + Branch von `main`
2. `cargo test && cargo clippy -- -D warnings`
3. PR mit Beschreibung was und warum

---

## Hardware-Empfehlungen

| Komponente | Minimum | Empfohlen |
|---|---|---|
| CPU | VT-d/AMD-Vi | Intel 10. Gen+ oder AMD Zen 2+ |
| RAM | 8 GB | 32 GB |
| SSD | 120 GB | 500 GB NVMe |
| USB-Stick | 8 GB | 32 GB USB 3.x |
| TPM | — | TPM 2.0 |

---

## Sicherheits-Prinzipien

1. **dom0 bekommt keinen Internetzugang** — nie
2. **Kein Shared Kernel** — jede AppVM in echter Xen-Domain
3. **Clipboard nie automatisch** — immer explizite Nutzerbestätigung
4. **USB nie direkt in AppVMs** — immer via sys-usb
5. **Keine Secrets in git**
6. **Security by Default** — restriktivste Policy ist der Default

---

## Philosophie

> "Complexity is the enemy of security."

Krypt ist komplex im Sicherheitsmodell, einfach in der Nutzung.
Ein System das zu aufwändig zu nutzen ist wird nicht genutzt.
Ein System das nicht genutzt wird schützt nichts.

---

GPL-3.0 · [docs/](docs/) · [PROGRESS.md](PROGRESS.md)
