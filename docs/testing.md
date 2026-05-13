# Krypt OS — Test-Checkliste (Pre-First-Boot)

Dieses Dokument beschreibt alle Checks die vor dem ersten produktiven Boot
eines Krypt OS-Systems durchgeführt werden müssen.

Status-Legende: ✅ bestanden · ❌ fehlgeschlagen · ⚠️ manuell prüfen · ⬜ noch nicht getestet

---

## 1. Build-Verifikation (Entwicklungsumgebung)

### 1.1 Rust-Workspace

```bash
cd ~/krypt-os

# Alle 3 Crates müssen sauber kompilieren
cargo build --release
# Erwartung: Finished release — keine errors

# Clippy muss ohne Warnings durchlaufen
cargo clippy -- -D warnings
# Erwartung: Finished — keine errors

# Alle Tests grün
cargo test
# Erwartung: test result: ok. N passed; 0 failed
```

| Test | Status |
|---|---|
| `cargo build --release` sauber | ⬜ |
| `cargo clippy -- -D warnings` sauber | ⬜ |
| `cargo test` alle grün (34+ Tests) | ⬜ |
| Binary `krypt-daemon` existiert in `target/release/` | ⬜ |
| Binary `krypt-stick` existiert in `target/release/` | ⬜ |
| Binary `krypt-gui` existiert in `target/release/` | ⬜ |

### 1.2 Installer-Syntax

```bash
# Python Syntax-Check aller Installer-Dateien
python3 -m py_compile installer/main.py
python3 -m py_compile installer/steps/welcome.py
python3 -m py_compile installer/steps/disk.py
python3 -m py_compile installer/steps/luks.py
python3 -m py_compile installer/steps/install.py
python3 -m py_compile installer/steps/usb.py
python3 -m py_compile installer/steps/vms.py
# Erwartung: kein Output (kein Fehler)
```

| Test | Status |
|---|---|
| `installer/main.py` syntaktisch korrekt | ⬜ |
| Alle `installer/steps/*.py` syntaktisch korrekt | ⬜ |

### 1.3 Shell-Scripts

```bash
bash -n build/build.sh
bash -n dotfiles/install.sh
bash -n initramfs/hooks/krypt
bash -n initramfs/install/krypt
bash -n initramfs/krypt-boot-agent.sh
# Erwartung: kein Output
```

| Test | Status |
|---|---|
| Alle .sh-Dateien syntaktisch korrekt | ⬜ |

---

## 2. ISO-Build (Arch Linux mit archiso)

```bash
# Voraussetzungen
pacman -S archiso xorriso squashfs-tools mtools

# Build starten (dauert 10–30 Minuten je nach Netzwerk)
sudo ./build/build.sh --clean 2>&1 | tee /tmp/krypt-build.log

# Ergebnis prüfen
ls -la dist/*.iso
sha256sum -c dist/*.sha256
```

| Test | Status | Notizen |
|---|---|---|
| `build.sh` läuft ohne Fehler durch | ⬜ | |
| ISO wurde in `dist/` erzeugt | ⬜ | |
| ISO-Größe plausibel (800 MB – 3 GB) | ⬜ | |
| SHA256 stimmt überein | ⬜ | |
| profiledef.sh im Profil korrekt generiert | ⬜ | |
| krypt-daemon Binary im ISO (`/usr/bin/krypt-daemon`) | ⬜ | |
| krypt-stick Binary im ISO (`/usr/bin/krypt-stick`) | ⬜ | |
| daemon.toml im ISO (`/etc/krypt/daemon.toml`) | ⬜ | |
| Trust-Level in daemon.toml lowercase | ⬜ | `grep trust_level dist/...` |
| Installer im ISO (`/usr/share/krypt-installer/main.py`) | ⬜ | |
| krypt-install Wrapper (`/usr/bin/krypt-install`) | ⬜ | |
| GRUB-Theme im ISO (`/boot/grub/themes/krypt-grub/`) | ⬜ | |
| Plymouth-Theme im ISO (`/usr/share/plymouth/themes/krypt/`) | ⬜ | |
| Dotfiles in `/etc/skel/.config/` | ⬜ | nvim, hypr, waybar, rofi, foot |

