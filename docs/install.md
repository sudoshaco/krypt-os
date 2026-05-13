# Krypt OS — Installationsanleitung

> **Zielgruppe:** Sicherheitsbewusste Nutzer, die ein Xen-basiertes, vollständig verschlüsseltes
> System mit hardwarebasiertem Kill-Switch aufbauen möchten.
>
> Geschätzter Zeitaufwand: **60–90 Minuten**

---

## Inhaltsverzeichnis

1. [Voraussetzungen](#1-voraussetzungen)
2. [Bootbares Medium erstellen](#2-bootbares-medium-erstellen)
3. [Partitionierung & LUKS2](#3-partitionierung--luks2)
4. [Arch Linux Basis-Installation](#4-arch-linux-basis-installation)
5. [Xen Hypervisor](#5-xen-hypervisor)
6. [Krypt OS Komponenten bauen](#6-krypt-os-komponenten-bauen)
7. [USB Kill-Switch einrichten](#7-usb-kill-switch-einrichten)
8. [AppVMs konfigurieren](#8-appvms-konfigurieren)
9. [Dotfiles & Desktopumgebung](#9-dotfiles--desktopumgebung)
10. [Erste Anmeldung & Verifikation](#10-erste-anmeldung--verifikation)
11. [Automatisierte Installation (TUI)](#11-automatisierte-installation-tui)
12. [Troubleshooting](#12-troubleshooting)

---

## 1. Voraussetzungen

### Hardware

| Komponente | Minimum | Empfohlen |
|---|---|---|
| CPU | x86_64 mit VT-x/VT-d | Intel 10. Gen+ oder AMD Zen 2+ |
| RAM | 8 GB | 32 GB |
| SSD | 120 GB | 500 GB NVMe |
| USB-Stick | 8 GB | 32 GB (USB 3.x) |
| TPM | — | TPM 2.0 |

IOMMU muss im BIOS aktiviert sein: **VT-d** (Intel) bzw. **AMD-Vi** (AMD).

### BIOS-Einstellungen

```
Secure Boot        → Disabled
IOMMU / VT-d       → Enabled
USB Boot           → Enabled (nur für Installation, danach optional deaktivieren)
Hyperthreading     → nach Bedarf (SMT erhöht Angriffsfläche leicht)
```

### Live-Medium

Empfohlen: **Arch Linux ISO** (aktuell von archlinux.org).

```bash
# USB-Stick beschreiben (sda = USB-Stick, anpassen):
dd if=archlinux-*.iso of=/dev/sda bs=4M status=progress conv=fsync
```

---

## 2. Bootbares Medium erstellen

Nach dem Booten vom Arch-ISO:

```bash
# Tastaturbelegung setzen (optional)
loadkeys de-latin1

# Netzwerk prüfen
ping -c2 archlinux.org

# Systemuhr synchronisieren
timedatectl set-ntp true
```

---

## 3. Partitionierung & LUKS2

### Schema (GPT, empfohlen für UEFI)

```
/dev/sda1   512 MB    EFI System Partition   (FAT32, ESP-Flag)
/dev/sda2   REST      LUKS2-Container        → /dev/mapper/krypt-root
```

### Partitionierung

```bash
sgdisk --zap-all /dev/sda
sgdisk -n 1:0:+512M -t 1:ef00 -c 1:"EFI"   /dev/sda
sgdisk -n 2:0:0     -t 2:8309 -c 2:"LUKS"  /dev/sda
```

### LUKS2 mit Argon2id

```bash
cryptsetup luksFormat \
    --type luks2 \
    --cipher aes-xts-plain64 \
    --key-size 512 \
    --hash sha512 \
    --pbkdf argon2id \
    --pbkdf-memory 524288 \
    --pbkdf-parallel 4 \
    --iter-time 3000 \
    /dev/sda2
```

> **Passphrase-Anforderungen:** Mindestens 20 Zeichen, Kombination aus Groß-/Kleinbuchstaben,
> Ziffern und Sonderzeichen. Diese Passphrase sichert alle Daten — sie wird **nicht** wiederherstellbar sein.

```bash
# Container öffnen
cryptsetup luksOpen /dev/sda2 krypt-root

# Dateisysteme anlegen
mkfs.fat  -F32 -n EFI  /dev/sda1
mkfs.ext4 -L   ROOT    /dev/mapper/krypt-root

# Einhängen
mount /dev/mapper/krypt-root /mnt
mkdir /mnt/boot
mount /dev/sda1               /mnt/boot
```

---

## 4. Arch Linux Basis-Installation

```bash
# Spiegelliste optimieren (optional, beschleunigt Downloads)
reflector --country Germany,Austria,Switzerland --sort rate --save /etc/pacman.d/mirrorlist

# Basis-System installieren
pacstrap /mnt base base-devel linux linux-headers linux-firmware \
              amd-ucode intel-ucode \
              networkmanager sudo git vim \
              python python-pip

# fstab generieren
genfstab -U /mnt >> /mnt/etc/fstab

# Chroot
arch-chroot /mnt
```

### Im Chroot

```bash
# Zeitzone
ln -sf /usr/share/zoneinfo/Europe/Berlin /etc/localtime
hwclock --systohc

# Locale
echo "de_DE.UTF-8 UTF-8" >> /etc/locale.gen
echo "en_US.UTF-8 UTF-8" >> /etc/locale.gen
locale-gen
echo "LANG=de_DE.UTF-8" > /etc/locale.conf
echo "KEYMAP=de-latin1"  > /etc/vconsole.conf

# Hostname
echo "krypt" > /etc/hostname
cat >> /etc/hosts <<EOF
127.0.0.1   localhost
::1         localhost
127.0.1.1   krypt.localdomain krypt
EOF

# Root-Passwort
passwd

# Benutzer anlegen
useradd -m -G wheel,video,audio,storage -s /bin/bash BENUTZERNAME
passwd BENUTZERNAME
echo "%wheel ALL=(ALL:ALL) ALL" >> /etc/sudoers.d/wheel

# mkinitcpio mit encrypt-Hook
# /etc/mkinitcpio.conf: HOOKS=(... block encrypt filesystems ...)
sed -i 's/^HOOKS=.*/HOOKS=(base udev autodetect modconf block encrypt filesystems keyboard fsck)/' \
    /etc/mkinitcpio.conf
mkinitcpio -P
```

### GRUB mit LUKS

```bash
pacman -S grub efibootmgr

# UUID des LUKS-Partitions ermitteln
LUKS_UUID=$(blkid -s UUID -o value /dev/sda2)

# /etc/default/grub anpassen
sed -i "s|GRUB_CMDLINE_LINUX=\"\"|GRUB_CMDLINE_LINUX=\"cryptdevice=UUID=${LUKS_UUID}:krypt-root root=/dev/mapper/krypt-root\"|" \
    /etc/default/grub

grub-install --target=x86_64-efi --efi-directory=/boot --bootloader-id=KRYPT
grub-mkconfig -o /boot/grub/grub.cfg
```

---

## 5. Xen Hypervisor

```bash
# Xen und Dom0-Kernel
pacman -S xen linux-xen

# Xen-GRUB-Eintrag aktivieren
# (xen-grub-mkconfig legt /boot/grub/grub.cfg-Einträge an)
grub-mkconfig -o /boot/grub/grub.cfg

# Dom0-Speicher begrenzen (in /etc/default/grub):
# GRUB_CMDLINE_XEN_DEFAULT="dom0_mem=4096M,max:4096M dom0_vcpus_pin"
sed -i 's|^#GRUB_CMDLINE_XEN_DEFAULT.*|GRUB_CMDLINE_XEN_DEFAULT="dom0_mem=4096M,max:4096M dom0_vcpus_pin iommu=1"|' \
    /etc/default/grub
grub-mkconfig -o /boot/grub/grub.cfg

# Services
systemctl enable xen-init-dom0
systemctl enable xenconsoled
systemctl enable xendomains
systemctl enable xen-watchdog

# Dom0 paravirt-Netzwerk
pacman -S bridge-utils
systemctl enable NetworkManager
```

---

## 6. Krypt OS Komponenten bauen

```bash
# Rust-Toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Repository klonen
git clone https://github.com/BENUTZER/krypt-os ~/krypt-os
cd ~/krypt-os

# Workspace bauen
cargo build --release

# Binaries installieren
sudo install -m755 target/release/krypt-daemon   /usr/local/sbin/krypt-daemon
sudo install -m755 target/release/krypt-stick    /usr/local/bin/krypt-stick
sudo install -m755 target/release/gui-protocol   /usr/local/bin/krypt-gui

# Systemd-Units
sudo install -m644 krypt-daemon/krypt-daemon.service  /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now krypt-daemon
```

### Daemon-Konfiguration

```bash
sudo mkdir -p /etc/krypt
sudo cp krypt-daemon/daemon.toml.example /etc/krypt/daemon.toml
sudo vim /etc/krypt/daemon.toml
```

Wichtige Felder in `/etc/krypt/daemon.toml`:

```toml
[daemon]
socket_path = "/run/krypt/daemon.sock"
log_level   = "info"

[policy]
usb_kill_switch = true
kill_on_unplug  = true
allowed_devices = []  # wird von krypt-stick befüllt

[vms]
# Werden automatisch aus XL-Configs geladen
xl_config_dir = "/etc/xen/krypt"
```

---

## 7. USB Kill-Switch einrichten

Der USB-Stick dient als **Hardware-Schlüssel**: Wird er entfernt, sperrt der krypt-daemon
alle laufenden VMs sofort (Suspend-to-RAM oder Force-Shutdown je nach Policy).

```bash
# Stick einlegen, dann einrichten:
sudo krypt-stick --setup \
    --luks-device /dev/mapper/krypt-root \
    --slot 1

# Test: Stick abziehen — alle VMs müssen einfrieren
# Stick wieder einlegen — System entsperrt
```

> Der Stick wird als zusätzlicher LUKS-Schlüssel in Slot 1 eingetragen (Slot 0 = Passphrase).
> Der Daemon überwacht `/dev/disk/by-id/` via udev.

---

## 8. AppVMs konfigurieren

### XL-Konfiguration

Jede AppVM benötigt eine XL-Config unter `/etc/xen/krypt/`:

```bash
sudo mkdir -p /etc/xen/krypt

# Beispiel: Arbeits-VM
sudo tee /etc/xen/krypt/work.cfg <<'EOF'
name        = "work"
vcpus       = 2
memory      = 2048
disk        = ['phy:/dev/mapper/krypt-work,xvda,rw']
vif         = ['bridge=xenbr0']
kernel      = "/var/lib/xen/boot/vm-kernel"
extra       = "root=/dev/xvda rw console=hvc0"
on_poweroff = "destroy"
on_reboot   = "restart"
on_crash    = "destroy"
EOF
```

### Trust-Level (Fenstertitel-Format)

Fenstertitel folgen ADR-011: `[<trust>] <vm>: <titel>`

| Trust | Bedeutung | Hyprland-Farbe |
|---|---|---|
| `high` | Verschlüsselt, isoliert (vault, work) | Krypt-Violet `#9d4edd` |
| `medium` | Netzwerk-VMs (browser) | Catppuccin Blue `#89b4fa` |
| `low` | Untrusted (disposable) | Catppuccin Red `#f38ba8` |

### VM starten

```bash
sudo xl create /etc/xen/krypt/work.cfg
sudo xl list       # alle laufenden VMs
sudo xl console work  # Konsole öffnen
```

---

## 9. Dotfiles & Desktopumgebung

```bash
# Pakete
sudo pacman -S hyprland waybar foot rofi-wayland \
               neovim ripgrep fd fzf git-delta lazygit \
               ttf-jetbrains-mono-nerd noto-fonts-emoji

# Dotfiles installieren
cd ~/krypt-os
./dotfiles/install.sh

# Bei Root für GRUB + Plymouth-Theme:
sudo ./dotfiles/install.sh
```

Das Skript legt Symlinks an für:
- `~/.config/hypr/`      ← Hyprland-Konfiguration
- `~/.config/waybar/`    ← Statusbar
- `~/.config/foot/`      ← Terminal
- `~/.config/rofi/`      ← App-Launcher
- `~/.config/nvim/`      ← Neovim (lazy.nvim, LSP, Catppuccin Mocha)

### Neovim-Ersteinrichtung

Beim ersten Start installiert lazy.nvim alle Plugins automatisch:

```bash
nvim   # → lazy.nvim bootstrapped sich selbst
       # → :Mason öffnet LSP-Installer
       # → rust-analyzer, pyright, lua_ls werden automatisch installiert
```

---

## 10. Erste Anmeldung & Verifikation

Nach dem Neustart (Xen-Kernel):

```bash
# Dom0-Info
sudo xl info

# IOMMU-Status prüfen
sudo xl dmesg | grep -i iommu

# Krypt-Daemon läuft
systemctl status krypt-daemon

# USB Kill-Switch testen
# → Stick abziehen
journalctl -f -u krypt-daemon
# Erwartet: "USB kill-switch triggered — suspending all VMs"

# Stick wieder einlegen
# Erwartet: "Kill-switch device restored"
```

### Sicherheits-Checkliste

- [ ] IOMMU aktiv (`xl dmesg | grep -i iommu`)
- [ ] Dom0-Speicher begrenzt (`xl info | grep free_memory` < 4096 MB)
- [ ] LUKS-Header gesichert (`cryptsetup luksHeaderBackup /dev/sda2 --header-backup-file luks-header.bak`)
- [ ] USB Kill-Switch verifiziert
- [ ] Hyprland windowrulev2-Regeln aktiv (Vertrauensindikatoren in Titelleiste)
- [ ] `cargo test` und `cargo clippy` fehlerfrei

---

## 11. Automatisierte Installation (TUI)

Alternativ zur manuellen Installation bietet Krypt OS einen interaktiven TUI-Installer:

```bash
# Aus dem Live-ISO heraus:
git clone https://github.com/BENUTZER/krypt-os
cd krypt-os/installer

pip install -r requirements.txt
sudo python main.py
```

Der Installer führt durch:
1. Datenträgerwahl
2. LUKS2-Passphrase (Stärkeanzeige, min. 20 Zeichen)
3. Basis-Installation (Arch + Xen)
4. USB Kill-Switch-Setup
5. AppVM-Auswahl und Konfiguration

> **Hinweis:** Der TUI-Installer ist noch Beta. Für Produktivsysteme wird die manuelle
> Installation empfohlen, da sie mehr Kontrolle über jeden Schritt gibt.

---

## 12. Troubleshooting

### LUKS-Container öffnet nicht nach Neustart

```bash
# Richtiger UUID in /etc/default/grub?
blkid /dev/sda2 | grep UUID
grep cryptdevice /etc/default/grub

# HOOKS korrekt in /etc/mkinitcpio.conf?
grep ^HOOKS /etc/mkinitcpio.conf
# Muss enthalten: block encrypt filesystems
```

### Xen startet nicht (kein xl-Befehl)

```bash
# Xen-Kernel läuft?
uname -r   # sollte "xen" enthalten oder
dmesg | grep -i hypervisor

# Dom0-Kernel ist nicht der Standard-Kernel:
grep -A3 "Xen" /boot/grub/grub.cfg
# Ggf. GRUB-Standard-Eintrag auf Xen setzen:
grub-set-default "Xen ..."
```

### krypt-daemon startet nicht

```bash
journalctl -u krypt-daemon --no-pager -n 50

# Socket-Verzeichnis vorhanden?
ls -la /run/krypt/

# daemon.toml vorhanden?
ls /etc/krypt/daemon.toml
```

### Neovim: LSP nicht aktiv

```bash
# Im Neovim:
:LspInfo      # Status aller LSPs
:Mason        # Fehlende Server nachinstallieren
:checkhealth  # Gesamtdiagnose
```

### USB Kill-Switch löst nicht aus

```bash
# udev-Regeln prüfen
krypt-stick --status

# Daemon-Log
journalctl -f -u krypt-daemon | grep -i usb

# Manuell testen
sudo krypt-stick --test-kill
```

---

## Anhang

### Wichtige Pfade

| Pfad | Inhalt |
|---|---|
| `/etc/krypt/daemon.toml` | Daemon-Konfiguration |
| `/etc/xen/krypt/` | XL-Configs der AppVMs |
| `/run/krypt/daemon.sock` | IPC-Socket |
| `/var/log/krypt/` | Daemon-Logs |
| `~/.config/nvim/` | Neovim-Config (Symlink aus Dotfiles) |
| `~/.config/hypr/` | Hyprland-Config |

### Referenzen

- [Arch Linux Installationsanleitung](https://wiki.archlinux.org/title/Installation_guide)
- [Arch Linux Xen](https://wiki.archlinux.org/title/Xen)
- [LUKS2 / dm-crypt](https://wiki.archlinux.org/title/Dm-crypt)
- [Hyprland](https://hyprland.org)
- `docs/architecture.md` — Systemarchitektur
- `docs/decisions.md` — Architekturentscheidungen (ADRs)
- `docs/usb-kill-switch.md` — Kill-Switch-Details
