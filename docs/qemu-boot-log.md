# Krypt OS — QEMU Boot-Test Log

Dokumentiert den ISO-Build, QEMU-Boot und alle aufgetretenen Fehler + Fixes.
Wird bei jedem Test-Run aktualisiert.

Status-Legende: ✅ bestanden · ❌ fehlgeschlagen · ⚠️ workaround · ⬜ noch nicht getestet

---

## Voraussetzungen (einmalig einrichten)

```bash
# Auf Arch Linux:
sudo pacman -S archiso qemu-full edk2-ovmf mtools squashfs-tools

# Verifikation:
mkarchiso --version    # z.B. archiso 88
qemu-system-x86_64 --version   # QEMU 11.0.0
ls /usr/share/edk2/x64/OVMF_CODE.4m.fd   # OVMF vorhanden
```

---

## ISO-Build

```bash
cd ~/krypt-os
sudo ./build/build.sh --clean 2>&1 | tee /tmp/krypt-build.log
```

Erwartete Ausgabe (Ende):
```
[krypt] ✓ ISO fertig!
  Datei:     krypt-os-<version>.iso
  Größe:     800M – 3G
  SHA256:    <hash>
```

### Build-Ergebnisse

| Schritt | Status | Zeit | Notizen |
|---|---|---|---|
| `cargo build --release` | ⬜ | | |
| `profiledef.sh` generiert | ⬜ | | |
| Pakete heruntergeladen | ⬜ | | pacstrap im squashfs |
| Binaries eingebunden | ⬜ | krypt-daemon, krypt-stick, krypt-gui | |
| ISO erzeugt in `dist/` | ⬜ | | |
| SHA256 stimmt | ⬜ | | |

### Build-Fehler und Fixes

*(Hier werden Fehler dokumentiert wenn sie auftreten)*

---

## Test 1: Live-ISO in QEMU (Installer-Test)

```bash
./build/test-qemu.sh --live
```

QEMU öffnet ein GTK-Fenster. Erwartete Boot-Sequenz:

### 1.1 GRUB-Menü (erscheint nach ~5s)

```
┌────────────────────────────────────────────────────────┐
│          K R Y P T  O S                                │
│                                                        │
│  > Krypt OS Live                                       │
│    Krypt OS Live (fallback initramfs)                  │
│    UEFI Shell                                          │
│                                                        │
│  5 seconds until auto-boot                             │
└────────────────────────────────────────────────────────┘
```

| Test | Status | Notizen |
|---|---|---|
| GRUB-Menü erscheint | ⬜ | Krypt-Theme oder Standard? |
| Live-Eintrag vorhanden | ⬜ | |
| GRUB-Timeout abgelaufen → Auto-Boot | ⬜ | |

### 1.2 Plymouth-Splash (kurz, ~2s)

```
[    0.XXX] Plymouth: Theme=krypt
```

| Test | Status | Notizen |
|---|---|---|
| Plymouth erscheint kurz | ⬜ | Nur bei plymouth-installiert |
| Kein Plymouth-Crash | ⬜ | |

### 1.3 Live-System gestartet

```bash
# Im Live-System landet root via agetty-autologin auf tty1.
# ~/.zlogin ruft ~/.automated_script.sh, das den Installer startet.
# Manuell neustarten (z. B. wenn beendet wurde):
sudo krypt-install

# Installer durch Cmdline-Param überspringen:  krypt.installer=off
```

| Test | Status | Notizen |
|---|---|---|
| Arch-Live-System bootet | ⬜ | Kein Kernel-Panic |
| `krypt-install` Command verfügbar | ⬜ | `which krypt-install` |
| Installer-TUI erscheint auf tty1 | ⬜ | |
| TUI zeigt Krypt ASCII-Logo | ⬜ | |
| Textual-Import funktioniert | ⬜ | `python3 -c "import textual"` |

### 1.4 Netzwerk im Live-System

```bash
# Netzwerk wird für pacstrap gebraucht
ping -c 1 archlinux.org
# Erwartung: pong (User-Mode-Networking in QEMU)
```

| Test | Status | Notizen |
|---|---|---|
| Netzwerk verfügbar (`ip addr`) | ⬜ | DHCP via QEMU user-mode? |
| `ping archlinux.org` antwortet | ⬜ | |