---

## 3. VM-Boot-Test (QEMU-Test ohne echte Hardware)

Für einen ersten Funktionstest reicht QEMU — Xen kann dort nicht getestet werden,
aber LUKS, GRUB und Installer laufen im QEMU-Modus (kein Xen).

```bash
# QEMU-Boot des ISO (UEFI)
qemu-system-x86_64 \
  -m 4096 \
  -cpu host \
  -enable-kvm \
  -bios /usr/share/OVMF/OVMF_CODE.fd \
  -cdrom dist/krypt-os-*.iso \
  -boot d \
  -nographic
```

| Test | Status | Notizen |
|---|---|---|
| ISO bootet ohne Kernel-Panic | ⬜ | |
| GRUB-Menü erscheint mit Krypt-Theme | ⬜ | |
| Plymouth-Splash erscheint kurz | ⬜ | |
| Arch-Basis booted ins Live-System | ⬜ | |
| `krypt-install` Command verfügbar | ⬜ | `which krypt-install` |
| Installer-TUI startet auf tty1 | ⬜ | |
| Installer-TUI auf tty2 manuell startbar | ⬜ | `sudo krypt-install` |

---

## 4. Installer-Ablauf (QEMU mit virtuellem Disk)

```bash
# QEMU mit virtuellem Disk (für Installer-Test)
qemu-img create -f qcow2 /tmp/krypt-test.qcow2 40G
qemu-system-x86_64 \
  -m 8192 \
  -cpu host -enable-kvm \
  -bios /usr/share/OVMF/OVMF_CODE.fd \
  -cdrom dist/krypt-os-*.iso \
  -drive file=/tmp/krypt-test.qcow2,format=qcow2 \
  -boot d
```

### 4.1 Welcome-Screen

| Test | Status |
|---|---|
| ASCII-Logo sichtbar | ⬜ |
| "Starten" Button vorhanden | ⬜ |
| "Beenden" Button vorhanden | ⬜ |

### 4.2 Disk-Auswahl

| Test | Status |
|---|---|
| Virtuelle Disk erscheint in der Liste | ⬜ |
| Auswahl aktiviert "Weiter"-Button | ⬜ |
| Warnung "Alle Daten gelöscht" sichtbar | ⬜ |

### 4.3 LUKS2-Passphrase

| Test | Status | Notizen |
|---|---|---|
| Stärke-Indikator reagiert auf Eingabe | ⬜ | |
| Passphrase < 20 Zeichen: "Weiter" disabled | ⬜ | |
| Passphrase ≥ 20 Zeichen, Bestätigung: "Weiter" enabled | ⬜ | |

### 4.4 Installation

| Test | Status | Notizen |
|---|---|---|
| Partitionierung: kein Fehler | ⬜ | sgdisk -n Ausgabe |
| LUKS2-Format: kein Fehler | ⬜ | Argon2id |
| Basis-System pacstrap: kein Fehler | ⬜ | Netzwerk muss im QEMU vorhanden sein |
| Xen-Pakete: kein Fehler | ⬜ | |
| GRUB-Install: kein Fehler | ⬜ | |
| Fortschrittsbalken erreicht 100% | ⬜ | |
| "Weiter"-Button erscheint | ⬜ | |

### 4.5 USB-Screen

| Test | Status | Notizen |
|---|---|---|
| Screen erscheint nach Installation | ⬜ | |
| "Überspringen" navigiert weiter | ⬜ | QEMU hat keine echten USB-Sticks |

### 4.6 VM-Konfiguration

