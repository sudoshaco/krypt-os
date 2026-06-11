# Krypt OS — USB Kill Switch & Hardware Authentication

## Konzept

Der USB-Stick ist kein Passwort-Ersatz. Er ist der physische Schlüssel zur Existenz des Systems.

```
Stick drin   →  System läuft. 
Stick raus   →  Sofort alles Verschlüsselt.
Kein Stick   →  Kein Boot. 
```

Kein anderes OS macht das so. Das ist Krypt.

---

## Was auf dem Stick ist

Der Stick sieht für jeden der ihn findet normal aus.

```
USB-Stick (Außenansicht für Finder):
  /DCIM/         ← normale Fotos
  /Documents/    ← normale Dateien
  /.krypt        ← versteckte Datei — der eigentliche Schlüssel
```

`.krypt` enthält:
- 512 Byte kryptografisches Schlüsselmaterial (zufällig generiert bei Setup)
- Signiert mit dem TPM2 des jeweiligen Geräts (optional: Stick funktioniert nur auf DIESEM Gerät)
- Oder: nur Schlüsselmaterial ohne TPM-Bindung (Stick funktioniert auf jedem Krypt-System)

Der User entscheidet bei der Einrichtung welches Modell er will.

---

## Boot-Prozess mit USB-Stick

```
1. UEFI / Secure Boot verifiziert Bootloader
2. krypt-initramfs startet (vor allem anderen)
3. initramfs sucht USB-Stick mit .krypt Datei
       → Nicht gefunden: "Krypt Authentication Required"
                          Cursor blinkt. Nichts passiert. Für immer.
       → Gefunden: Schlüsselmaterial lesen
4. LUKS2 Entschlüsselung mit Stick-Schlüssel
       → Falsche Datei / manipuliert: Fehlschlag → Neustart
       → Korrekt: System entschlüsselt, Boot weiter
5. Xen startet, dom0 bootet
6. krypt-daemon registriert Stick-UUID als "aktiver Authentikator"
7. udev-Monitor läuft → wartet auf USB-Events
```

Ohne Stick: System bleibt auf Stufe 3. Kein Passwort-Fallback. Kein Recovery-Modus.
Das ist eine Designentscheidung, keine Limitation.

---

## Runtime: USB-Monitor

`krypt-daemon` überwacht permanent den USB-Bus.

```rust
// Pseudocode — wird in ipc.rs / usb.rs implementiert
loop {
    if authenticated_stick_removed() {
        trigger_panic(PanicLevel::from_config())
    }
}
```

### Panic-Level (konfigurierbar bei Setup)

```
"lock" — LOCK (schnellste Reaktion, kein Datenverlust)
  - loginctl lock-sessions: Wayland-Session sperrt sich
  - AppVMs laufen weiter (laut by-design)
  - Implementiert in panic::panic_lock (hyprctl dispatch exec hyprlock)
  - Fallback im Daemon: loginctl lock-sessions

"suspend" — SUSPEND (sicher, wiederherstellbar)
  - AppVMs werden via xl pause eingefroren
  - System suspended-to-RAM (S3 — /sys/power/state = "mem")
  - Wieder-Aufwachen via Stick einstecken + Wake-Event
  - HINWEIS: NICHT suspended-to-disk (S4) — das würde den
    LUKS-Key auf Swap schreiben. Siehe commit c91895e.

"nuke" — NUKE (keine Wiederherstellung ohne Stick)
  - Alle AppVMs via xl destroy zerrissen (kein graceful shutdown)
  - RAM-Wipe: Phase-5-Stub (placeholder, noch kein Cold-Boot-Schutz)
  - systemctl poweroff --force + libc::reboot als Hammer
  - Nur mit Stick neu bootbar (krypt-Hook im initramfs)
```

Standardkonfiguration: `panic_level = "suspend"`. Delay nicht konfigurierbar —
die Reaktion erfolgt im selben Event-Loop-Tick wie das UDEV-Remove-Event
(typisch <50ms).

Konfiguration in `/etc/krypt/daemon.toml` (Format gemäß
[`vm-daemon/daemon.toml.example`](../vm-daemon/daemon.toml.example)):

```toml
[daemon]
log_level   = "info"
panic_level = "suspend"          # "lock" | "suspend" | "nuke"
```

**Was es NICHT gibt** (war im alten Doc behauptet, ist nie implementiert worden):
  - `delay_seconds`: kein Timer, sofortige Reaktion (kein Wartebudget)
  - `ram_wipe_passes`: RAM-Wipe ist noch Stub (`panic::wipe_sensitive_memory`,
    "memory wipe: placeholder (Phase 5)")
  - `[panic]`-Section: das gesamte File heißt `daemon.toml`, Panic-Setting
    lebt unter `[daemon]`

---

## Backup-Sticks