---

## Test 2: Installation durchlaufen (--install)

```bash
./build/test-qemu.sh --install
```

QEMU startet mit ISO + 40GB virtuellem Disk. Installer auf tty1.

### 2.1 Installer-Schritte

| Schritt | Status | Notizen |
|---|---|---|
| Welcome-Screen erscheint | ⬜ | ASCII-Logo sichtbar? |
| "Starten" Button klickbar | ⬜ | |
| Disk-Screen: virtuelle Disk erscheint | ⬜ | `/dev/vda` via virtio-blk |
| Disk auswählen → Weiter | ⬜ | |
| LUKS2-Passphrase: Stärke-Indikator | ⬜ | |
| Passphrase ≥ 20 Zeichen → Weiter | ⬜ | |
| Installation: sgdisk | ⬜ | |
| Installation: cryptsetup luksFormat | ⬜ | Argon2id, dauert 30–60s |
| Installation: pacstrap base | ⬜ | Netzwerk muss aktiv sein |
| Installation: pacstrap xen + hyprland | ⬜ | |
| Installation: grub-install | ⬜ | |
| Installation: mkinitcpio -P | ⬜ | krypt-Hook eingebunden? |
| USB-Screen: Überspringen | ⬜ | Kein echter Stick in QEMU |
| VM-Screen: sys-gui + work + browser | ⬜ | |
| VM-Images erstellt | ⬜ | `ls /mnt/var/lib/krypt/vms/` |
| Finish-Screen: "Neu starten" | ⬜ | |
| Reboot → kein ISO mehr (eject) | ⬜ | |

---

## Test 3: Installiertes System booten (--boot-installed)

```bash
./build/test-qemu.sh --boot-installed
```

### 3.1 GRUB-Menü des installierten Systems

```
┌────────────────────────────────────────────────────────┐
│  Krypt OS — Xen Hypervisor (Linux-LTS)                 │  ← NICHT wählen in QEMU
│  Krypt OS — Linux LTS                                  │  ← Diese Zeile wählen
│  Krypt OS — Linux LTS (fallback)                       │
└────────────────────────────────────────────────────────┘
```

**WICHTIG für QEMU:** Xen-Eintrag wählen würde scheitern (Xen braucht Bare-Metal).
Linux-LTS Eintrag wählen, dann `e` drücken und `krypt_timeout=15` zum linux-Parameter hinzufügen:

```
# In GRUB editor (Taste 'e'):
linux   /boot/vmlinuz-linux-lts root=/dev/mapper/krypt-root \
        cryptdevice=UUID=<UUID>:krypt-root \
        krypt_luks_uuid=<UUID> krypt_luks_name=krypt-root \
        krypt_timeout=15   ← DIESE ZEILE HINZUFÜGEN
# Dann Ctrl+X zum Booten
```

| Test | Status | Notizen |
|---|---|---|
| GRUB-Menü des installierten Systems | ⬜ | Xen + Linux Einträge? |
| Linux-LTS wählbar | ⬜ | |
| GRUB-Editor mit 'e' öffenbar | ⬜ | |
| `krypt_timeout=15` hinzugefügt | ⬜ | |

### 3.2 krypt initramfs Hook

```
  krypt: kein USB-Stick gefunden — warte...
  ██╗  ██╗██████╗ ...
  Authentication required. Insert your Krypt stick.
  (Fallback auf Passphrase nach 15s)

  [nach 15s:]
  krypt: Timeout nach 15s — Fallback auf encrypt-Hook.
  krypt: LUKS-Passphrase-Eingabe folgt.

  Enter passphrase for /dev/vda2:   ← encrypt-Hook
```

| Test | Status | Notizen |
|---|---|---|
| krypt-Hook gestartet | ⬜ | ASCII-Banner erscheint? |
| Timeout nach 15s | ⬜ | |
| encrypt-Hook Passphrase-Prompt | ⬜ | |
| LUKS öffnet mit Passphrase | ⬜ | |

### 3.3 dom0 Boot