| Test | Status | Notizen |
|---|---|---|
| VM-Checkboxen sichtbar | ⬜ | sys-gui, work, browser, vault, personal |
| XL-Configs werden in `/mnt/etc/xen/krypt/` geschrieben | ⬜ | |
| daemon.toml wird in `/mnt/etc/krypt/daemon.toml` geschrieben | ⬜ | |
| Trust-Level in erzeugtem daemon.toml lowercase | ⬜ | `grep trust_level /mnt/etc/krypt/daemon.toml` |
| Finish-Screen erscheint | ⬜ | |

---

## 5. Installiertes System (nach Neustart)

### 5.1 Boot-Sequenz

| Test | Status | Notizen |
|---|---|---|
| GRUB bootet ohne Fehler | ⬜ | Xen-Eintrag vorhanden? |
| LUKS-Passphrase-Prompt erscheint | ⬜ | |
| System bootet in dom0 | ⬜ | |
| `xl info` funktioniert | ⬜ | Xen läuft |
| `xl list` zeigt Domain-0 | ⬜ | |

### 5.2 IOMMU

```bash
# IOMMU muss aktiv sein — ohne IOMMU keine echte Isolation
xl dmesg | grep -i iommu
# Erwartet: "IOMMU: ... enabled" oder "VT-d ... detected"

dmesg | grep -i iommu
# Erwartet: "DMAR: ... IOMMU enabled"
```

| Test | Status |
|---|---|
| IOMMU aktiv (VT-d/AMD-Vi) | ⬜ |
| `xl dmesg` zeigt kein "iommu: disabled" | ⬜ |

### 5.3 krypt-daemon

```bash
systemctl status krypt-daemon
# Erwartet: active (running)

# Log prüfen
journalctl -u krypt-daemon --no-pager -n 30
# Erwartet: "daemon started", keine FATAL-Zeilen

# Socket vorhanden
ls -la /run/krypt/
# Erwartet: ipc.sock (root:root, 0600)
```

| Test | Status | Notizen |
|---|---|---|
| krypt-daemon läuft (`systemctl status`) | ⬜ | |
| Keine FATAL-Fehler in journalctl | ⬜ | |
| Socket `/run/krypt/ipc.sock` vorhanden | ⬜ | |
| Socket-Permissions: 0600, root:root | ⬜ | |

### 5.4 USB Kill-Switch (echte Hardware)

```bash
# Auth-Stick einrichten (falls nicht im Installer gemacht)
sudo krypt-stick --luks-dev /dev/sda2 setup --stick-dev /dev/sdb

# daemon.toml aktualisieren mit [[auth_sticks]]-Eintrag
# systemctl restart krypt-daemon

# Test:
journalctl -f -u krypt-daemon &
# → Stick abziehen
# Erwartet im Log: "USB kill-switch triggered"
# Erwartet: Bildschirm gesperrt / System suspended (je nach panic_level)

# → Stick wieder einstecken
# Erwartet: "Kill-switch device restored"
```

| Test | Status | Notizen |
|---|---|---|
| krypt-stick setup: kein Fehler | ⬜ | |
| daemon.toml serial korrekt | ⬜ | `krypt-stick --luks-dev /dev/sda2 list` |
| Stick abziehen → Kill-Switch ausgelöst | ⬜ | |
| Stick einstecken → System entsperrt | ⬜ | |
| Backup-Stick einrichten | ⬜ | `krypt-stick ... add-backup --stick-dev /dev/sdc` |
| Backup-Stick öffnet System | ⬜ | |
| Primär-Stick widerrufen → nur Backup funktioniert | ⬜ | `krypt-stick ... revoke 0` |

### 5.5 sys-gui VM

```bash
# sys-gui XL-Config prüfen
cat /etc/xen/krypt/sys-gui.cfg

# sys-gui starten
sudo xl create /etc/xen/krypt/sys-gui.cfg

# VM läuft
sudo xl list
# Erwartet: sys-gui in der Liste mit State "r" oder "b"

# Console öffnen
sudo xl console sys-gui
```