LUKS2 unterstützt bis zu 32 Key-Slots. Jeder Backup-Stick bekommt einen eigenen Slot.

### Backup-Stick erstellen (im laufenden System)

Voraussetzung: das LUKS-Mapping muss offen sein (`/dev/mapper/krypt-root`).
Der `--stick-dev` ist das Block-Device des neuen Backup-Sticks, nicht des
Primärsticks.

```bash
sudo krypt-stick \
    --luks-dev /dev/mapper/krypt-root \
    add-backup --stick-dev /dev/sdY
```

Beispiel-Ausgabe (krypt-stick/src/backup.rs):

```
Adding backup stick /dev/sdY → LUKS device /dev/mapper/krypt-root
Key added to slot 2 on /dev/mapper/krypt-root

Backup stick added — LUKS slot 2
Add to /etc/krypt/daemon.toml:
  [[auth_sticks]]
  serial = "<udevadm info --query=property --name=/dev/sdY | grep ID_SERIAL_SHORT>"
  luks_slot = 2
```

`krypt-stick` ergänzt den daemon.toml-Block NICHT selbst — der Output
zeigt dir was du anhängen musst.

### Backup-Stick widerrufen (Slot löschen)

```bash
sudo krypt-stick \
    --luks-dev /dev/mapper/krypt-root \
    revoke 2
```

Sofortige Wirkung — `cryptsetup luksKillSlot` fragt zur Bestätigung nach
einer ANDEREN gültigen Passphrase. Der widerrufene Stick kann das System
nie wieder booten.

### Stick-Übersicht (aktive LUKS2-Slots)

```bash
sudo krypt-stick --luks-dev /dev/mapper/krypt-root list
```

```
LUKS2 device: /dev/mapper/krypt-root
  Slot 0: ENABLED
  Slot 1: ENABLED
  Slot 2: ENABLED
```

LUKS2 selbst speichert keine Stick-Metadata (Modell, Hinzufügedatum etc.) —
die Zuordnung Slot → Stick steht ausschließlich in `daemon.toml`. Wer eine
Mapping-Notiz braucht: dort kommentieren.

---

## Was passiert wenn der Stick verloren geht

Der Angreifer hat einen USB-Stick mit einer versteckten Datei.
Er weiß nicht was das ist. Er kann damit nichts anfangen.
Das System ist LUKS2-verschlüsselt und startet nicht ohne den Stick.

Wenn du keinen Backup-Stick hast, ist das System unzugänglich.
Das ist kein Bug. Das ist Krypt.

Wenn du einen Backup-Stick hast:
1. Backup-Stick einlegen → System bootet
2. LUKS-Mapping öffnen falls nicht offen:
   `sudo cryptsetup open /dev/sda2 krypt-root` (Backup-Stick liefert die Key-Datei)
3. Verlorenen Slot widerrufen:
   `sudo krypt-stick --luks-dev /dev/mapper/krypt-root revoke 0`
4. Neuen Primär-Stick einrichten:
   `sudo krypt-stick --luks-dev /dev/mapper/krypt-root add-backup --stick-dev /dev/sdZ`
5. `daemon.toml` `[[auth_sticks]]` Einträge entsprechend aktualisieren (alten Stick raus, neuen rein)

---

## Technische Implementierung

### Komponenten

```
krypt-os/
├── initramfs/
│   ├── krypt-boot-agent.sh     ← Stick-Suche beim Boot
│   └── hooks/
│       └── krypt               ← mkinitcpio Hook
│
├── vm-daemon/src/
│   └── usb.rs                  ← Runtime USB-Monitor
│
├── krypt-stick/                ← CLI-Tool für Stick-Management
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── create.rs           ← Stick erstellen
│       ├── backup.rs           ← Backup-Sticks verwalten
│       ├── revoke.rs           ← Slot widerrufen
│       └── luks.rs             ← LUKS2-Key-Slot-Management
│
└── panic/
    └── krypt-panic.rs          ← Panic-Handler (separates Binary)
```

### initramfs Hook (mkinitcpio)