```bash
# Nach Login als root:
uname -r                     # linux-lts Kernel
xl info 2>/dev/null || echo "Xen nicht verfügbar (QEMU-Normal)"
systemctl status krypt-daemon
ip route                     # leer (dom0 Isolation)
ls /var/lib/krypt/vms/       # sys-gui.img, work.img, browser.img
```

| Test | Status | Notizen |
|---|---|---|
| dom0 bootet ohne Kernel-Panic | ⬜ | |
| Login als root möglich | ⬜ | |
| `krypt-daemon.service` active | ⬜ | `systemctl status krypt-daemon` |
| `/run/krypt/ipc.sock` vorhanden | ⬜ | |
| `ip route` leer (dom0 Isolation) | ⬜ | |
| AppVM-Images vorhanden | ⬜ | `ls /var/lib/krypt/vms/` |
| `krypt-vm-open` verfügbar | ⬜ | `which krypt-vm-open` |
| Keine FATAL-Zeilen in journalctl | ⬜ | `journalctl -u krypt-daemon -n 30` |

---

## Test 4: USB Kill-Switch Simulation

Voraussetzung: System installiert, Stick mit echtem LUKS-Key vorbereitet.

```bash
# Erst: USB-Stick-Image vorbereiten (auf dem installierten System)
# 1. /dev/vdb als "Stick" in QEMU hinzufügen (--with-stick)
# 2. Im laufenden System:
sudo krypt-stick --luks-dev /dev/vda2 setup --stick-dev /dev/vdb

# 3. QEMU mit Stick neu starten:
./build/test-qemu.sh --boot-installed --with-stick build/krypt-test-stick.img
```

### Kill-Switch erstellen für QEMU-Test:

```bash
# Lokales Stick-Image anlegen (1MB, kein Filesystem nötig)
dd if=/dev/zero bs=1M count=1 of=build/krypt-test-stick.img

# Krypt-Key (64 Byte) an Offset 512 schreiben — MUSS dem LUKS-Key entsprechen
# (Normalerweise macht krypt-stick das. Für manuellen Test:)
# dd if=/path/to/key bs=64 count=1 seek=512 of=build/krypt-test-stick.img bs=1
```

| Test | Status | Notizen |
|---|---|---|
| QEMU mit --with-stick gestartet | ⬜ | USB-Stick erkannt? |
| krypt-Hook findet Stick | ⬜ | LUKS öffnet ohne Passphrase |
| Stick abziehen (QEMU monitor: `device_del usbstick0`) | ⬜ | |
| Kill-Switch ausgelöst | ⬜ | journalctl: "USB kill-switch triggered" |
| Stick einstecken → System entsperrt | ⬜ | |

---

## Häufige Fehler und Fixes

### F1: "Package not found" beim ISO-Build

```
error: target not found: <paket>
```

**Fix:** Paket aus `build/packages.x86_64` entfernen oder korrekten Namen finden:
```bash
pacman -Ss <paket>   # Suche im Arch-Repo
```

### F2: krypt-Hook hängt (kein Timeout)

**Symptom:** Nach dem Boot bleibt das System beim ASCII-Banner hängen, kein Passphrase-Prompt.

**Workaround:** Im GRUB-Editor `krypt_timeout=15` zum linux-Cmdline hinzufügen.

**Ursache:** Kein USB-Stick im System, `krypt_timeout` nicht gesetzt.

**Fix (dauerhaft):** Nach der Installation als root:
```bash
# /etc/default/grub editieren:
GRUB_CMDLINE_LINUX="... krypt_timeout=30"
grub-mkconfig -o /boot/grub/grub.cfg
```

### F3: GRUB startet nicht / "No bootable device"

**Symptom:** OVMF zeigt "Boot Manager" aber kein GRUB.

**Mögliche Ursachen:**
- `grub-install --target=x86_64-efi` fehlgeschlagen
- EFI-Partition nicht korrekt gemountet (/boot/efi)
- OVMF findet keine EFI-Partition

**Debug:**
```bash
# Im Live-System nach fehlgeschlagener Installation:
ls /mnt/boot/efi/EFI/krypt/grubx64.efi
ls /mnt/boot/grub/grub.cfg
```

### F4: mkinitcpio -P schlägt fehl: krypt-Hook nicht gefunden