| Test | Status | Notizen |
|---|---|---|
| XL-Config für sys-gui vorhanden | ⬜ | |
| `xl create` fehlerfrei | ⬜ | |
| VM erscheint in `xl list` | ⬜ | |
| Hyprland startet in sys-gui | ⬜ | Benötigt funktionierenden GPU-Passthrough |

---

## 6. Erster Hardware-Boot (nach Installation)

Schritt-für-Schritt Checkliste für den ersten echten Boot auf Hardware.
Voraussetzung: Installer hat erfolgreich durchgelaufen, System ist auf Ziel-Disk installiert.

### 6.1 Boot-Sequenz

```bash
# Zuerst USB-Stick einlegen (Auth-Stick — falls eingerichtet)
# Dann System neu starten
```

| Schritt | Status | Notizen |
|---|---|---|
| GRUB-Menü erscheint (5s Timeout) | ⬜ | Krypt OS Eintrag? Xen-Eintrag? |
| LUKS-Prompt erscheint ODER Stick öffnet automatisch | ⬜ | krypt-Hook aktiv? |
| System bootet in dom0 | ⬜ | `uname -r` → linux-lts |
| `xl info` funktioniert | ⬜ | Xen läuft: `xen_major` etc. |
| `xl list` zeigt Domain-0 | ⬜ | |
| `systemctl status krypt-daemon` → active | ⬜ | |

### 6.2 dom0 Netzwerk-Isolation prüfen

```bash
# dom0 darf KEINEN Internetzugang haben
ip route
# Erwartet: leer oder nur loopback

ip addr
# Erwartet: nur lo (127.0.0.1) — keine eth0/eno1 IP

systemctl is-active NetworkManager
# Erwartet: inactive (disabled)

systemctl is-active systemd-networkd
# Erwartet: active
```

| Test | Status |
|---|---|
| `ip route` leer (keine Default-Route) | ⬜ |
| Physische Interfaces ohne IP (`ip addr`) | ⬜ |
| NetworkManager inactive | ⬜ |
| systemd-networkd active | ⬜ |

### 6.3 AppVM Disk-Images + krypt-vm-open

```bash
# Images prüfen
ls -lh /var/lib/krypt/vms/
# Erwartet: sys-gui.img, work.img, browser.img (je ~10GB)

ls /etc/krypt/keys/
# Erwartet: sys-gui.key, work.key, browser.key (0400, root:root)

# Erste VM starten
sudo krypt-vm-open sys-gui
# Erwartet: "Öffne sys-gui.img…" → "Starte VM: sys-gui" → keine Fehler

xl list
# Erwartet: sys-gui in der Liste mit State "r" oder "b"
```

| Test | Status | Notizen |
|---|---|---|
| AppVM-Images vorhanden (`ls /var/lib/krypt/vms/`) | ⬜ | |
| Keys vorhanden + 0400 (`ls -la /etc/krypt/keys/`) | ⬜ | |
| `krypt-vm-open sys-gui` fehlerfrei | ⬜ | LUKS öffnet, xl create startet |
| VM erscheint in `xl list` | ⬜ | |
| `/dev/mapper/sys-gui-root` vorhanden | ⬜ | `cryptsetup status sys-gui-root` |

### 6.4 IOMMU (GPU-Passthrough-Voraussetzung)

```bash
xl dmesg | grep -i iommu
# Erwartet: "IOMMU: ... enabled" oder "VT-d ... detected"

dmesg | grep -i iommu
# Erwartet: "DMAR: ... IOMMU enabled"
```

| Test | Status |
|---|---|
| IOMMU aktiv (VT-d/AMD-Vi) | ⬜ |
| `xl dmesg` zeigt kein "iommu: disabled" | ⬜ |

### 6.5 krypt-daemon IPC

```bash
ls -la /run/krypt/
# Erwartet: ipc.sock (root:root, 0600)

journalctl -u krypt-daemon -n 20 --no-pager
# Erwartet: "daemon started", keine FATAL-Zeilen

# Socket-Test (als root)
python3 -c "
import socket, json, struct
s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
s.connect('/run/krypt/ipc.sock')
msg = json.dumps({'type': 'list_vms_query'}).encode()
s.send(struct.pack('<I', len(msg)) + msg)
data = s.recv(4096)
print(json.loads(data[4:]))
"
# Erwartet: {'type': 'list_vms_response', 'vms': [...]}
```

