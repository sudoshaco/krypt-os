# Krypt OS — USB Kill Switch & Hardware Authentication

## Konzept

Der USB-Stick ist kein Passwort-Ersatz. Er ist der physische Schlüssel zur Existenz des Systems.

```
Stick drin   →  System läuft. Normal. Kein Passwort nötig.
Stick raus   →  Sofort. Alles. Verschlüsselt. Aus.
Kein Stick   →  Kein Boot. Punkt.
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
Level 1 — LOCK (schnellste Reaktion, kein Datenverlust)
  - Alle AppVM-Bildschirme sofort schwarz
  - Hyprland gesperrt
  - RAM-Keys in dom0 gelöscht
  - System wartet auf Stick-Wiedereinlegen
  - Zeit bis Level 2: konfigurierbar (Standard: 30 Sekunden)

Level 2 — SUSPEND (sicher, wiederherstellbar)
  - Alle AppVMs pausiert (Xen suspend)
  - RAM-Inhalt auf verschlüsselte Swap geschrieben
  - System suspended to disk
  - Nur mit Stick + optionalem Passwort aufweckbar

Level 3 — NUKE (keine Wiederherstellung ohne Stick)
  - Alle AppVMs sofort killed (kein graceful shutdown)
  - RAM mehrfach überschrieben (Cold-Boot-Attack-Schutz)
  - System shutdown
  - Nur mit Stick neu bootbar
```

Standardkonfiguration: Level 3 nach 5 Sekunden ohne Stick.
Konfiguration in `/etc/krypt/panic.toml`:

```toml
[panic]
level = 3
delay_seconds = 5
ram_wipe = true
ram_wipe_passes = 3
```

---

## Backup-Sticks

LUKS2 unterstützt bis zu 32 Key-Slots. Jeder Backup-Stick bekommt einen eigenen Slot.

### Backup-Stick erstellen (im laufenden System)

```bash
krypt-stick --add-backup
```

```
Ausgabe:
  [krypt] Stecke den neuen Backup-Stick ein...
  [krypt] Stick erkannt: Samsung 64GB (UUID: a3f2...)
  [krypt] Generiere Schlüsselmaterial...
  [krypt] Schreibe .krypt Datei...
  [krypt] Registriere LUKS2 Key-Slot 2...
  [krypt] Backup-Stick bereit. Slot: 2/32
  [krypt] Bewahre ihn getrennt vom Primär-Stick auf.
```

### Backup-Stick widerrufen

```bash
krypt-stick --revoke --slot 2
```

Sofortige Wirkung. Der widerrufene Stick kann das System nie wieder booten.

### Stick-Übersicht

```bash
krypt-stick --list

Slot 0  [PRIMÄR]   Kingston 32GB    UUID: 9a1b...  Hinzugefügt: 2025-05-01
Slot 1  [BACKUP]   Samsung 64GB     UUID: a3f2...  Hinzugefügt: 2025-05-13
Slot 2  [BACKUP]   SanDisk 16GB     UUID: c8e4...  Hinzugefügt: 2025-05-13
```

---

## Was passiert wenn der Stick verloren geht

Kurze Antwort: Der Angreifer hat einen USB-Stick mit einer versteckten Datei.
Er weiß nicht was das ist. Er kann damit nichts anfangen.
Das System ist LUKS2-verschlüsselt und startet nicht ohne den Stick.

Längere Antwort: Wenn du keinen Backup-Stick hast, ist das System unzugänglich.
Das ist kein Bug. Das ist Krypt.

Wenn du einen Backup-Stick hast:
1. Backup-Stick einlegen → System bootet
2. Verlorenen Slot sofort widerrufen: `krypt-stick --revoke --slot 0`
3. Neuen Primär-Stick erstellen: `krypt-stick --add-backup --promote`

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