```
ERROR: module not found: `krypt'
```

**Ursache:** `/etc/initcpio/hooks/krypt` oder `/etc/initcpio/install/krypt` fehlt.

**Fix in install.py:** Hook-Dateien werden aus dem Live-System kopiert. Im Live-ISO müssen sie in `/etc/initcpio/` vorhanden sein. Prüfen:
```bash
ls /etc/initcpio/hooks/krypt    # muss existieren
ls /etc/initcpio/install/krypt  # muss existieren
```

Wenn nicht vorhanden: Build-Script prüfen (`install -Dm644 initramfs/hooks/krypt ...`).

### F5: cryptsetup: "No key available with this passphrase"

**Ursache:** Die AppVM-Disk-Images wurden mit einem anderen Key formatiert als der Stick enthält.

**Fix:** Disk-Image neu formatieren (löscht Daten) oder korrekten Key-File nutzen:
```bash
cryptsetup open --key-file /etc/krypt/keys/work.key /var/lib/krypt/vms/work.img work-root
```

### F6: python-textual ImportError im Live-System

```
ModuleNotFoundError: No module named 'textual'
```

**Fix A:** Paket im Live-System installieren:
```bash
pip install --break-system-packages textual
```

**Fix B:** `python-textual` zu `build/packages.x86_64` hinzufügen (sollte bereits drin sein).
Prüfen ob es im Arch-Repo verfügbar ist:
```bash
pacman -Si python-textual
```

### F7: krypt-daemon startet, xl-Befehle scheitern (QEMU ohne Xen)

```
xl: command not found
# oder:
ERROR: Unable to connect to xenstore
```

**Erklärung:** Das ist normal im QEMU-Test ohne Xen als Hypervisor. krypt-daemon hat `Wants=xenstore.service` (nicht `Requires=`), startet also trotzdem. Die VM-Management-Funktionen sind ohne Xen nicht verfügbar.

**Für den QEMU-Test:** krypt-daemon läuft, IPC-Socket vorhanden, Policy-Engine aktiv — nur xl-Calls schlagen fehl. Das ist kein Blocker für den alpha-Test.

### F8: Virtio-Block-Disk nicht als /dev/vda erkannt

**Symptom:** Installer zeigt keine Disk in der Liste, oder `/dev/vda` fehlt.

**Fix:** QEMU-Kommando prüft ob `virtio-blk-pci` korrekt konfiguriert ist:
```bash
# Prüfen:
qemu-system-x86_64 ... -device virtio-blk-pci,drive=disk0 ...
# statt:
qemu-system-x86_64 ... -drive file=...,if=virtio ...
```

---

## Bekannte QEMU-Einschränkungen

| Einschränkung | Auswirkung | Workaround |
|---|---|---|
| Kein Xen als Hypervisor | `xl` commands scheitern | Linux-LTS Eintrag in GRUB wählen |
| USB-Stick-Simulation via QEMU | krypt-Hook findet Stick manchmal nicht sofort | `krypt_timeout=15` + 10s warten |
| Kein GPU-Passthrough | sys-gui VM kann nicht starten | Xen-Funktionen auf echter Hardware testen |
| Netzwerk via User-Mode | Langsamer als TAP (pacstrap dauert länger) | Normal, kein Fix nötig |
| krypt_timeout default 0 | Hook hängt ohne Stick | `krypt_timeout=15` in GRUB editor |

---

## Test-Ergebnis-Zusammenfassung

Datum: _____
Build-Version: _____
Host-System: _____

| Phase | Status | Fehler | Fix angewendet |
|---|---|---|---|
| ISO-Build | ⬜ | | |
| Live-ISO bootet | ⬜ | | |
| Installer-TUI startet | ⬜ | | |
| Installation durchläuft | ⬜ | | |
| Installed System bootet | ⬜ | | |
| krypt-Hook (mit Timeout) | ⬜ | | |
| LUKS öffnet (Passphrase) | ⬜ | | |
| krypt-daemon active | ⬜ | | |
| dom0 Netzwerk-Isolation | ⬜ | | |
| AppVM-Images vorhanden | ⬜ | | |
| krypt-vm-open ausführbar | ⬜ | | |