| Test | Status | Notizen |
|---|---|---|
| Socket `/run/krypt/ipc.sock` vorhanden (0600) | ⬜ | |
| Keine FATAL-Zeilen in journalctl | ⬜ | |
| IPC-Test gibt ListVmsResponse zurück | ⬜ | |

---

## 7. GUI-Protokoll (krypt-gui)

> **Status Phase 13:** krypt-gui nutzt noch Stub-Pixel (trust_colored_frame).
> Echte Xen Grant-Table Integration ist Phase 14.

```bash
# Auf einem System mit laufendem Wayland in sys-gui:
WAYLAND_DISPLAY=wayland-1 krypt-gui &
# Erwartet: 3 farbige Rechtecke (work=blue, browser=green, vault=purple)
```

| Test | Status | Notizen |
|---|---|---|
| krypt-gui startet ohne Fehler | ⬜ | WAYLAND_DISPLAY gesetzt? |
| Wayland-Verbindung wird hergestellt | ⬜ | |
| 3 Fenster mit Trust-Farben erscheinen | ⬜ | Stub-Farben |
| Fenster reagieren auf Hyprland windowrulev2 | ⬜ | Border-Farben |
| `SIGTERM` beendet sauber | ⬜ | Kein Hang |

---

## 8. Dotfiles + Neovim

```bash
# Dotfiles installieren (als normaler Benutzer)
cd ~/krypt-os
./dotfiles/install.sh

# Neovim starten
nvim
# → lazy.nvim bootstrapped sich (erste Ausführung)
# → :Mason → rust-analyzer, pyright vorhanden
# → :checkhealth → keine kritischen Fehler
```

| Test | Status | Notizen |
|---|---|---|
| `dotfiles/install.sh` fehlerfrei | ⬜ | |
| `~/.config/nvim` → `dotfiles/neovim` Symlink | ⬜ | |
| `~/.config/hypr/` → Symlink | ⬜ | |
| `~/.config/waybar/` → Symlink | ⬜ | |
| Neovim startet ohne Fehler | ⬜ | |
| lazy.nvim installiert alle Plugins | ⬜ | |
| rust-analyzer LSP aktiv für .rs Dateien | ⬜ | |
| pyright LSP aktiv für .py Dateien | ⬜ | |
| Telescope fuzzy search funktioniert | ⬜ | `<leader>ff` |
| Catppuccin Mocha Theme aktiv | ⬜ | |
| Krypt-Violet `#9d4edd` als @type-Farbe | ⬜ | |

---

## 9. Sicherheits-Checkliste (vor Produktiveinsatz)

| Check | Status | Befehl |
|---|---|---|
| LUKS-Header-Backup erstellt | ⬜ | `cryptsetup luksHeaderBackup /dev/sda2 --header-backup-file luks-header.bak` |
| Backup-Stick eingerichtet | ⬜ | `krypt-stick ... add-backup` |
| dom0 hat keinen Netzwerkzugang | ⬜ | `ip route` in dom0 muss leer sein |
| IOMMU aktiv | ⬜ | `xl dmesg | grep -i iommu` |
| dom0-RAM begrenzt (max 4096M) | ⬜ | `xl info | grep free_memory` |
| Alle AppVM-Disk-Images verschlüsselt | ⬜ | Pro VM: `cryptsetup status /dev/mapper/<vm>-root` |
| `krypt-daemon` läuft | ⬜ | `systemctl is-active krypt-daemon` |
| Audit-Log konfiguriert | ⬜ | Zukünftig: krypt-daemon Audit-Modul |
| Hyprland windowrulev2 Trust-Borders aktiv | ⬜ | Roter Rand für red-VMs? |
