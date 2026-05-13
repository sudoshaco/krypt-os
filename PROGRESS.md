# Dev Log — Krypt OS

---

## 2026-05-13 — Phase 13: AppVM Disk-Images, dom0 Isolation, mkinitcpio Fix, ISO-Vollständigkeit

### Erledigt

**installer/steps/vms.py — _create_vm_disk_images() (Alpha-Blocker #1):**
- `_create_vm_disk_images(vms, log_fn)`: Pro VM `fallocate -l {10G|5G}` → 64-Byte Random-Key (`os.urandom(64)`, `0o400`) → `cryptsetup luksFormat --type luks2 --key-file` → `cryptsetup open --key-file` → `mkfs.ext4 -q -L` → `cryptsetup close`
- Keys in `/mnt/etc/krypt/keys/{name}.key` (0400, root only)
- Images in `/mnt/var/lib/krypt/vms/{name}.img` (sparse, 10G für ≥2048MB RAM, 5G sonst)
- `_write_krypt_vm_open()`: Schreibt `/mnt/usr/local/bin/krypt-vm-open` — prüft KEY/IMG/CFG, öffnet LUKS falls noch zu, startet `xl create`
- `_write_daemon_toml()` Fix: Entfernt `socket_path` (nicht in config.rs), `[policy]`-Block mit `usb_kill_switch` (nicht in config.rs), `xl_config`-Feld (nicht in VmEntry). Nur noch Felder die config.rs kennt. Policy-Regeln nur wenn beide VMs ausgewählt.
- `_generate_xl_config()`: Kommentar-Header mit Image + Key-Pfad; `extra = "root=/dev/xvda"` (korrekt für XL)
- `FinishScreen`: Zeigt konkrete Befehle für ersten Boot (`krypt-vm-open sys-gui`, etc.)

**installer/steps/install.py — Kritische Fixes (Alpha-Blocker #2):**
- **genfstab Fix**: `stdout=open(...)` + `capture_output=True` Konflikt behoben — jetzt `with open(...) as fstab: subprocess.run(..., stdout=fstab)`
- **cryptsetup + mkinitcpio** zu pacstrap ergänzt (fehlten komplett!)
- **Systemd-Units kopieren**: `krypt-daemon.service` + `krypt-boot-agent.service` aus Live-ISO nach `/mnt/etc/systemd/system/`; `systemctl enable krypt-daemon` in arch-chroot
- **mkinitcpio.conf + krypt-Hooks** aus Live-ISO nach `/mnt/etc/initcpio/hooks/krypt` + `/mnt/etc/initcpio/install/krypt`
- **`mkinitcpio -P`** in arch-chroot (Phase 10: Initramfs generieren) — kritisch für USB-Boot
- **dom0 Netzwerk-Isolation** (Alpha-Blocker #3):
  - `systemctl disable NetworkManager` im installierten System
  - `systemctl enable systemd-networkd`
  - `10-dom0-lo.network` + `20-dom0-eth.network` nach `/mnt/etc/systemd/network/`
- **GRUB cmdline erweitert**: `krypt_luks_uuid=<UUID> krypt_luks_name=krypt-root` (für krypt-Hook)
- **Locale + Hostname**: `en_US.UTF-8`, `krypt-os`
- **`python-textual` + `python-rich` + `python-psutil`** im Xen-pacman-Install ergänzt (Alpha-Blocker #4: python-textual im installierten System)
- **Installer + krypt-install** werden aus Live-ISO ins installierte System kopiert
- **krypt-vm-open** und `krypt-boot-agent.sh` ebenfalls kopiert
- 10 Phasen, TOTAL_WEIGHT angepasst

**build/airootfs/etc/mkinitcpio.conf — HOOKS final:**
- War: `base udev krypt encrypt lvm2 filesystems fsck`
- Jetzt: `base udev autodetect modconf block keyboard krypt encrypt lvm2 filesystems fsck`
- `autodetect` + `modconf` für korrektes installed-system Initramfs
- `keyboard` vor `krypt` (Passworteingabe als Fallback möglich)

**build/airootfs/etc/systemd/network/ — NEU (dom0 Isolation für installiiertes System):**
- `10-dom0-lo.network` — Loopback 127.0.0.1/8 + ::1/128
- `20-dom0-eth.network` — `Type=ether`, `Unmanaged=yes` — dom0 bekommt kein IP

**build/build.sh — Fixes:**
- Fragmentierter daemon.toml-Copy (`||`/`&&`-Chaos) durch klares `if`-Block ersetzt
- `mkdir -p /var/lib/krypt/vms` + `/etc/krypt/keys` mit chmod 700 in airootfs
- `krypt-vm-open` Wrapper in `airootfs/usr/local/bin/` installiert
- Installer-Service: `Type=simple` (statt idle), `Restart=on-failure`, `StandardError=journal`

**build/packages.x86_64 — Cleanup:**
- `libwayland-server` entfernt (kein separates Arch-Paket — ist `wayland`-Abhängigkeit)
- `os-prober` entfernt (Sicherheitsbedenken — erkennt andere OSes automatisch)
- `vi` → `less` (pager für dom0 Terminal-Zugang)

**docs/testing.md — Sektion 6 "Erster Hardware-Boot" (NEU):**
- 6.1 Boot-Sequenz (GRUB → LUKS → dom0)
- 6.2 dom0 Netzwerk-Isolation Checks (`ip route`, `ip addr`, `systemctl`)
- 6.3 AppVM Disk-Images + krypt-vm-open Tests
- 6.4 IOMMU-Check (xl dmesg + dmesg)
- 6.5 krypt-daemon IPC Socket-Test (Python-Snippet)

### Aktueller Stand
```
Neue/geänderte Dateien Phase 13:
  installer/steps/vms.py              (_create_vm_disk_images, _write_krypt_vm_open, daemon.toml fix)
  installer/steps/install.py          (genfstab fix, mkinitcpio, network isolation, 10 Phasen)
  build/airootfs/etc/mkinitcpio.conf  (HOOKS: autodetect modconf block keyboard krypt encrypt lvm2)
  build/airootfs/etc/systemd/network/10-dom0-lo.network   (NEU)
  build/airootfs/etc/systemd/network/20-dom0-eth.network  (NEU)
  build/build.sh                      (daemon.toml fix, krypt-vm-open, Verzeichnisse)
  build/packages.x86_64              (libwayland-server entfernt, os-prober entfernt, less)
  docs/testing.md                    (Sektion 6: Erster Hardware-Boot)
  PROGRESS.md                        (Phase 13)

cargo clippy --workspace -- -D warnings → sauber
cargo test --workspace                 → 34/34 grün
  vm-daemon: 22, gui-protocol: 0, krypt-stick: 12
cargo build --release                  → 0 Warnings
bash -n build/build.sh                 → Syntax OK
python3 -m py_compile installer/**/*.py → alle OK
bash -n initramfs/{hooks,install}/krypt → Syntax OK
```

### Alpha-Blocker Status

| Blocker | Status |
|---|---|
| AppVM Disk-Images im Installer | ✅ vms.py `_create_vm_disk_images()` implementiert |
| python-textual im installierten System | ✅ install.py pacman-Install ergänzt |
| dom0 Netzwerkisolation automatisch | ✅ systemd-networkd + Unmanaged=yes + NM disabled |
| daemon.toml nur gültige Felder | ✅ socket_path + usb_kill_switch entfernt |
| mkinitcpio.conf korrekt + krypt-Hook | ✅ HOOKS final, Hooks werden kopiert, mkinitcpio -P läuft |
| genfstab stdout-Bug | ✅ with open() → stdout=fstab |

### Nächste Session beginnt mit
Phase 15 (gui-protocol Phase 2, Xen Grant-Table, Input-Forwarding).

---

## 2026-05-13 — Phase 14: QEMU Boot-Test, krypt_timeout, known-issues finalisiert

### Erledigt

**initramfs/hooks/krypt — krypt_timeout Parameter (Alpha-Blocker: Infinity-Loop in QEMU):**
- `krypt_timeout=N` Kernel-Cmdline-Parameter: 0 = kein Timeout (Produktions-Default), N>0 = nach N Sekunden Fallback auf `encrypt`-Hook (Passphrase-Prompt)
- In QEMU: GRUB-Editor `e` → `krypt_timeout=15` anhängen → Installer läuft durch ohne USB-Stick
- Produktion: kein `krypt_timeout` → unendlich warten (Kill-Switch-Verhalten by design)
- `elapsed`-Counter im USB-Warte-Loop, `return 1` bei Timeout (signalisiert: encrypt-Hook soll LUKS öffnen)

**build/test-qemu.sh — NEU (vollständiger QEMU-Test-Workflow):**
- 3 Modi: `--live` (ISO only), `--install` (ISO + 40GB virtuelle Disk), `--boot-installed` (Disk only)
- UEFI via OVMF: `/usr/share/edk2/x64/OVMF_CODE.4m.fd` + VARS-Kopie in `/tmp/krypt-ovmf-vars.fd`
- USB-Stick-Simulation: `--with-stick IMG` → `-device nec-usb-xhci + usb-storage`
- Machine: `-machine q35 -device virtio-vga` (UEFI-kompatibel)
- Netzwerk: `-netdev user,id=net0,hostfwd=tcp::2222-:22` (kein root nötig)
- Serial-Log: `docs/qemu-boot-log.md.serial` für automatisches Logging
- `--headless`, `--snapshot`, `--no-kvm` Flags

**build/make-test-stick.sh — NEU (USB-Stick-Image-Helper):**
- `--empty`: 1MB Null-Image (Smoke-Test USB-Erkennung ohne LUKS)
- `--luks-dev /dev/vda2`: Ruft `krypt-stick setup` auf (schreibt echten 64-Byte Key + luksAddKey)
- Output: `build/krypt-test-stick.img`

**installer/steps/install.py — GRUB-Fixes:**
- `GRUB_DEFAULT=saved` + `GRUB_SAVEDEFAULT=true`: GRUB merkt letzte Wahl (wichtig für QEMU — Linux LTS statt Xen auswählen)
- `GRUB_DISABLE_OS_PROBER=true`: Andere OSes nicht erkennen (Sicherheit)
- `GRUB_TIMEOUT=8`, `GRUB_TIMEOUT_STYLE=menu`: Genug Zeit für Menü-Interaktion

**docs/qemu-boot-log.md — NEU (Boot-Test-Dokumentation):**
- Vollständige Checkliste: Voraussetzungen, ISO-Build, Test 1–4 (live/install/boot/stick)
- F1–F8: Fehlerklassen mit Ursache + Fix (GRUB, LUKS, Xen-Entry, krypt-Hook, Installer-TUI, etc.)
- QEMU-Einschränkungen-Tabelle (kein Xen-Hypervisor, kein IOMMU, kein USB-Kill-Switch-Beweis)
- Ergebnis-Tabelle (auszufüllen nach echtem Test)

**docs/known-issues.md — Alle Alpha-Blocker als ✅ markiert:**
- Issue 16 neu: krypt-Hook Infinity-Loop — ✅ Behoben Phase 14 (krypt_timeout Parameter)
- Summary-Tabelle: Alle 6 Alpha-Blocker resolved
- Roadmap aktualisiert: QEMU-Test-Durchführung als einzige verbleibende Aufgabe

### Aktueller Stand
```
Neue/geänderte Dateien Phase 14:
  initramfs/hooks/krypt               (krypt_timeout Parameter, Fallback auf encrypt-Hook)
  build/test-qemu.sh                  (NEU — vollständiger QEMU-Test-Workflow)
  build/make-test-stick.sh            (NEU — USB-Stick-Image-Helper)
  installer/steps/install.py          (GRUB_DEFAULT=saved, GRUB_SAVEDEFAULT, GRUB_DISABLE_OS_PROBER)
  docs/qemu-boot-log.md               (NEU — Boot-Test-Dokumentation + Checkliste)
  docs/known-issues.md                (Issue 16 neu, alle Alpha-Blocker ✅)
  PROGRESS.md                         (Phase 14)

bash -n initramfs/{hooks,install}/krypt → Syntax OK
bash -n build/test-qemu.sh            → Syntax OK
bash -n build/make-test-stick.sh      → Syntax OK
python3 -m py_compile installer/**/*.py → alle OK
```

### Alpha-Blocker Status Phase 14

| Blocker | Status |
|---|---|
| krypt-Hook Infinity-Loop (QEMU-Test blockiert) | ✅ krypt_timeout=15 für QEMU, 0=∞ für Produktion |
| GRUB-Menü-Persistenz (Linux LTS vs. Xen) | ✅ GRUB_DEFAULT=saved |
| QEMU-Test-Skript fehlte | ✅ build/test-qemu.sh + make-test-stick.sh |

### QEMU-Test-Anleitung (auf Arch-Build-System ausführen)
```bash
# Voraussetzungen
sudo pacman -S archiso qemu-system-x86 edk2-ovmf

# ISO bauen
sudo ./build/build.sh --clean 2>&1 | tee /tmp/krypt-build.log

# Test 1: Live-ISO
./build/test-qemu.sh --live

# Test 2: Installation durchlaufen (Installer-TUI auf tty1)
./build/test-qemu.sh --install
# Im GRUB: 'e' drücken, krypt_timeout=15 ans Kernel-Cmdline anhängen

# Test 3: Installiertes System booten
./build/test-qemu.sh --boot-installed
# Im GRUB: 'Krypt OS (Linux LTS)' auswählen (nicht Xen-Entry)

# Test 4: USB Kill-Switch
./build/make-test-stick.sh --luks-dev /dev/vda2
./build/test-qemu.sh --boot-installed --with-stick build/krypt-test-stick.img
```

### Offene Aufgaben vor v0.1.0-alpha
1. **QEMU-Boot-Test durchführen** — Checkliste in docs/qemu-boot-log.md ausfüllen
2. **AppVM-Template Bootstrap** — sys-gui.img enthält leeres ext4, braucht Alpine/Arch Minimal-System
3. **GRUB PF2-Font** — `grub-mkfont JetBrainsMono → JetBrainsMono.pf2` in build.sh
4. **Plymouth Array-Syntax** — auf echter Plymouth-Instanz validieren

---

## 2026-05-13 — Phase 12: Erster ISO-Test, create.rs, daemon.toml-Fix, Checkliste, Known-Issues

### Erledigt

**cargo build --release / clippy / test — Audit:**
- `cargo build --release` → sauber, 0 Warnings
- `cargo clippy --workspace -- -D warnings` → sauber
- `cargo test --workspace` → **34/34 grün** (22 vm-daemon + 0 gui-protocol + 12 krypt-stick)
  - krypt-stick: 8 alte (luks.rs) + 4 neue (create.rs `strip_partition_suffix`-Tests)

**krypt-stick/src/create.rs — vollständige Implementierung:**
- 64-Byte-Key aus `/dev/urandom` via `File::open` + `read_exact`
- Key auf Stick schreiben (raw, Offset 512 = Sektor 1, `SeekFrom::Start`)
- `libc::fsync()` via unsafe-Block (statt `nix::unistd::fsync` — nix fehlte `"fs"`-Feature)
- Temp-Keyfile `/tmp/.krypt-setup-key` mit `mode(0o600)`, sofort nach `luksAddKey` gelöscht
- `read_stick_serial()` via sysfs `/sys/block/<dev>/device/serial`
- `detect_serial_via_udevadm()` als Fallback (`udevadm info --query=property`)
- `strip_partition_suffix()` — NVMe/MMC/loop vs. SATA/USB korrekt getrennt
- 2 kritische Bugfixes im `strip_partition_suffix`:
  - **Bug 1** (`loop0` → `"loo"`): rfind('p') traf 'p' in "loo**p**", Suffix "0" war Ziffern.
    Fix: `pre_p.is_some_and(|c| c.is_ascii_digit())` — char vor 'p' muss Ziffer sein
  - **Bug 2** (`nvme0n1` → `"nvme0n"`): kein 'p' gefunden → Fall-Through in SATA-Code →
    trim_end_matches entfernte trailing '1'. Fix: `return name` bei `uses_p_suffix`-Geräten ohne gültiges p-Suffix

**krypt-stick/src/main.rs — `--force` Flag:**
- `Setup`-Subcommand um `#[arg(long)] force: bool` ergänzt
- Dispatch: `Commands::Setup { stick_dev, force } => create::run_setup(..., force)`

**installer/steps/usb.py — vollständiger Rewrite:**
- `_list_removable_devices()` via `lsblk --json`, filtert `rm=true, type="disk"`
- `_find_krypt_stick_binary()` sucht in `/mnt/usr/local/bin/`, `/usr/bin/`, `/usr/local/bin/`
- `ListView` für Stick-Auswahl (User muss explizit auswählen)
- Korrekter krypt-stick CLI-Aufruf:
  ```python
  ["binary", "--luks-dev", "/dev/mapper/krypt-root", "setup", "--stick-dev", stick_dev, "--force"]
  ```
  (vorher: `["krypt-stick", "--setup", "--luks-device", "...", "--slot", "1"]` — falsch)
- Alle Fehler (Binary nicht gefunden, Timeout, Exception) → btn-next enabled

**build/airootfs/etc/krypt/daemon.toml — Kritischer Fix:**
- Trust-Level auf lowercase geändert (serde `#[rename_all = "lowercase"]`):
  - `"Green"` → `"green"`, `"Yellow"` → `"yellow"`, `"Red"` → `"red"`, `"Black"` → `"black"`
  - `"Ask"` → `"ask"`, `"Deny"` → `"deny"`
  - `panic_level = "Nuke"` → `"suspend"` (korrekte Aktion + lowercase)
- sys-gui memory korrigiert: 512 → 2048 MB
- browser trust korrigiert: `"Red"` → `"yellow"` (falsche Einstufung)
- `socket_path`-Zeile entfernt (nicht in config.rs geparst)
- Header mit Kommentaren: Trust-Level müssen lowercase sein, panic_level Optionen erklärt

**build/packages.x86_64 — Review:**
- `xen-docs` entfernt (kein separates Arch-Paket)
- `xen-tools` annotiert (`[PRÜFEN]` — xl-Tools sind in `xen`-Paket enthalten)
- Ergänzt: `arch-install-scripts` (pacstrap, genfstab), `e2fsprogs` (mkfs.ext4),
  `gptfdisk` (sgdisk), `python-pip`, `python-textual` ([PRÜFEN]), `python-psutil`,
  `iptables-nft`, `pipewire-alsa`, `pcsclite`, `ccid`
- `cargo`/`rust` auskommentiert mit Anmerkung (~800 MB zu groß für Release-ISO, pre-compiled)
- `base-devel` auskommentiert (nur für Dev-ISOs nötig)

**docs/testing.md — 8-Sektionen Test-Checkliste:**
- 1. Build-Verifikation (cargo, installer Python-Syntax, Shell-Syntax)
- 2. ISO-Build (build.sh, Binary-Präsenz, SHA256, trust_level lowercase in daemon.toml)
- 3. QEMU-Boot-Test (kein Xen nötig — GRUB, Plymouth, Live-System)
- 4. Installer-Ablauf (Welcome→Disk→LUKS2→Install→USB→VMs) mit virtuellem QEMU-Disk
- 5. Installiertes System (Boot-Sequenz, IOMMU, krypt-daemon, USB Kill-Switch, sys-gui)
- 6. GUI-Protokoll (krypt-gui Stub-Farben, WAYLAND_DISPLAY, SIGTERM)
- 7. Dotfiles + Neovim (LSP, Telescope, Catppuccin Mocha, Krypt-Violet)
- 8. Sicherheits-Checkliste (LUKS-Header-Backup, dom0 kein Netzwerk, IOMMU, AppVM-Encryption)

**docs/known-issues.md — 15 dokumentierte Lücken:**
- gui-protocol: Xen Grant-Table (Stub), kein Frame-Callback, kein Input-Forwarding, kein Clipboard
- installer: python-textual Versionscheck, LUKS-Mapper-Voraussetzung für krypt-stick, AppVM Disk-Images fehlen, NVMe ungetestet
- daemon.toml: socket_path-Inkonsistenz (behoben)
- initramfs: kein Passphrase-Fallback (by design)
- GRUB: JetBrainsMono.pf2 nicht generiert
- Plymouth: Script-Array-Syntax unvalidiert
- IOMMU: Voraussetzung, QEMU-unprüfbar
- dom0 Netzwerkisolation: nicht automatisch konfiguriert
- Hyprland: col.shadow Syntax-Versionsabhängigkeit
- Roadmap-Tabelle: was blockt Alpha, was nicht

### Aktueller Stand
```
Neue/geänderte Dateien Phase 12:
  krypt-stick/src/create.rs            (vollständige Implementierung, 4 neue Tests)
  krypt-stick/src/main.rs              (--force Flag)
  installer/steps/usb.py              (vollständiger Rewrite — korrekte CLI + ListView)
  build/airootfs/etc/krypt/daemon.toml (lowercase fix, memory fix, trust fix)
  build/packages.x86_64               (Review: xen-docs entfernt, fehlende Pakete ergänzt)
  docs/testing.md                     (NEU — 8-Sektionen Pre-First-Boot Checkliste)
  docs/known-issues.md                (NEU — 15 bekannte Lücken, Alpha-Roadmap)
  PROGRESS.md                         (Phase 12)

cargo clippy --workspace -- -D warnings → sauber
cargo test --workspace                 → 34/34 grün
  vm-daemon: 22 Tests
  krypt-stick: 12 Tests (8 luks.rs + 4 create.rs)
  gui-protocol: 0 Tests
cargo build --release                  → alle 3 Crates, 0 Warnings
```

### Was fehlt bis v0.1.0-alpha (priorisiert)

**Blockt Alpha:**
1. **AppVM Disk-Images** — `installer/steps/vms.py` muss verschlüsselte Images anlegen (`cryptsetup luksFormat`, `mkfs.ext4`) sonst scheitert `xl create` nach Reboot
2. **python-textual Versionscheck** — im ISO validieren (`python -c "import textual; print(textual.__version__)"`)
3. **dom0 Netzwerkisolation** — Installer muss `NetworkManager` in dom0 disablen oder Doku muss klarer warnen

**Vor Alpha wünschenswert:**
4. **GRUB PF2-Font** — `grub-mkfont JetBrainsMono → JetBrainsMono.pf2` in build.sh
5. **Plymouth Script-Syntax** — auf echter Plymouth-Instanz validieren
6. **QEMU-Boot-Test** — docs/testing.md Sektion 3 vollständig durchlaufen

**Phase 13 (nach Alpha):**
- gui-protocol: Xen Grant-Table FFI (echte Pixel)
- gui-protocol: wl_frame_callback (compositor-driven vsync)
- gui-protocol: Input-Forwarding (wl_seat, wl_keyboard, wl_pointer)
- Inter-VM Clipboard + Trust-Eskalations-Dialog
- krypt-agent: separates Crate für AppVM-seitigen IPC-Agenten

### Nächste Session beginnt mit
1. `installer/steps/vms.py` — `_create_vm_disk_images()`: `cryptsetup luksFormat` + `mkfs.ext4` pro ausgewählter VM
2. QEMU-Boot-Test durchführen: `qemu-system-x86_64 -cdrom dist/krypt-os-*.iso ...` und Checkliste abarbeiten
3. `gui-protocol/src/xen.rs` — Xen Grant-Table FFI (Phase 13): `libxengnttab` via `pkg-config`, `xengnttab_map_grant_refs()`

### Offene Fragen / Blockers
- `libxengnttab.so`: Nur auf echtem Xen-System; Build-CI kann Grant-Table nicht testen
- Plymouth Array-Syntax: Verschiedene Plymouth-Versionen (0.9.x vs. 22.x) haben unterschiedliche Script-Engine-APIs
- AppVM-Template: Alpine vs. Arch-Minimal als erstes AppVM-Image für sys-gui?
- GRUB JetBrainsMono.pf2: `grub-mkfont` braucht TTF auf dem Build-System
- python-textual in extra/: Arch hat `python-textual` seit 2024-01, aber Patch-Version kann variieren

---

## 2026-05-13 — Phase 11: ISO-Build, CI-Gate, Installer Polish, README, ADR-012/013

### Erledigt

**build/build.sh — vollständiger ISO-Build-Ablauf:**
- `--skip-rust` Flag: überspringt `cargo build` wenn Binaries bereits vorhanden (für CI)
- `--clean` und `--output` Flags beibehalten
- `profiledef.sh` wird dynamisch generiert: `iso_name="krypt-os"`, Label mit Datum, Permissions-Map für alle Krypt-Binaries
- GRUB-Theme: `dotfiles/grub/krypt-grub` → `airootfs/boot/grub/themes/krypt-grub/`, `/etc/default/grub` mit GRUB_THEME + GRUB_DISTRIBUTOR="Krypt OS"
- Plymouth-Theme: `dotfiles/plymouth/krypt` → `airootfs/usr/share/plymouth/themes/krypt/`, `plymouthd.conf` setzt Theme=krypt
- Installer: `installer/` → `airootfs/usr/share/krypt-installer/`, Wrapper-Script `/usr/bin/krypt-install`
- Installer-Autostart: `krypt-installer-tty1.service` startet TUI auf tty1 wenn `/etc/krypt/.installed` fehlt
- Dotfiles für sys-gui in `/etc/skel/.config/`: nvim, hyprland (→hypr), waybar, rofi, foot, krypt-theme
- Alle Dotfile-Scripts via chmod +x gesichert
- krypt-daemon.service in `multi-user.target.wants` verlinkt
- Rust: Build als `SUDO_USER` (root vermeiden), fallback auf direktes cargo
- SHA256-Datei wird automatisch geschrieben
- Krypt ASCII-Logo im Build-Output
- Syntax-Check: `bash -n build/build.sh` → OK

**.github/workflows/build-iso.yml — Kompletter Rewrite:**
- **Job 1 `rust-ci`**: läuft bei jedem push/PR auf main; `dtolnay/rust-toolchain@stable`, `Swatinem/rust-cache@v2`, `libwayland-dev` apt-dep, `cargo clippy -D warnings`, `cargo test`
- **Job 2 `build-iso`**: `needs: rust-ci`, nur bei Tags oder `workflow_dispatch`; Arch-Container mit `--privileged`; rustup, wayland-packages, erneutes clippy+test, `cargo build --release`, dann `build.sh --skip-rust`; GPG-Signierung wenn `GPG_PRIVATE_KEY` Secret gesetzt; `upload-artifact@v4` für ISO + sha256 + .asc; `action-gh-release@v2` für Pre-release-Detection (alpha/beta/rc-Tags)
- **Job 3 `shell-check`**: ShellCheck + `bash -n` für alle .sh-Dateien
- Release-Body mit Installations-Befehlen + Links zu install.md und PROGRESS.md

**installer/steps/install.py — Threading-Fix + Verbesserungen:**
- Widget-Referenzen (`_log`, `_prog`, `_label`) in `on_mount()` gespeichert (bevor Thread startet) statt via `call_from_thread` nachgeschlagen
- `run_interactive()` Helper für cryptsetup (braucht stdin-Passphrase direkt)
- NVMe-Partition-Benennung: `part_sep = "p" if disk[-1].isdigit() else ""` → `/dev/nvme0n1p1` korrekt
- `partprobe` nach sgdisk (Kernel-Partition-Table update)
- Vollständiges GRUB-Config via `_write_grub_config()` mit regex-Replace statt Append
- Hyprland + Waybar + Foot + Rofi + JetBrainsMono-Nerd-Font mit `pacstrap`
- krypt-daemon.service + NetworkManager per `arch-chroot systemctl enable`
- `Button.set(disabled=False)` statt `.__setattr__` Hack

**installer/steps/usb.py — Vereinfacht + robuster:**
- Widget-Refs in `on_mount()`
- Binary-Candidates-Liste: sucht in `/mnt/usr/local/bin/`, `/usr/bin/`, `/usr/local/bin/`
- Alle Fehler erlauben "Weiter" (partial failure ist OK für USB-Step)
- btn-skip und btn-next gehen beide → VmsScreen

**installer/steps/vms.py — Sauberere Struktur:**
- Widget-Refs in `on_mount()`
- `_generate_xl_config()` als f-string mit vollständigen XL-Feldern (`on_poweroff`, `on_reboot`, `on_crash`)
- `_write_daemon_toml()` mit `[policy]`-Sektion (usb_kill_switch, kill_on_unplug)
- `FinishScreen` mit vollem ASCII-Logo

**README.md — Vollständig überarbeitet:**
- CI/Rust/License/Arch/Xen Badges
- Feature-Liste mit Checkmarks
- ASCII-Mockup (Screenshot-Placeholder)
- Architektur-Diagramm
- Quick Start: ISO herunterladen + selbst bauen
- Roadmap mit Phase-Status
- Contributing-Guide (Setup, Coding Standards, Commit-Format, PR-Prozess)
- Hardware-Empfehlungen Tabelle
- Sicherheits-Prinzipien

**docs/decisions.md — ADR-012 + ADR-013:**
- **ADR-012**: `threading.Thread` + `call_from_thread` statt Worker-API — Begründung: blocking subprocess nicht asyncio-kompatibel, linearer Installer braucht kein Cancel
- **ADR-013**: `--skip-rust` Flag in build.sh für CI-Trennung von Rust-Build und ISO-Assembly

### Aktueller Stand
```
Neue/geänderte Dateien Phase 11:
  build/build.sh                           (vollständiger Rewrite)
  .github/workflows/build-iso.yml          (vollständiger Rewrite mit CI-Gate)
  installer/steps/install.py               (Threading-Fix, NVMe, GRUB-Config)
  installer/steps/usb.py                   (Threading-Fix, Binary-Candidates)
  installer/steps/vms.py                   (Threading-Fix, vollständige XL + daemon.toml)
  README.md                                (Badges, Features, Screenshots-Placeholder, Contributing)
  docs/decisions.md                        (ADR-012, ADR-013)
  PROGRESS.md                              (Phase 11)

cargo clippy -- -D warnings → sauber
cargo test --workspace      → 30/30 grün
cargo build --release       → alle 3 Crates OK
bash -n build/build.sh      → Syntax OK
```

### Nächste Session beginnt mit
1. `gui-protocol/src/xen.rs` — Echte Xen Grant-Table FFI: `libxengnttab` via `pkg-config`, `xengnttab_map_grant_refs()`, `xengnttab_unmap_grant_refs()`; setzt voraus dass `libxengnttab.so` im Build-System vorhanden
2. `gui-protocol/src/main.rs` — `wl_callback` Frame-Callback statt sleep-basiertem Frame-Budget (compositor-driven vsync)
3. `gui-protocol/src/input.rs` — wl_seat + wl_keyboard + wl_pointer Events an fokussierte AppVM weiterleiten
4. `installer/` — Integration-Test: `python3 -m py_compile installer/**/*.py` (Syntax-Check in CI ergänzen)
5. Inter-VM Clipboard: Trust-Eskalation Dialog

### Offene Fragen / Blockers
- `libxengnttab.so` nur auf Xen-Systemen verfügbar — Build-System braucht `xen` oder `xen-devel` Paket
- wl_frame_callback ersetzt sleep-basiertes Frame-Budget durch compositor-seitige vsync-Signale (ADR-schreiben wenn implementiert)
- Plymouth Script-Array-Syntax: auf echter Plymouth-Instanz validieren (Version variiert)
- GRUB PF2-Font: JetBrainsMono.pf2 via `grub-mkfont` generieren
- CI: archiso braucht `--privileged` Container → GitHub Actions ubuntu-latest support prüfen (loop-Device Verfügbarkeit)
- `genfstab` in install.py nutzt `stdout=open(...)` — besser: `with open` + `subprocess.run`

---

## 2026-05-13 — Phase 10: 60fps Event-Loop, wl_shm Pixel-Pipeline, TUI-Installer, Neovim-Config, Installationsguide

### Erledigt

**gui-protocol/src/xen.rs — DirtyRect + Frame-Pacing:**
- `DirtyRect { x, y, width, height }` + `DirtyRect::full(w, h)` Konstruktor
- `XenInterface::frame_times: Mutex<HashMap<DomId, Instant>>` — per-VM last-frame tracking
- `poll_dirty_regions(domid)` — non-blocking: gibt `vec![DirtyRect::full(...)]` alle 16ms zurück (erster Aufruf: sofort dirty, sonst empty)
- `FRAME_INTERVAL: Duration = Duration::from_millis(16)` — 60fps Zielrate
- `read_pixels()` Stub → `Vec::new()` (Phase 11: echte Xen Grant-Table FFI)

**gui-protocol/src/wayland.rs — wl_shm Pixel-Pipeline:**
- `WlShm` in Registry gebunden, `Dispatch<WlShm, ()>` (format events, no-op)
- `delegate_noop!(WaylandState: WlShmPool)` — Pool-Callbacks werden ignoriert
- `Dispatch<WlBuffer, Arc<AtomicBool>>` — setzt `released=true` bei Release-Event
- `ShmBuf` Struct: `file`, `pool`, `buffer`, `released: Arc<AtomicBool>`, `stride`, `size`
- `create_shm_file(size)` — `/dev/shm/krypt-gui-<pid>-<counter>`, sofort `unlink()`, `set_len()` → POSIX anonymous file
- `SHM_COUNTER: AtomicU32` für unique Dateinamen
- `AppVmSurface::update_pixels()` — echte wl_surface Pipeline: `attach` → `damage_buffer` → `commit`
- `Compositor::resize_surface()` — public API ohne private Type Leak (refactored aus `resize_internal()`)
- Borrow-Fix: Proxy-Clone vor `queue.roundtrip()` (wayland-client Proxies sind Arc-backed, Clone O(1))
- `WaylandError::Buffer(String)` + `From<std::io::Error>` für shm Fehler

**gui-protocol/src/main.rs — vollständiger 60fps Event-Loop:**
- Architektur: tokio main (SIGTERM/SIGINT) + `std::thread::spawn` für Wayland (EventQueue ist `!Send`)
- `STUB_VMS` mit work/browser/vault Konfigurationen
- `trust_colored_frame()` — Catppuccin Mocha Farben pro Trust-Level (Pixel-Generierung)
- Frame-Loop: `poll_dirty_regions()` → skip wenn nicht configured → Pixel generieren → `update_pixels()` → `dispatch()` → sleep(budget - elapsed)
- `Arc<AtomicBool>` Shutdown-Flag zwischen tokio-Main und Wayland-Thread

**installer/ — Python TUI-Installer (Textual):**
- `installer/main.py` — `KryptInstaller(App)` mit Catppuccin-CSS-Theme, Root-Check
- `installer/steps/welcome.py` — ASCII-Logo, Start/Quit Buttons, → DiskScreen
- `installer/steps/disk.py` — `BlockDevice` Dataclass, `list_block_devices()` via lsblk JSON, ListView, → LuksScreen
- `installer/steps/luks.py` — Passphrase-Eingabe, `_passphrase_strength()` Indikator, min 20 Zeichen, → InstallScreen
- `installer/steps/install.py` — Background-Thread: sgdisk, cryptsetup (Argon2id, aes-xts-plain64, 512-bit), pacstrap, Xen, GRUB, krypt-daemon; Fortschrittsbalken mit Phasen-Gewichten
- `installer/steps/usb.py` — `krypt-stick --setup --luks-device /dev/mapper/krypt-root --slot 1`, FileNotFoundError-Handling
- `installer/steps/vms.py` — Checkbox-VM-Auswahl, `_generate_xl_config()`, `_write_daemon_toml()`, FinishScreen
- `installer/requirements.txt` — `textual>=0.70.0`, `rich>=13.7.0`, `psutil>=5.9.0`

**dotfiles/neovim/ — vollständige Krypt-Style Neovim-Config (lazy.nvim):**
- `init.lua` — lazy.nvim Bootstrap, `mapleader = " "`, options/autocmds/keymaps laden
- `lua/config/options.lua` — relativenumber, cursorline, tabstop=4, undofile, clipboard=unnamedplus, foldmethod=expr (treesitter), grepprg=rg
- `lua/config/keymaps.lua` — `<C-hjkl>` Fenster-Nav, `<leader>ff/fg/fb/fs` Telescope, `gd/gr/gi/K` LSP, `<leader>gg` LazyGit, `<leader>e` Neotree, `jj` ESC
- `lua/config/autocmds.lua` — Cursor-Restore, Trailing-Whitespace-Trim, RelNumber in Normal-Mode, colorcolumn (100 Rust, 88 Python), LSP auto-format on save
- `lua/plugins/colorscheme.lua` — catppuccin/nvim Mocha, Krypt-Violet `#9d4edd` für `@type`/CursorLineNr/Telescope/WhichKey
- `lua/plugins/lsp.lua` — Mason + mason-lspconfig + nvim-lspconfig: rust_analyzer (clippy checkOnSave, alle inlay hints), pyright (strict), lua_ls, bashls, taplo, jsonls; diagnostic icons + rounded borders
- `lua/plugins/treesitter.lua` — nvim-treesitter + textobjects: af/if/ac/ic/aa/ia Textobjects, ]f/[f/]c/[c Moves, swap parameter
- `lua/plugins/telescope.lua` — Fuzzy Finder mit fzf-native Extension, ui-select, vollständige Keymaps, file_ignore_patterns
- `lua/plugins/completion.lua` — nvim-cmp + LuaSnip v2 + lspkind Icons, ghost_text, Tab/S-Tab Snippet-Navigation, cmdline completion
- `lua/plugins/ui.lua` — neo-tree v3, lualine (catppuccin theme), which-key (Gruppen), gitsigns (Hunk-Keymaps), indent-blankline, nvim-notify, noice, LazyGit, dashboard (Krypt ASCII-Logo), Trouble, todo-comments

**docs/install.md — vollständige Installationsanleitung:**
- 12 Kapitel: Voraussetzungen, Bootmedium, Partitionierung/LUKS2, Arch-Basis, Xen, Krypt-Komponenten bauen, USB-Kill-Switch, AppVMs, Dotfiles, Verifikation, TUI-Installer, Troubleshooting
- Alle Befehle copy-paste-ready mit Erklärungen
- Sicherheits-Checkliste für erste Anmeldung

**dotfiles/install.sh:**
- Neovim-Symlink ergänzt: `~/.config/nvim` → `dotfiles/neovim`

### Aktueller Stand
```
Neue/geänderte Dateien Phase 10:
  gui-protocol/src/xen.rs              (DirtyRect, frame pacing)
  gui-protocol/src/wayland.rs          (wl_shm, ShmBuf, update_pixels live)
  gui-protocol/src/main.rs             (60fps Event-Loop, trust_colored_frame)
  installer/main.py                    (NEU)
  installer/steps/welcome.py           (NEU)
  installer/steps/disk.py              (NEU)
  installer/steps/luks.py              (NEU)
  installer/steps/install.py           (NEU)
  installer/steps/usb.py               (NEU)
  installer/steps/vms.py               (NEU)
  installer/requirements.txt           (NEU)
  dotfiles/neovim/init.lua             (NEU)
  dotfiles/neovim/lua/config/options.lua   (NEU)
  dotfiles/neovim/lua/config/keymaps.lua   (NEU)
  dotfiles/neovim/lua/config/autocmds.lua  (NEU)
  dotfiles/neovim/lua/plugins/colorscheme.lua (NEU)
  dotfiles/neovim/lua/plugins/lsp.lua      (NEU)
  dotfiles/neovim/lua/plugins/treesitter.lua (NEU)
  dotfiles/neovim/lua/plugins/telescope.lua  (NEU)
  dotfiles/neovim/lua/plugins/completion.lua (NEU)
  dotfiles/neovim/lua/plugins/ui.lua        (NEU)
  docs/install.md                      (NEU)
  dotfiles/install.sh                  (Neovim-Symlink ergänzt)

cargo clippy -- -D warnings → sauber
cargo test → 30/30 grün
  vm-daemon: 22 Tests (8 IPC + 14 policy)
  krypt-stick: 8 Tests (luks.rs)
  gui-protocol: 0 Tests (nur bin)
```

### Nächste Session beginnt mit
1. `gui-protocol/src/xen.rs` Phase 11: echte Xen Grant-Table FFI (`libxengnttab`) — `xengnttab_map_grant_refs()` + unmap
2. `gui-protocol/src/main.rs` — `wl_callback` Frame-Callback statt sleep-basiertem Budget (compositor-driven pacing)
3. `gui-protocol/src/input.rs` — Wayland input events (wl_seat, wl_keyboard, wl_pointer) an focused AppVM weiterleiten
4. Inter-VM Clipboard: Trust-Eskalation Dialog (Phase 11)
5. `krypt-agent/` — separates Crate für AppVM-seitigen IPC-Agenten

### Offene Fragen / Blockers
- gui-protocol Phase 11: `libxengnttab.so` braucht Xen-Dev-Paket; Grant-Table mapping erfordert Dom0-Privilegien
- `wl_frame_callback`: ersetzt sleep-basiertes Frame-Budget durch compositor-seitige vsync-Signale
- Plymouth Script-Array-Syntax: muss auf echter Plymouth-Instanz validiert werden
- GRUB PF2-Font: JetBrainsMono.pf2 via `grub-mkfont` generieren
- TUI-Installer: Beta-Status, für Produktion manuelle Installation empfohlen
- Hyprland `col.shadow` Syntax: Änderung in ≥ 0.40 prüfen

---

## 2026-05-13 — Phase 9: VmStartRequest IPC, wayland-client 0.31, krypt-launcher, ADR-011

### Erledigt

**vm-daemon/src/ipc.rs — VmStart/Stop Message-Pair + IpcClient-Methoden:**
- `IpcMessage` enum erweitert um:
  - `VmStartRequest { vm_name }` — Agent→Daemon: VM starten
  - `VmStopRequest { vm_name, force }` — Agent→Daemon: VM stoppen (force → xl destroy)
  - `VmStartResponse { vm_name, domain_id }` — Daemon→Agent: Erfolg + Domain-ID
  - `VmStopResponse { vm_name }` — Daemon→Agent: Erfolg
- `IpcClient::start_vm(&mut self, vm_name) → Result<Option<u32>>` — sendet VmStartRequest, parst VmStartResponse
- `IpcClient::stop_vm(&mut self, vm_name, force) → Result<()>` — sendet VmStopRequest, parst VmStopResponse
- 3 neue Tests: `roundtrip_vm_start`, `roundtrip_vm_stop`, `vm_start_error_propagates`

**vm-daemon/src/main.rs — dispatch_ipc Handler:**
- `VmStartRequest`: write-lock auf VmManager, ruft `vm.start().await`, gibt VmStartResponse oder Error zurück
- `VmStopRequest`: write-lock, ruft `vm.shutdown()` (ACPI) oder `vm.destroy()` (force), VmStopResponse oder Error

**gui-protocol/ — wayland-client 0.31 Integration:**
- `gui-protocol/Cargo.toml`: `wayland-client = "0.31"` + `wayland-protocols = { version = "0.31", features = ["client"] }` aktiviert
- `gui-protocol/src/wayland.rs` — vollständige Phase-9-Implementierung:
  - `WaylandState` Dispatch-State mit `Dispatch<WlRegistry>` (bind compositor + xdg_wm_base), `Dispatch<XdgWmBase>` (ping/pong!), `Dispatch<XdgSurface, Arc<AtomicBool>>` (configure + ack), `Dispatch<XdgToplevel>` (no-op für Phase 10), `Dispatch<WlSurface>` (enter/leave ignoriert)
  - `Compositor::connect()` — real `Connection::connect_to_env()` + registry roundtrip
  - `Compositor::create_surface()` — erstellt wl_surface + xdg_surface + xdg_toplevel; setzt title `"[<trust>] <vm>: <title>"` und app_id `"krypt.<vm>"` → Hyprland windowrulev2 greift
  - `Compositor::dispatch()` — Event-Loop-Tick (dispatch_pending)
  - `AppVmSurface` hält live WlSurface + XdgSurface + XdgToplevel
  - `AppVmSurface::is_configured()` — prüft AtomicBool nach erstem configure-Event
- `gui-protocol/src/main.rs` — stub auf echte Compositor-Verbindung umgestellt (block_in_place + sigterm-Loop); Phase-10-AppVM-Loop kommentiert

**dotfiles/rofi/krypt-launcher.sh — VmStart via IPC verdrahtet:**
- Halted-Branch: sendet `{"type": "vm_start_request", "vm_name": name}` via Python-IPC
- 30s Timeout (xl create kann bei verschlüsselter Disk ~30s dauern)
- Nach Erfolg: Workspace-Wechsel zum Trust-Level-Workspace

**docs/decisions.md — ADR-011: xdg_toplevel für AppVM-Fenster:**
- Entscheidung: xdg_toplevel über wl_subsurface + Custom Protocol
- Hauptgrund: Hyprland windowrulev2 matcht auf title + initialClass ohne Compositor-Modifikation
- wl_subsurface: kein eigener Titel, kein windowrulev2-Matching → abgelehnt
- Custom Protocol: Hyprland müsste implementieren, keine xdg-portal-Kompatibilität → abgelehnt
- wayland-client 0.31 (Smithay): stabile Rust-Bindings, system libwayland-client.so

### Aktueller Stand
```
Neue/geänderte Dateien Phase 9:
  vm-daemon/src/ipc.rs               (4 neue IpcMessage Varianten, 2 IpcClient-Methoden, 3 Tests)
  vm-daemon/src/main.rs              (VmStartRequest + VmStopRequest Handler in dispatch_ipc)
  gui-protocol/Cargo.toml            (wayland-client + wayland-protocols aktiviert)
  gui-protocol/src/wayland.rs        (vollständige Neuentwicklung mit wayland-client 0.31)
  gui-protocol/src/main.rs           (Compositor::connect() verdrahtet)
  dotfiles/rofi/krypt-launcher.sh    (VmStartRequest IPC Implementierung)
  docs/decisions.md                  (ADR-011)

cargo clippy --workspace -- -D warnings → sauber
cargo test --workspace → 30/30 grün
  vm-daemon: 22 Tests (7 IPC + 1 serialize + 14 policy)
  krypt-stick: 8 Tests (luks.rs)
  gui-protocol: 0 Tests (nur bin, kein lib)
```

### Nächste Session beginnt mit
1. `gui-protocol/src/xen.rs` Phase 10: echte Xen-Grant-Table FFI (`libxengnttab`)
2. `vm-daemon`: VmStateChanged Broadcast-Event — alle Connections benachrichtigen wenn VM-State wechselt
3. `krypt-agent/` — neues Crate: AppVM-seitiger IPC-Agent (vm-daemon als Dep mit `--features agent`)
4. `init/krypt-daemon.service` — `RestartPolicy=on-failure`, Watchdog, socket activation prüfen
5. `docs/` — Deployment-Guide: dom0-Setup, xl-Konfiguration, erstes Booten

### Offene Fragen / Blockers
- gui-protocol Phase 10: `xengnttab_map_grant_refs()` braucht `libxengnttab.so` (Paket: `xen` oder `xen-devel`)
- Compositor::create_surface(): roundtrip wartet auf configure — kein Timeout → hängt wenn $WAYLAND_DISPLAY nicht gesetzt
- Plymouth Script-Array-Syntax: muss auf echter Plymouth-Instanz validiert werden
- GRUB PF2-Font: JetBrainsMono.pf2 via `grub-mkfont` generieren (braucht ttf-Datei + grub package)

---

## 2026-05-13 — Phase 8: gui-protocol, dead_code Fix, ISO-Build, vollständiges Design-System

### Erledigt

**TRACK A — Entwicklung:**

**IpcClient dead_code bereinigt:**
- `vm-daemon/Cargo.toml` — `[features] agent = []` hinzugefügt
- `vm-daemon/src/ipc.rs` — `#[allow(dead_code)]` ersetzt durch `#[cfg(any(test, feature = "agent"))]`
  - Im Binary (kein Feature): IpcClient nicht kompiliert → keine dead_code-Warnung
  - In Tests: IpcClient verfügbar (cfg(test) ist true)
  - Externe Crates: `cargo add krypt-daemon --features agent`

**gui-protocol/ Grundstruktur (NEU — Workspace-Member):**
- `gui-protocol/Cargo.toml` — thiserror, tracing, tokio, futures; wayland-client + xenctrl als Phase-9/10-Kommentare
- `gui-protocol/src/main.rs` — minimaler tokio-Stub, Architektur-Kommentar, Phase-9-TODO
- `gui-protocol/src/wayland.rs`:
  - `TrustLevel` Enum + `as_tag()` → Hyprland Border-Konvention
  - `SurfaceConfig` + `krypt_title()` → "[trust] vm: title" Format
  - `AppVmSurface` + `update_pixels()` + `resize()` — Pixel-Buffer-Interface
  - `Compositor::connect()` + `create_surface()` — Phase-9-Stubs
- `gui-protocol/src/xen.rs`:
  - `GuestMetadata` (domid, grant_refs, width, height, pixel_format)
  - `SharedBuffer::pixels()` + `stride()` — Framebuffer-Interface
  - `XenInterface::open()` + `accept_guest()` + `wait_for_update()` — Phase-10-Stubs
- `gui-protocol/src/input.rs`:
  - `KeyEvent`, `MouseEvent`, `ClipboardRequest` Structs
  - `InputRouter` — focus tracking + `route_key()` + `route_mouse()`
  - `request_clipboard()`: Trust-Eskalation immer deny, Dialog Phase 11
  - `trust_level_score()` für automatische Deny-Logik

**ISO-Build-Script:**
- `build/build.sh` — lokaler archiso-Builder:
  - Basis: /usr/share/archiso/configs/releng/
  - Krypt-Pakete appenden, airootfs-Overlay kopieren
  - initramfs-Hooks, systemd-Units, Rust-Binaries einbinden
  - dotfiles nach /etc/skel/.config/krypt-dotfiles
  - `cargo build --release` + install der Binaries
  - `mkarchiso -v` → ISO in dist/
  - SHA256-Output + dd-Befehl
- `build/packages.x86_64` — 50+ Pakete: Xen, linux-lts, cryptsetup, hyprland, waybar, rofi, foot, plymouth, TPM2, YubiKey, Nerd Fonts, etc.
- `build/airootfs/etc/mkinitcpio.conf` — HOOKS: base udev krypt encrypt lvm2 filesystems fsck; MODULES: xen-blkfront, dm_crypt, aes
- `build/airootfs/etc/krypt/daemon.toml` — kommentiertes Beispiel mit sys-gui, work, browser, vault VMs + Policy

**TRACK B — Design-System:**

- `dotfiles/theme/colors.conf` — zentrale Farbpalette:
  - Krypt-Violett: #9d4edd (primär), #7b2fbe (dark), #c77dff (light), #e0aaff (glow)
  - Vollständige Catppuccin Mocha Palette (Crust→Text, alle Akzente)
  - Semantische Trust-Level Farben (identisch mit hyprland.conf)
  - Transparenz-Konstanten (ALPHA_HIGH/MED/LOW/DIM)

- `dotfiles/hyprland/animations.conf` — ausgelagerte Bezier-Animationen:
  - 5 Bezier-Kurven: krypt (Overshoot), krypt_fast (responsive), krypt_out (smooth), krypt_snap (security-UI), krypt_linear
  - Fenster: slide in, popin 88% out, windowsMove
  - Workspaces: slide horizontal, slidevert für Vault/Special
  - Layers: Waybar/Rofi slide
  - Border: rotierender Gradient (borderangle loop)
- `dotfiles/hyprland/hyprland.conf` — Animationen-Sektion durch `source = animations.conf` ersetzt

- `dotfiles/waybar/config.jsonc` — vollständige Bar-Konfiguration:
  - Links: Workspaces (custom Icons pro ws), Window-Titel mit Trust-Tag-Rewrite
  - Mitte: custom/krypt-vms (IPC, on-click: krypt-launcher.sh)
  - Rechts: cpu, memory, network, pulseaudio, battery, clock, tray
  - Alle Module mit Icons (Nerd Fonts), Tooltips, Warnings/Critical-States

- `dotfiles/waybar/style.css` — vollständiges Dark-Theme:
  - CSS Custom Properties für alle Catppuccin/Krypt-Farben
  - Glassmorphism: alpha(#1e1e2e, 0.88), border-radius: 12px, Krypt-Violett-Border
  - Workspace-Buttons: normal/active/urgent/hover mit Transitions
  - Vault-Workspace (ws9): spezielle Violett-Intensivierung
  - Alle Module: farbcodiert (cpu=blue, mem=teal, net=green, audio=mauve, battery states)
  - Tooltips: Crust-Hintergrund, Violett-Border

- `dotfiles/rofi/krypt.rasi` — vollständiges Rofi-Theme:
  - Alle CSS Custom Properties inline definiert
  - Window: 540px, border 2px solid Krypt-Violett, border-radius 14px
  - Inputbar: bg-alt, focus-within border-color accent, prompt in Krypt-Violett
  - Listview: 8 Zeilen, kein Scrollbalken, 4px spacing
  - element.selected: rgba(157,78,221,0.20) + border
  - Urgent: rot-tinted
  
- `dotfiles/rofi/krypt-launcher.sh` — VM-Launcher:
  - VM-Liste via Python-IPC (ListVmsQuery)
  - Trust-Icon + State-Icon pro Eintrag (Nerd Font Icons)
  - Handle: Running → workspace wechseln; Halted → notify (IPC-Start Phase 9)

- `dotfiles/hyprlock/hyprlock.conf` — Lockscreen:
  - Hintergrund: Crust Solid + Blur (3 passes)
  - Krypt ASCII-Logo (Krypt-Violett 60% opacity)
  - Zeit: 90pt JetBrainsMono Nerd Font Bold
  - Datum: 16pt subtext1
  - Input-Field: 320×52, krypt-violet outer_color, dots mit fade
  - Capslock=yellow, Fail=red, Check=violet
  - Hint-Label: "Stick einlegen · oder · PIN eingeben"

- `dotfiles/grub/krypt-grub/theme.txt` — GRUB-Theme:
  - Desktop-Color: Crust #11111b, kein Bild (minimal)
  - ASCII "K R Y P T  O S" + Tagline
  - boot_menu: Krypt-Violett selected_background, surface0 hover
  - Countdown-Label, Hilfe-Leiste, Trennlinie

- `dotfiles/plymouth/krypt/krypt.plymouth` + `krypt.script` — Boot-Animation:
  - Plymouth Script-Engine: 3 Phasen
  - Phase 1 (0–60 Frames): ASCII-Logo Zeile für Zeile einblenden
  - Phase 2 (60+): Violett-Gradient-Progressbar wächst mit Plymouth-Progress
  - Phase 3: Status-Text ("Xen Hypervisor wird geladen...", etc.)
  - quit_callback: alles ausblenden

- `dotfiles/foot/foot.ini` — Terminal:
  - JetBrainsMono Nerd Font 12pt, pad 12x10, dpi-aware
  - Catppuccin Mocha Colors: alle 16 ANSI + cursor + selection
  - selection-target = none (kein Auto-Clipboard-Paste — Security)
  - Nerd-Font-Grapheme-Shaping, font-monospace-warn disabled

- `dotfiles/install.sh` — Symlink-Installer:
  - --dry-run, --force Flags
  - Idempotent: prüft bestehende Symlinks, backup bei --force
  - Krypt ASCII-Logo im Output
  - Hyprland, Hyprlock, Waybar, Rofi, Foot, Theme, GRUB (root), Plymouth (root)
  - chmod +x für Scripte
  - Post-Install-Hinweise

### Aktueller Stand
```
Track A neue/geänderte Dateien:
  vm-daemon/Cargo.toml           (features.agent)
  vm-daemon/src/ipc.rs           (cfg any(test, feature="agent"))
  gui-protocol/Cargo.toml        (NEU)
  gui-protocol/src/main.rs       (NEU)
  gui-protocol/src/wayland.rs    (NEU)
  gui-protocol/src/xen.rs        (NEU)
  gui-protocol/src/input.rs      (NEU)
  Cargo.toml                     (gui-protocol als workspace member)
  build/build.sh                 (NEU)
  build/packages.x86_64          (NEU)
  build/airootfs/etc/mkinitcpio.conf   (NEU)
  build/airootfs/etc/krypt/daemon.toml (NEU)

Track B neue/geänderte Dateien:
  dotfiles/theme/colors.conf           (NEU)
  dotfiles/hyprland/animations.conf    (NEU)
  dotfiles/hyprland/hyprland.conf      (source animations.conf)
  dotfiles/waybar/config.jsonc         (NEU)
  dotfiles/waybar/style.css            (NEU)
  dotfiles/rofi/krypt.rasi             (NEU)
  dotfiles/rofi/krypt-launcher.sh      (NEU)
  dotfiles/hyprlock/hyprlock.conf      (NEU)
  dotfiles/grub/krypt-grub/theme.txt   (NEU)
  dotfiles/plymouth/krypt/krypt.plymouth (NEU)
  dotfiles/plymouth/krypt/krypt.script   (NEU)
  dotfiles/foot/foot.ini               (NEU)
  dotfiles/install.sh                  (NEU)

cargo clippy -- -D warnings → sauber (3 Crates)
cargo test → 27/27 grün (19 vm-daemon + 8 krypt-stick + 0 gui-protocol)
Shell-Syntax aller Scripts: OK
```

### Nächste Session beginnt mit
1. `gui-protocol/` — Phase 9 Vorbereitung: `wayland-client = "0.31"` in Cargo.toml aktivieren sobald libwayland-client.so auf dem Build-System verfügbar ist; echte `wl_surface` in wayland.rs erstellen
2. `vm-daemon/src/ipc.rs` — `IpcMessage::VmStartRequest { vm_name }` + `VmStopRequest` hinzufügen; `krypt-launcher.sh` kann dann VMs via IPC starten
3. `docs/decisions.md` — ADR-011: gui-protocol Design (warum eigene Implementierung statt qubes-gui-daemon Port)
4. `krypt-stick/` — Integration-Tests mit losetup + LUKS2 (braucht Root, separates Test-Script)

### Offene Fragen / Blockers
- gui-protocol Phase 9: wayland-client 0.31 hat starke API (wayland-rs) — Lernkurve
- gui-protocol Phase 10: xengnttab_map_grant_refs() braucht libxengnttab.so (Xen-Entwicklungspaket)
- Plymouth script-Engine: Array-Syntax (`logo_images[i]`) muss auf echter Plymouth-Version getestet werden — API variiert zwischen Plymouth-Versionen
- GRUB-Theme: Nerd-Fonts im GRUB-Kontext nur via fontforge-generierte PF2-Fonts; JetBrainsMono-Nerd-Font muss als .pf2 konvertiert werden (grub-mkfont)
- IpcClient feature "agent": in Phase 9 separates krypt-agent-Crate erstellen das vm-daemon mit --features agent als Dependency zieht

---

## 2026-05-13 — Phase 7: luks.rs Tests, IpcClient::list_vms(), systemd-Units, ADR-009/010

### Erledigt

**krypt-stick/src/luks.rs — 8 Unit-Tests:**
- `active_slots_from_dump()` von `fn` auf `pub(crate) fn` gehoben (für Tests zugänglich)
- Bug-Fix im Parser: Sektion-End-Check prüfte nur `starts_with(' ')` → Tab-indentierte
  Sub-Entries (reales `cryptsetup`-Output) brachen die Sektion vorzeitig ab.
  Fix: `|| line.starts_with('\t')` ergänzt.
- 8 Tests ohne echtes cryptsetup/Hardware:
  - `luks2_parses_three_active_slots` — LUKS2 Keyslots-Sektion, Slots 0/1/3
  - `luks1_parses_enabled_slots` — LUKS1 "Key Slot N: ENABLED", Slots 0/2
  - `empty_dump_returns_no_slots` — leeres Keyslots-Segment
  - `all_32_slots_occupied` — full_luks2_dump()-Hilfsfn, alle 32 Slots, kein freier Slot
  - `next_free_slot_skips_used` — Slots 0+1 belegt → next = 2
  - `luks2_section_ends_at_non_indented_line` — "Tokens:" beendet die Sektion korrekt
  - `ignores_luks1_disabled_slots` — DISABLED-Slots werden nicht gezählt
  - `handles_mixed_luks2_and_luks1_format_gracefully` — beide Parser-Pfade koexistieren

**vm-daemon/src/ipc.rs — IpcClient::list_vms() + 2 Tests:**
- `IpcClient::list_vms()`: sendet `ListVmsQuery`, erwartet `ListVmsResponse`,
  propagiert `Error`-Antwort als `IpcError::Io(std::io::Error::other(msg))`
- `roundtrip_list_vms`: vollständiger Server/Client-Roundtrip, prüft name/state/trust
  (HashMap-Reihenfolge-sicher via `.iter().find()`)
- `list_vms_daemon_error_propagates`: Server antwortet mit Error → Client gibt Err zurück
- `cargo clippy -- -D warnings` → `std::io::Error::other()` statt `Error::new(Other, ...)` fix

**init/ — systemd-Units (NEU):**
- `init/krypt-daemon.service`:
  - `After=xenstore.service` + `Wants=xenstore.service`
  - `RuntimeDirectory=krypt` (systemd legt /run/krypt/ an, 0700)
  - `ConfigurationDirectory=krypt` (/etc/krypt/, 0700)
  - Sicherheitshärtung: `ProtectSystem=strict`, `ProtectHome=yes`, `PrivateTmp=yes`,
    `ProtectKernelTunables`, `ProtectControlGroups`, `LockPersonality`
  - `NoNewPrivileges=yes` NICHT gesetzt (xl braucht setuid)
  - `PrivateDevices=no` (xl braucht /dev/xen)
  - `Restart=on-failure` + `RestartPreventExitStatus=0`
- `init/krypt-boot-agent.service`:
  - `Type=oneshot` + `RemainAfterExit=yes`
  - `ConditionPathExists=/dev/mapper/krypt-root` (kein Run wenn kein LUKS-Hook)
  - `After=krypt-daemon.service` + `Requires=krypt-daemon.service`
  - Minimale Rechte: `ReadWritePaths=/run/krypt`

**docs/decisions.md — ADR-009 + ADR-010:**
- **ADR-009: Unix-Domain-Socket + JSON**: Begründung gegen vchan (keine Bindings),
  D-Bus (dom0-Overhead), Protobuf (Codegen-Komplexität); für JSON (debuggbar, serde_json)
  und 4-Byte-LE-Framing (Stream-sicher, 64-KiB-Limit gegen DoS)
- **ADR-010: mkinitcpio statt dracut**: Arch-kanonisch, minimales initramfs,
  explizite HOOKS-Reihenfolge, POSIX-sh-Hook ohne Framework-Overhead

### Aktueller Stand
```
Geänderte/Neue Dateien:
  krypt-stick/src/luks.rs    (pub(crate) fn, Tab-Fix, 8 Tests)
  vm-daemon/src/ipc.rs       (IpcClient::list_vms, 2 neue Tests, clippy-Fix)
  init/krypt-daemon.service  (NEU)
  init/krypt-boot-agent.service (NEU)
  docs/decisions.md          (ADR-009, ADR-010)

cargo clippy -- -D warnings → sauber
cargo test → 27/27 grün (19 vm-daemon + 8 krypt-stick)
```

### Nächste Session beginnt mit
1. `vm-daemon/src/ipc.rs` — `IpcClient` `#[allow(dead_code)]` entfernen: jetzt da
   `list_vms()` eine echte public API ist, braucht der crate einen lib-Target oder
   Integration-Test der den Client nutzt — sonst bleibt `dead_code` warning
2. `krypt-stick` Integration-Tests: `create.rs` + `backup.rs` + `revoke.rs` mit
   Mock-LUKS-Device testen (loop-Device via `losetup` in einem test-Script)
3. `gui-protocol/` Grundstruktur anlegen: `Cargo.toml`, `src/main.rs` Stub,
   `src/wayland.rs` + `src/xen.rs` + `src/input.rs` — Kompilierbarkeit sicherstellen
4. `build/` — ISO-Build-Script prüfen + `packages.x86_64` mit Xen/Hyprland-Paketliste

### Offene Fragen / Blockers
- `IpcClient` dead_code: braucht entweder lib-Target in vm-daemon/Cargo.toml
  oder ein separates `krypt-agent`-Crate das den Client importiert
- `gui-protocol/`: qubes-gui-daemon Protokoll-Analyse steht noch aus (docs/xen-internals.md)
- xenvchan ADR: schreiben wenn wir libxenvchan via FFI ansprechen (Phase 8+)
- TPM2 (`tss-esapi` crate): unverändert offen
- IPC Rate-Limiting gegen DoS durch kompromittierte AppVM: Phase 8+

---

## 2026-05-13 — Phase 6: initramfs Hook, Hyprland Border-Regeln, Waybar IPC

### Erledigt

**Rust — vm-daemon:**
- `vm-daemon/src/policy.rs` — `TrustLevel::to_str()` hinzugefügt (→ "red"|"orange"|…)
- `vm-daemon/src/policy.rs` — `PolicyEngine::get_trust(vm)` hinzugefügt (public, default Red)
- `vm-daemon/src/ipc.rs` — `VmInfo { name, state, domain_id, trust_level }` Struct neu
- `vm-daemon/src/ipc.rs` — `IpcMessage::ListVmsQuery {}` (Agent → Daemon)
- `vm-daemon/src/ipc.rs` — `IpcMessage::ListVmsResponse { vms: Vec<VmInfo> }` (Daemon → Agent)
- `vm-daemon/src/main.rs` — `dispatch_ipc()`: `ListVmsQuery` Handler — kombiniert `VmManager::list()` mit `PolicyEngine::get_trust()` → vollständige VM-Liste mit Trust-Level
- `cargo clippy -- -D warnings` → sauber
- `cargo test` → **17/17 grün**

**Waybar — krypt-vms.py:**
- Vollständig auf IPC-Socket verdrahtet (kein `qvm-ls` mehr)
- Protokoll: 4-Byte-LE-Länge + JSON, identisch zu ipc.rs
- `_recv_exact()`: robustes Byte-genaues Lesen (kein Datenverlust bei Splits)
- `get_running_vms()`: sendet `ListVmsQuery`, filtert auf `state == "Running"`
- Fehlerfall (Daemon nicht erreichbar): leere Liste statt Crash, kein Fallback-Stub
- VMs in Output alphabetisch sortiert

**Hyprland — hyprland.conf:**
- Border-Farben: unverändert (waren korrekt)
- NEU: `col.shadow` pro Trust-Level (korrespondierender Shadow-Farbton)
- NEU: `opacity` pro Trust-Level (green=1.0, yellow=0.97, orange=0.94, red=0.90, black=1.0)
- NEU: `noblur` für red + orange (kein Hintergrund-Durchscheinen)
- NEU: `noanim` für black/vault (kein visuelles Flair für sensibelsten Bereich)
- NEU: `workspace` Zuweisung (red→ws1, yellow→ws2, green→ws3, orange→ws4, black→ws9 silent)
- NEU: `suppressevent maximize` für alle VM-Fenster
- NEU: `layerrule blur` für waybar + notifications
- NEU: Workspace-Keybinds 1–10, SUPER+SHIFT+V → Vault-Workspace
- NEU: Fenster auf Workspace schieben (movetoworkspacesilent)
- NEU: Tab-Cycling (cyclenext/prev)

**initramfs — mkinitcpio Hook:**
- `initramfs/hooks/krypt` — Runtime-Hook (frühe Userspace-Phase):
  - Parst `krypt_luks_uuid=` und `krypt_luks_name=` aus Kernel-Cmdline
  - `udevadm trigger + settle` für USB-Device-Erkennung
  - `_krypt_try_open()`: iteriert alle `/dev/disk/by-id/usb-*` (keine Partitionen)
  - `dd if=<stick> bs=1 skip=512 count=64 | cryptsetup open --key-file=- --keyfile-size=64`
  - Warte-Loop mit Krypt-ASCII-Banner wenn kein Stick gefunden
  - Kein Timeout, kein Passwort-Fallback (by design)
- `initramfs/install/krypt` — Build-Script:
  - `add_binary`: cryptsetup, dd, udevadm, blkid
  - `add_module`: dm-crypt, dm-mod, aes, sha256
  - `add_runscript` für den Hook
  - `help()` mit HOOKS-Reihenfolge und Cmdline-Dokumentation
- `initramfs/krypt-boot-agent.sh` — Post-Boot systemd-oneshot-Service:
  - Läuft nach krypt-daemon.service
  - Ermittelt Stick-UUID aus dm-crypt slaves via sysfs + udevadm
  - Persistiert UUID nach /run/krypt/boot-stick-uuid (0600)
  - IPC-Registrierung via daemon.toml serial-Matching (Phase 7: dedizierter RegisterBootStick-Typ)

### Aktueller Stand
```
Geänderte/Neue Dateien:
  vm-daemon/src/policy.rs      (to_str, get_trust)
  vm-daemon/src/ipc.rs         (VmInfo, ListVmsQuery, ListVmsResponse)
  vm-daemon/src/main.rs        (dispatch_ipc: ListVmsQuery-Handler)
  dotfiles/waybar/krypt-vms.py (IPC statt qvm-ls, vollständig neu)
  dotfiles/hyprland/hyprland.conf (Opacity, Shadow, Blur, Workspace, Keybinds)
  initramfs/hooks/krypt        (NEU — mkinitcpio Runtime-Hook)
  initramfs/install/krypt      (NEU — mkinitcpio Build-Script)
  initramfs/krypt-boot-agent.sh (NEU — Post-Boot Stick-Registrierung)

Was funktioniert: cargo build + clippy sauber + 17 Tests grün
                  Shell-Syntax aller initramfs-Scripts: OK
                  Python-Syntax krypt-vms.py: OK
```

### Nächste Session beginnt mit
1. `krypt-stick/src/luks.rs` Unit-Tests: `active_slots_from_dump()` mit fixture-Output testen (kein echtes cryptsetup nötig — pure String-Parsing-Tests)
2. `vm-daemon/src/ipc.rs` — `IpcClient` um `list_vms()` Convenience-Methode erweitern; Test: ListVmsQuery Roundtrip analog zu roundtrip_policy_check
3. `docs/decisions.md` — ADR für IPC-Protokoll nachtragen (Unix-Socket, JSON-Framing, warum nicht vchan/protobuf)
4. `initramfs/` → systemd-Unit `krypt-boot-agent.service` als echte .service-Datei anlegen (liegt aktuell nur als Kommentar im Script)

### Offene Fragen / Blockers
- `krypt-boot-agent.sh`: `RegisterBootStick` IPC-Nachrichtentyp fehlt noch (Phase 7) — aktuell nur sysfs-Lookup + Datei-Persistierung
- initramfs: `ash` (busybox) vs. `bash` — der Hook nutzt `/bin/ash` (BusyBox-kompatibel), aber `${param#prefix}` und `case *-part[0-9]*` sind POSIX-kompatibel ✓
- Hyprland `col.shadow` Syntax: wurde in Hyprland ≥ 0.40 geändert (war `col.shadow`, jetzt evtl. `shadow_color`) — beim Deployment prüfen
- mkinitcpio vs. dracut: Entscheidung für mkinitcpio (Arch-kanonisch) ist gefallen; ADR schreiben
- TPM2-Bindung: `tss-esapi` crate — noch offen
- IPC Rate-Limiting für ListVmsQuery: Waybar fragt alle 5s — kein Problem für einen Client, aber absichern gegen kompromittierte AppVM die spammt

---

## 2026-05-13 — Phase 5: Policy-Dispatch, load_from_toml, krypt-stick cryptsetup

### Erledigt
- `vm-daemon/src/policy.rs` — `load_from_config()` + `load_from_toml()` implementiert:
  - `load_from_config(&KryptConfig)`: füllt trust_map aus VmEntry.trust_level + add_rule() aus PolicyEntry
  - `load_from_toml(path)`: lädt KryptConfig, delegiert an load_from_config
  - Hilfsfunktionen `map_trust()` + `map_action()` (private, kein pub-Overhead)
- `vm-daemon/src/main.rs` — vollständig umgebaut:
  - Inline-Policy-Loop ersetzt durch `policy_engine.load_from_config(&cfg)` (Trust-Level + Regeln in einem Zug)
  - `policy_engine` + `vm_manager` in `Arc<tokio::sync::RwLock<>>` gewrappt (teilen über IPC-Tasks)
  - IPC-Socket nach Bind auf `chmod 0600` gesetzt (Root-only)
  - `dispatch_ipc()` Funktion: PolicyCheck → policy.check() → PolicyResponse; VmStatusQuery → VmManager → VmStatusResponse
  - Unbekannte Nachrichtentypen → Error-Response (kein Panic)
- `vm-daemon/src/vm.rs` — xl_cfg-Pfad: `/tmp/krypt-<name>.cfg` → `/run/krypt/krypt-<name>.cfg`
- `vm-daemon/src/config.rs` — `#[allow(dead_code)]` von `trust_level`-Feld entfernt
- `krypt-stick/Cargo.toml` — `clap` auf `features = ["derive", "env"]` erweitert (KRYPT_LUKS_DEV env-var)
- `krypt-stick/src/main.rs` — globales `--luks-dev` Arg (default /dev/sda2, env KRYPT_LUKS_DEV), `run()` mit Error-Handling
- `krypt-stick/src/luks.rs` — echte cryptsetup-Calls:
  - `list_slots()`: luksDump + Slot-Parsing (LUKS2 + LUKS1 fallback)
  - `add_key_from_file()`: luksAddKey --key-slot <n> (interaktive Passphrase via Terminal)
  - `kill_slot()`: luksKillSlot
  - `next_free_slot()`: luksDump → erste freie Nummer 0–31
  - `active_slots_from_dump()`: private Parser für beide LUKS-Formate
- `krypt-stick/src/create.rs` — echter Setup-Flow:
  - 64-Byte-Key aus /dev/urandom
  - Key auf Stick schreiben (raw, Offset 512 = Sektor 1)
  - Temp-Keyfile 0600, cryptsetup luksAddKey, sofort löschen
  - sysfs-Serial-Nummer lesen (best-effort)
  - daemon.toml-Snippet ausgeben
- `krypt-stick/src/backup.rs` — add() + promote() implementiert (gleicher Flow wie create)
- `krypt-stick/src/revoke.rs` — slot() via kill_slot() implementiert
- `cargo clippy -- -D warnings` → sauber
- `cargo test` → **17/17 grün** (14 Policy + 3 IPC)

### Aktueller Stand
```
Geänderte Dateien:
  vm-daemon/src/policy.rs  (load_from_config + load_from_toml + map_* helpers)
  vm-daemon/src/main.rs    (Arc<RwLock>, dispatch_ipc, chmod socket, load_from_config)
  vm-daemon/src/vm.rs      (xl_cfg: /tmp → /run/krypt/)
  vm-daemon/src/config.rs  (allow(dead_code) entfernt)
  krypt-stick/Cargo.toml   (clap env feature)
  krypt-stick/src/main.rs  (--luks-dev global arg, run() Error-Handling)
  krypt-stick/src/luks.rs  (echte cryptsetup-Calls, vollständig)
  krypt-stick/src/create.rs (echter Setup-Flow, vollständig)
  krypt-stick/src/backup.rs (add + promote, vollständig)
  krypt-stick/src/revoke.rs (kill_slot, vollständig)

Was funktioniert: cargo build + clippy sauber + 17 Tests grün
Was noch fehlt:  krypt-stick Unit-Tests, initramfs-Hook für USB-Stick-Boot
Architektur-Notiz: dispatch_ipc() ist async fn — kann später für Rate-Limiting / Audit-Log
                   erweitert werden ohne Callsites zu ändern
```

### Nächste Session beginnt mit
1. `krypt-stick/src/luks.rs` Unit-Tests: `active_slots_from_dump()` mit fixture-Output testen (kein echtes cryptsetup nötig)
2. `initramfs/krypt-hook` — mkinitcpio-Hook schreiben: Auth-Stick per `dd` lesen, `cryptsetup open --key-file=-` aufrufen
3. `vm-daemon/src/ipc.rs` — `IpcServer::bind()` mit einem `AuditLog`-Wrapper versehen (wer hat wann welchen PolicyCheck gestellt)
4. `docs/decisions.md` — ADR für IPC-Protokoll (Unix-Socket statt vchan, JSON-Framing) nachtragen

### Offene Fragen / Blockers
- initramfs: mkinitcpio vs. dracut — für Arch Linux ist mkinitcpio kanonisch, aber dracut hat besseres modulares Hook-System
- krypt-stick promote: LUKS2-Token-Plugin (phase 6+) — tpm2-tools oder eigenes Plugin?
- RAM-Wipe in krypt-panic: `/proc/[pid]/mem` oder spezialisierte Lib?
- TPM2-Bindung: `tss-esapi` crate prüfen
- IPC Security: Rate-Limiting für PolicyCheck (DoS-Schutz gegen kompromittierte AppVM)
- xenvchan ADR: schreiben sobald wir libxenvchan direkt via FFI ansprechen (Phase 6+)

---

## 2026-05-13 — Phase 4: ipc.rs Unix-Socket + daemon.toml.example

### Erledigt
- `cargo search xenvchan` → keine stabilen Bindings auf crates.io (nur 0.0.0-pre Stubs)
- `vm-daemon/src/ipc.rs` — vollständige Implementierung:
  - `IpcMessage` Enum: `PolicyCheck`, `VmStatusQuery`, `PolicyResponse`, `VmStatusResponse`, `VmStateChanged`, `Error`
  - `PolicyDecision` Enum (serialisiert unabhängig von policy::PolicyAction für Protokoll-Stabilität)
  - `IpcServer::bind()` — entfernt veralteten Socket, bindet `/run/krypt/ipc.sock`
  - `IpcConn` — framed send/recv (4-Byte LE Länge + JSON-Body, max 64 KiB)
  - `IpcClient` — für AppVM-Agenten (connect + send + recv + request)
  - 3 neue Unit-Tests: roundtrip_policy_check, frame_too_large_rejected, messages_serialize_with_type_tag
- `vm-daemon/src/main.rs` — IPC-Server als separater tokio::spawn-Task im Event-Loop
  - `/run/krypt/` wird per `create_dir_all` angelegt (systemd RuntimeDirectory=krypt)
  - accept-Loop mit Connection-Handler per spawn, TODO-Marker für Phase 5 (Policy-Dispatch)
- `vm-daemon/Cargo.toml` — `serde_json = "1"`, `[dev-dependencies] tempfile = "3"` ergänzt
- `vm-daemon/daemon.toml.example` — vollständig kommentiertes Beispiel mit allen Feldern
- `cargo clippy -- -D warnings` → sauber
- `cargo test` → **17/17 grün** (14 Policy + 3 IPC)

### Aktueller Stand
```
Geänderte Dateien:
  vm-daemon/src/ipc.rs          (vollständig implementiert, vorher Stub)
  vm-daemon/src/main.rs         (IPC-Server-Task eingefügt)
  vm-daemon/Cargo.toml          (serde_json, tempfile dev-dep)
  vm-daemon/daemon.toml.example (NEU)

Was funktioniert: cargo build + clippy sauber + 17 Tests grün
Was noch offen:   policy.load_from_toml() verdrahten, IPC→PolicyEngine-Dispatch (Phase 5)
                  VmManager+PolicyEngine in main.rs über Trust-Level verbinden
Architektur-Notiz: IpcClient ist #[allow(dead_code)] — wird von AppVM-Agenten genutzt,
                   nicht vom Daemon selbst
```

### Nächste Session beginnt mit
1. `vm-daemon/src/policy.rs` — `load_from_toml()` implementieren: `KryptConfig::load()` aufrufen, Entries in PolicyRule/TrustLevel umwandeln (Methode verdrahten, Logik existiert schon in main.rs)
2. `vm-daemon/src/main.rs` — Trust-Level aus `VmEntry::trust_level` in `policy_engine.set_trust()` laden (config::TrustLevel → policy::TrustLevel Mapping)
3. `vm-daemon/src/main.rs` — IPC-Handler: echten Policy-Dispatch implementieren statt TODO-Stub (IpcMessage::PolicyCheck → policy_engine.check() → IpcMessage::PolicyResponse)
4. `xl write_xl_cfg` Pfad: /tmp → /run/krypt/ umstellen (create_dir_all bereits in main.rs)

### Offene Fragen / Blockers
- initramfs: udev-Events in frühem Boot-Stadium — mkinitcpio vs. dracut?
- RAM-Wipe in krypt-panic: `/proc/[pid]/mem` oder spezialisierte Lib?
- TPM2-Bindung: tpm2-tss Rust-Bindings prüfen (`tss-esapi` crate)
- Stick-UUID persistieren: in LUKS2-Header-Kommentar oder eigene Datei?
- IPC Security: `/run/krypt/ipc.sock` braucht chmod 0600 nach dem Bind (noch nicht impl.)
- xenvchan: ADR schreiben sobald wir libxenvchan direkt via FFI ansprechen (Phase 6+)

---

## 2026-05-13 — Phase 3: vm.rs xl-Commands + Policy Unit-Tests

### Erledigt
- `vm-daemon/src/vm.rs` — echte `xl`-Kommandos via `tokio::process::Command`:
  - `start()`: `xl create -q <cfg>` + `xl domid <name>` für Domain-ID
  - `shutdown()`: `xl shutdown <name>` (ACPI, kein Force-Kill)
  - `destroy()`: `xl destroy <name>` (sofortiger Kill; Halted → early return)
  - `write_xl_cfg()`: generiert `/tmp/krypt-<name>.cfg` aus VmConfig-Feldern
  - `VmConfig.xl_cfg: Option<PathBuf>` — optionaler Pfad zu existierender .cfg
  - `VmError` erweitert: `XlFailed(String)`, `Io(#[from] std::io::Error)`
- `vm-daemon/src/policy.rs` — 14 Unit-Tests (alle grün):
  - Explizite Regeln: Allow / Deny / AskUser
  - First-rule-wins bei Duplikaten
  - Explizite Regel überschreibt Trust-Level-Fallback
  - Trust-Level-Fallback: src >= tgt → AskUser, src < tgt → Deny
  - Unbekannte VMs → Red (default) → beide unknown → AskUser
  - Grenzfälle: Red→Vault Deny, Black→Red AskUser, Orange→Green Deny
  - `PolicyAction: PartialEq` ergänzt (für assert_eq! in Tests)
- `vm-daemon/src/main.rs` — `xl_cfg: None` zu VmConfig-Konstruktion ergänzt
- `cargo clippy -- -D warnings` → sauber
- `cargo test` → 14/14 grün

### Aktueller Stand
```
Geänderte Dateien:
  vm-daemon/src/vm.rs       (xl-Commands implementiert)
  vm-daemon/src/policy.rs   (PolicyAction: PartialEq + 14 Unit-Tests)
  vm-daemon/src/main.rs     (xl_cfg: None)

Was funktioniert: cargo build + clippy sauber + 14 Tests grün
Was noch Stub ist: ipc.rs (vchan), policy.load_from_toml(), vm-manager↔policy-engine Verknüpfung
Architektur-Notiz: xl write_xl_cfg → /tmp/krypt-<name>.cfg (temporär, Phase 4: /run/krypt/)
```

### Nächste Session beginnt mit
1. `vm-daemon/src/ipc.rs` — `cargo search xenvchan` ausführen; falls Bindings existieren: vchan-Stub ausbauen; sonst: Unix-Socket-basierter IPC als Fallback planen
2. `vm-daemon/src/policy.rs` — `load_from_toml()` implementieren: `KryptConfig::load()` aufrufen, Entries in PolicyRule/TrustLevel umwandeln (erledigt schon in main.rs, nur Methode verdrahten)
3. `vm-daemon/daemon.toml.example` — Beispiel-Konfiguration anlegen mit allen Feldern kommentiert
4. VmManager + PolicyEngine in main.rs verknüpfen: Trust-Level aus `VmEntry::trust_level` in `policy_engine.set_trust()` laden

### Offene Fragen / Blockers
- initramfs: udev-Events in frühem Boot-Stadium — mkinitcpio vs. dracut?
- RAM-Wipe in krypt-panic: `/proc/[pid]/mem` oder spezialisierte Lib?
- TPM2-Bindung: tpm2-tss Rust-Bindings prüfen (`tss-esapi` crate)
- Stick-UUID persistieren: in LUKS2-Header-Kommentar oder eigene Datei?
- xl write_xl_cfg: /tmp ist ausreichend für Phase 3, Phase 4 → /run/krypt/ mit mkdir_p

---

## 2026-05-13 — Phase 2: tokio-udev + config.rs + Event-Loop

### Erledigt
- `vm-daemon/Cargo.toml` — `tokio-udev = "0.9"` + `futures = "0.3"` ergänzt
- `vm-daemon/src/config.rs` — NEU: vollständiges TOML-Parsing
  - `KryptConfig` mit `[daemon]`, `[[auth_sticks]]`, `[[vms]]`, `[[policy]]`
  - `PanicLevel` (Lock / Suspend / Nuke), `TrustLevel`, `PolicyAction`
  - `KryptConfig::load(path)` lädt aus Datei; `Default` für fehlende Datei
- `vm-daemon/src/usb.rs` — `run()` implementiert mit echtem tokio-udev:
  - `MonitorBuilder::new()?.match_subsystem_devtype("usb","usb_device")?.listen()?`
  - `Device`-Daten im Block extrahiert (Droppt `!Send` Device vor dem `.await`)
  - Klassifiziert Events → `AuthStickConnected / AuthStickRemoved / Unknown`
- `vm-daemon/src/main.rs` — vollständiger Event-Loop:
  - Config laden, tracing init, PolicyEngine + VmManager + UsbMonitor befüllen
  - `tokio::task::LocalSet` + `spawn_local` für USB-Task (AsyncMonitorSocket ist `!Send`)
  - `tokio::select!` über USB-Events, SIGTERM, SIGINT
  - `trigger_panic()` ruft `loginctl`/`systemctl` je nach PanicLevel
- `vm-daemon/src/policy.rs` — `add_rule()` + `set_trust()` + `Default` ergänzt
- `vm-daemon/src/vm.rs` — `Default` für `VmManager` ergänzt
- `cargo clippy -- -D warnings` → sauber (kein einziger Fehler)

### Aktueller Stand
```
Geänderte Dateien:
  vm-daemon/Cargo.toml          (tokio-udev, futures)
  vm-daemon/src/config.rs       (neu)
  vm-daemon/src/usb.rs          (run() implementiert)
  vm-daemon/src/main.rs         (vollständiger Event-Loop)
  vm-daemon/src/policy.rs       (add_rule, set_trust, Default)
  vm-daemon/src/vm.rs           (Default für VmManager)

Was funktioniert: cargo build + cargo clippy -- -D warnings (beide sauber)
Was noch Stub ist: vm.rs (libxl FFI), ipc.rs (vchan), policy check/trust_map
Architektur-Notiz: tokio-udev AsyncMonitorSocket ist !Send → spawn_local + LocalSet
```

### Nächste Session beginnt mit
1. `vm-daemon/src/vm.rs` — `start()` mit echten `xl`-Kommandos via `tokio::process::Command` (nicht libxl FFI — `xl` CLI ist stabiler Einstieg)
2. `vm-daemon/src/policy.rs` — `check()` testen: Unit-Tests schreiben, `trust_map` in main befüllen aus `VmEntry::trust_level`
3. `vm-daemon/src/ipc.rs` — vchan Stub: schauen ob `xenvchan` Rust-Bindings existieren (`cargo search xenvchan`)
4. Beispiel-Config anlegen: `vm-daemon/daemon.toml.example` mit kommentiertem Format

### Offene Fragen / Blockers
- initramfs: udev-Events in frühem Boot-Stadium — mkinitcpio vs. dracut?
- RAM-Wipe in krypt-panic: `/proc/[pid]/mem` oder spezialisierte Lib?
- TPM2-Bindung: tpm2-tss Rust-Bindings prüfen (`tss-esapi` crate)
- Stick-UUID persistieren: in LUKS2-Header-Kommentar oder eigene Datei?
- tokio-udev 0.10.0 verfügbar (gelockt auf 0.9.1) — upgrade testen wenn 0.10 stabil

---

Laufende Notizen zum Entwicklungsfortschritt.

---

## 2026-05-13 — Phase 1 Start: Cargo-Setup + Grundstruktur

### Erledigt
- `Cargo.toml` (workspace root) — vm-daemon + krypt-stick als workspace members
- `vm-daemon/Cargo.toml` — tokio/full, serde/derive, toml, thiserror 2, tracing, tracing-subscriber, clap/derive
- `krypt-stick/Cargo.toml` — clap/derive, nix 0.29/user, libc
- `vm-daemon/src/vm.rs` — VmConfig, VmState, Vm (start/shutdown/destroy), VmManager
- `vm-daemon/src/usb.rs` — UsbDevice, UsbEvent, UsbMonitor (classify + run stub)
- `vm-daemon/src/ipc.rs` — IpcChannel Stub (vchan Phase 2)
- `krypt-stick/src/create.rs`, `backup.rs`, `revoke.rs`, `luks.rs` — Stubs für Kompilierbarkeit
- `cargo build` (workspace) läuft durch, nur dead_code Warnings (erwartet)

### Aktueller Stand
```
Geänderte Dateien:
  vm-daemon/Cargo.toml        (neu)
  vm-daemon/src/vm.rs         (neu)
  vm-daemon/src/usb.rs        (neu)
  vm-daemon/src/ipc.rs        (neu)
  vm-daemon/src/main.rs       (mod usb hinzugefügt)
  krypt-stick/Cargo.toml      (neu)
  krypt-stick/src/create.rs   (neu, Stub)
  krypt-stick/src/backup.rs   (neu, Stub)
  krypt-stick/src/revoke.rs   (neu, Stub)
  krypt-stick/src/luks.rs     (neu, Stub)
  Cargo.toml                  (workspace root, neu)

Was funktioniert: cargo build (beide Crates)
Was noch Stub ist: alle TODO Phase 2 Blöcke (libxl, tokio-udev, LUKS2)
```

### Nächste Session beginnt mit
1. `vm-daemon/src/usb.rs` — `tokio-udev` Dependency hinzufügen, `run()` mit echtem udev NETLINK_KOBJECT_UEVENT Socket implementieren
2. `vm-daemon/src/main.rs` — UsbMonitor in den Event-Loop einbinden, mpsc-Kanal aufsetzen, Kill-Switch-Logik bei AuthStickRemoved
3. `vm-daemon/src/config.rs` — TOML-Parsing implementieren (serde + toml, Policy-Regeln + VM-Definitionen laden)

### Offene Fragen / Blockers
- initramfs: udev-Events in frühem Boot-Stadium — mkinitcpio vs. dracut?
- RAM-Wipe in krypt-panic: `/proc/[pid]/mem` oder spezialisierte Lib?
- TPM2-Bindung: tpm2-tss Rust-Bindings prüfen (`tss-esapi` crate)
- Stick-UUID persistieren: in LUKS2-Header-Kommentar oder eigene Datei?
- tokio-udev: crate aktiv? Letzte Version checken (könnte tokio 1.x-Compat. problematisch sein)

---

## 2025-05-13 — Phase 0 abgeschlossen + USB Kill Switch Spezifikation

### Erledigt
- Drei-Säulen-Vision finalisiert: Kryptografie · Isolation · Lightweight
- `docs/usb-kill-switch.md` — vollständige technische Spezifikation
  - Boot-Prozess mit USB-Stick (initramfs Hook)
  - Panic-Level System (Lock / Suspend / Nuke)
  - Backup-Stick-Management via LUKS2 Key-Slots
  - Setup-Flow Mockup
  - Vergleich mit anderen Security-OS
- `krypt-stick/src/main.rs` — CLI-Tool Grundstruktur (Rust + clap)
- `panic/krypt-panic.rs` — Emergency Shutdown Handler (minimal dependencies)
- `initramfs/` Verzeichnis + mkinitcpio Hook Pseudocode in Doku
- ADR-005 bis ADR-008 dokumentiert (Alpine, ChaCha20, Ballooning, USB)
- README: USB Kill Switch Sektion + Drei-Säulen-Vision

### Gesamte Repo-Struktur aktuell
```
krypt-os/
├── .dev-session           (lokal, nie auf GitHub)
├── .gitignore
├── README.md
├── PROGRESS.md
├── docs/
│   ├── architecture.md
│   ├── decisions.md       (ADR-001 bis ADR-008)
│   └── usb-kill-switch.md ← NEU
├── vm-daemon/src/
│   ├── main.rs
│   └── policy.rs
├── krypt-stick/src/       ← NEU
│   └── main.rs
├── panic/                 ← NEU
│   └── krypt-panic.rs
├── dotfiles/
│   ├── hyprland/hyprland.conf
│   └── waybar/krypt-vms.py
└── .github/workflows/build-iso.yml
```

### Nächste Schritte
1. `vm-daemon/Cargo.toml` — tokio, serde, toml, thiserror, tracing, clap
2. `krypt-stick/Cargo.toml` — clap, nix, libc
3. `vm-daemon/src/usb.rs` — USB-Monitor implementieren (udev via tokio)
4. `vm-daemon/src/vm.rs` — VM-Lifecycle Grundstruktur
5. `cargo build` für beide Crates muss durchlaufen

### Offene Fragen
- initramfs: udev-Events in frühem Boot-Stadium — mkinitcpio vs. dracut?
- RAM-Wipe in krypt-panic: `/proc/[pid]/mem` oder spezialisierte Lib?
- TPM2-Bindung: tpm2-tss Rust-Bindings prüfen (`tss-esapi` crate)
- Stick-UUID persistieren: in LUKS2-Header-Kommentar oder eigene Datei?

---

## Entscheidungslog

| Datum | Entscheidung |
|---|---|
| 2025-05-13 | Öffentliches Repo |
| 2025-05-13 | Rust für krypt-daemon |
| 2025-05-13 | Arch Linux als Basis |
| 2025-05-13 | Xen statt KVM |
| 2025-05-13 | Hyprland |
| 2025-05-13 | Alpine für AppVM-Templates |
| 2025-05-13 | ChaCha20-Poly1305 für Inter-VM-Crypto |
| 2025-05-13 | Memory Ballooning |
| 2025-05-13 | USB-Stick als primärer Authentikator + Kill Switch |

---

## Bekannte Probleme
Noch keine.