```bash
# /etc/initcpio/hooks/krypt
run_hook() {
    local stick_found=0
    local mount_point="/run/krypt-auth"

    mkdir -p "$mount_point"

    # Alle USB-Geräte durchsuchen
    for dev in /dev/disk/by-id/usb-*; do
        [ -b "$dev" ] || continue

        # Partitionen durchsuchen
        for part in "${dev}"-part* "$dev"; do
            [ -b "$part" ] || continue
            mount -o ro "$part" "$mount_point" 2>/dev/null || continue

            if [ -f "$mount_point/.krypt" ]; then
                stick_found=1
                KRYPT_KEYFILE="$mount_point/.krypt"
                break 2
            fi

            umount "$mount_point" 2>/dev/null
        done
    done

    if [ "$stick_found" -eq 0 ]; then
        # Kein Stick gefunden — warten
        echo ""
        echo "  ██╗  ██╗██████╗ ██╗   ██╗██████╗ ████████╗"
        echo "  ██║ ██╔╝██╔══██╗╚██╗ ██╔╝██╔══██╗╚══██╔══╝"
        echo "  █████╔╝ ██████╔╝ ╚████╔╝ ██████╔╝   ██║   "
        echo "  ██╔═██╗ ██╔══██╗  ╚██╔╝  ██╔═══╝    ██║   "
        echo "  ██║  ██╗██║  ██║   ██║   ██║        ██║   "
        echo "  ╚═╝  ╚═╝╚═╝  ╚═╝   ╚═╝   ╚═╝        ╚═╝   "
        echo ""
        echo "  Authentication required."
        echo "  Insert your Krypt stick to continue."
        echo ""

        while [ "$stick_found" -eq 0 ]; do
            sleep 1
            # Nochmal prüfen — warten bis Stick eingesteckt wird
            # (udev events in initramfs)
        done
    fi

    # Schlüssel an LUKS übergeben
    cryptsetup open /dev/disk/by-uuid/"$KRYPT_ROOT_UUID" \
        krypt-root \
        --key-file "$KRYPT_KEYFILE" \
        --keyfile-size 512
}
```

### usb.rs — Runtime Monitor in krypt-daemon

```rust
// usb.rs — USB-Kill-Switch Runtime Monitor
use std::path::Path;
use tokio::time::{sleep, Duration};

pub struct UsbMonitor {
    authenticated_uuid: String,
    panic_level: PanicLevel,
    panic_delay: Duration,
}

#[derive(Clone)]
pub enum PanicLevel {
    Lock,
    Suspend,
    Nuke,
}

impl UsbMonitor {
    pub async fn run(&self) {
        loop {
            if !self.stick_present() {
                log::warn!("AUTH STICK REMOVED — panic in {:?}", self.panic_delay);
                sleep(self.panic_delay).await;

                // Nochmal prüfen (versehentliches Ziehen)
                if !self.stick_present() {
                    self.trigger_panic().await;
                }
            }
            sleep(Duration::from_millis(200)).await;
        }
    }

    fn stick_present(&self) -> bool {
        Path::new(&format!(
            "/dev/disk/by-uuid/{}",
            self.authenticated_uuid
        )).exists()
    }

    async fn trigger_panic(&self) {
        match self.panic_level {
            PanicLevel::Lock    => self.panic_lock().await,
            PanicLevel::Suspend => self.panic_suspend().await,
            PanicLevel::Nuke    => self.panic_nuke().await,
        }
    }

    async fn panic_nuke(&self) {
        // 1. Alle AppVMs sofort stoppen
        // 2. RAM-Keys überschreiben
        // 3. Shutdown
        log::error!("PANIC NUKE — initiating");
        // krypt-panic Binary übernimmt (separater Prozess, minimal dependencies)
        std::process::Command::new("/usr/bin/krypt-panic")
            .arg("--level=nuke")
            .spawn()
            .unwrap();
    }
}
```

---

## Setup-Flow für den User

```
[Krypt Installer]

  Schritt 7 von 9: Hardware-Authentifizierung

  Stecke deinen USB-Stick ein.
  Er muss mindestens 1 MB freien Speicher haben.
  Deine Dateien auf dem Stick bleiben erhalten.

  Erkannter Stick: [Kingston DataTraveler 32GB] ✓

  Schlüssel-Modus:
  ○ Nur USB-Stick         (einfach, Stick = alles)
  ● USB-Stick + PIN       (sicherer, 2 Faktoren)
  ○ USB-Stick + Passwort  (maximale Sicherheit)

  Panic-Verhalten bei Stick-Entfernung:
  ○ Bildschirm sperren
  ○ System suspenden
  ● Sofort herunterfahren  (empfohlen)

  Verzögerung: [5] Sekunden

  [Backup-Stick jetzt erstellen? Ja / Überspringen]

  → Weiter
```

---

## Warum das niemand sonst so macht

QubesOS: Passwort beim Boot. Kein Hardware-Kill-Switch integriert.
Tails: Kein persistentes System. Kein Kill-Switch.
VeraCrypt + Panic-Scripts: Bastelarbeit, nicht tief integriert.
LUKS + USB: Existiert, aber kein Runtime-Monitor, kein Panic-Level-System.

Krypt verbindet:
- Boot-Authentifizierung via USB (LUKS2 Keyfile)
- Runtime-Monitor mit konfigurierbarem Panic-Level
- Xen-aware Panic (VMs werden sofort isoliert/gestoppt)
- Backup-Stick-Management mit Widerruf
- TPM2-Bindung optional (Stick funktioniert nur auf diesem Gerät)
- Normaler Stick-Außenauftritt (kein "ich bin ein Security-Dongle" Look)

Das gibt es nicht. Das ist Krypt.
