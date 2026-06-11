// create.rs — Primär-Stick Setup
//
// Flow:
//   1. 64-Byte-Key aus /dev/urandom generieren
//   2. Key auf den Stick schreiben (raw, Offset 512 = Sektor 1, hinter MBR/GPT)
//   3. cryptsetup luksAddKey: Key zum LUKS2-Device hinzufügen
//      (User gibt bestehendes Passwort interaktiv ein — cryptsetup-Prompt)
//   4. Slot-Nummer ermitteln, Serien-Nummer lesen, daemon.toml-Snippet ausgeben
//
// Key-Layout auf dem Stick (raw, kein Filesystem nötig):
//   Offset   0 –  511:  MBR / Partition-Table (unangetastet)
//   Offset 512 –  575:  64-Byte LUKS-Schlüsselmaterial
//
// Kompatibel mit initramfs-Hook:
//   dd if=/dev/<stick> bs=1 skip=512 count=64 | cryptsetup open --key-file=-
//
// SICHERHEIT:
//   - Temp-Keyfile wird in /tmp/ mit 0600 erstellt und sofort nach luksAddKey gelöscht
//   - Der Key-Bereich (Bytes 512–575) bleibt ausschließlich auf dem Stick

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;

const KEY_OFFSET: u64 = 512;
const KEY_LEN: usize = 64;
const TMP_KEY: &str = "/tmp/.krypt-setup-key";

pub fn run_setup(luks_dev: &str, stick_dev: &str, force: bool) -> crate::luks::Result<()> {
    // ── Voraussetzungen prüfen ─────────────────────────────────────────────
    if !std::path::Path::new(luks_dev).exists() {
        return Err(format!("LUKS device nicht gefunden: {luks_dev}").into());
    }
    if !std::path::Path::new(stick_dev).exists() {
        return Err(format!("Stick device nicht gefunden: {stick_dev}").into());
    }

    println!("Krypt OS — USB Auth-Stick Setup");
    println!("  LUKS device : {luks_dev}");
    println!("  Stick device: {stick_dev}");
    println!();

    // ── GPT-Schutz ─────────────────────────────────────────────────────────
    // Offset 512–575 liegt MITTEN IM GPT-Primärheader (LBA 1). Würden wir
    // dort 64 Byte hineinschreiben, ist die GPT zerstört: GPT-magic ("EFI
    // PART" @ Offset 512–519) wäre weg, partitionierte Sticks würden ihren
    // gesamten Inhalt verlieren. Vorher hieß die Warnung "Bestehende Daten
    // bleiben erhalten (nur Sektor 1)" — auf GPT-Sticks gelogen.
    if has_gpt_at_offset_512(stick_dev)? {
        return Err(format!(
            "{stick_dev} hat eine GPT-Partitionstabelle (EFI PART @ Offset 512). \
             Schreiben würde sie zerstören. \
             Vorgehen: 'sgdisk --zap-all {stick_dev}' (wischt GPT) oder einen \
             anderen Stick verwenden. Daten auf dem Stick gehen verloren."
        ).into());
    }

    // ── Bestätigung ────────────────────────────────────────────────────────
    if !force {
        eprintln!("WARNUNG: 64 Bytes werden auf {stick_dev} ab Offset 512 geschrieben.");
        eprintln!("         MBR-Sektor (0–511) bleibt erhalten. Falls eine erste");
        eprintln!("         Partition auf LBA 1 anfängt (selten — meist LBA 2048),");
        eprintln!("         werden deren ersten 64 Bytes überschrieben.");
        eprint!("Fortfahren? [y/N] ");
        io::stdout().flush().ok();
        let mut answer = String::new();
        io::stdin().read_line(&mut answer)?;
        let answer = answer.trim().to_lowercase();
        if answer != "y" && answer != "yes" {
            println!("Abgebrochen.");
            return Ok(());
        }
    }

    // ── 1. 64-Byte-Key aus /dev/urandom ───────────────────────────────────
    let mut key = vec![0u8; KEY_LEN];
    File::open("/dev/urandom")?.read_exact(&mut key)?;
    println!("Schlüsselmaterial generiert ({KEY_LEN} Bytes).");

    // ── 2. Key auf Stick schreiben (Sektor 1, Offset 512) ─────────────────
    {
        let mut stick = OpenOptions::new()
            .write(true)
            .open(stick_dev)
            .map_err(|e| format!("Stick öffnen fehlgeschlagen ({stick_dev}): {e}"))?;
        stick.seek(SeekFrom::Start(KEY_OFFSET))?;
        stick.write_all(&key)?;
        stick.flush()?;
        // sync auf Block-Device — wichtig vor luksAddKey (via libc).
        // Wenn fsync schlägt fehl (I/O-Error auf Stick, schreibgeschützt),
        // dann ist der Key nur im Cache, nicht persistent — bei Stromausfall
        // vor nächstem sync wäre der LUKS-Slot tot. Hart abbrechen statt
        // dem User vorzugaukeln, der Setup wäre erfolgreich.
        if unsafe { libc::fsync(stick.as_raw_fd()) } != 0 {
            let err = io::Error::last_os_error();
            return Err(format!("fsync auf {stick_dev} fehlgeschlagen: {err}").into());
        }
    }
    println!("Schlüssel auf {stick_dev} geschrieben (Offset {KEY_OFFSET}).");

    // ── 3. Temp-Keyfile (0600) für cryptsetup ─────────────────────────────
    {
        let mut tmp = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(TMP_KEY)
            .map_err(|e| format!("Temp-Keyfile erstellen fehlgeschlagen: {e}"))?;
        tmp.write_all(&key)?;
        tmp.flush()?;
    }

    // ── 4. cryptsetup luksAddKey ───────────────────────────────────────────
    println!();
    println!("cryptsetup luksAddKey — bitte bestehende Passphrase eingeben:");
    let slot_result = crate::luks::add_key_from_file(luks_dev, std::path::Path::new(TMP_KEY));

    // Temp-Keyfile immer löschen, auch bei Fehler
    if let Err(rm_err) = std::fs::remove_file(TMP_KEY) {
        eprintln!("WARNUNG: Temp-Keyfile konnte nicht gelöscht werden ({TMP_KEY}): {rm_err}");
    }

    let slot = slot_result?;

    // ── 5. Serien-Nummer lesen (best-effort) ───────────────────────────────
    let serial = read_stick_serial(stick_dev)
        .unwrap_or_else(|_| detect_serial_via_udevadm(stick_dev)
        .unwrap_or_else(|_| "<serial unbekannt — `udevadm info --name=STICK | grep ID_SERIAL_SHORT`>".to_string()));

    // ── 6. Ergebnis ausgeben ──────────────────────────────────────────────
    println!();
    println!("✓ Auth-Stick eingerichtet — LUKS Slot {slot}");
    println!();
    println!("Füge folgendes zu /etc/krypt/daemon.toml hinzu:");
    println!("─────────────────────────────────────────────");
    println!("[[auth_sticks]]");
    println!("serial    = \"{serial}\"");
    println!("luks_slot = {slot}");
    println!("─────────────────────────────────────────────");
    println!();
    println!("Test — Boot ohne Stick prüfen:");
    println!("  cryptsetup open --key-file=- {luks_dev} test-open \\");
    println!("    < <(dd if={stick_dev} bs=1 skip={KEY_OFFSET} count={KEY_LEN} 2>/dev/null)");
    println!("  cryptsetup close test-open");

    Ok(())
}

/// Liest die USB-Serien-Nummer aus sysfs (`/sys/block/<dev>/device/serial`).
fn read_stick_serial(dev: &str) -> crate::luks::Result<String> {
    let dev_name = std::path::Path::new(dev)
        .file_name()
        .ok_or("ungültiger Device-Pfad")?
        .to_string_lossy()
        .into_owned();

    // Partitions-Suffix entfernen: sdb1 → sdb, nvme0n1p1 → nvme0n1
    let base = strip_partition_suffix(&dev_name);

    let serial_path = format!("/sys/block/{base}/device/serial");
    let serial = std::fs::read_to_string(&serial_path)
        .map(|s| s.trim().to_owned())?;

    if serial.is_empty() {
        return Err("leere Serien-Nummer in sysfs".into());
    }
    Ok(serial)
}

/// Fallback: Serien-Nummer via `udevadm info`.
fn detect_serial_via_udevadm(dev: &str) -> crate::luks::Result<String> {
    let out = std::process::Command::new("udevadm")
        .args(["info", "--query=property", &format!("--name={dev}")])
        .output()?;

    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("ID_SERIAL_SHORT=") {
            let s = rest.trim().to_owned();
            if !s.is_empty() {
                return Ok(s);
            }
        }
    }
    Err("ID_SERIAL_SHORT nicht in udevadm-Output".into())
}

/// Prüft Bytes 512..520 (Beginn von LBA 1) auf den GPT-Magic-String "EFI PART".
/// Wenn vorhanden, wäre der vorgesehene Key-Bereich identisch mit dem
/// GPT-Primärheader und ein Schreiben würde die Partitionstabelle vernichten.
fn has_gpt_at_offset_512(stick_dev: &str) -> crate::luks::Result<bool> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::OpenOptions::new().read(true).open(stick_dev)?;
    f.seek(SeekFrom::Start(512))?;
    let mut magic = [0u8; 8];
    f.read_exact(&mut magic)?;
    Ok(&magic == b"EFI PART")
}

/// Entfernt Partitions-Suffix: `sdb1` → `sdb`, `nvme0n1p2` → `nvme0n1`.
///
/// NVMe/MMC-Geräte (nvme*, mmcblk*) verwenden `p<N>` als Suffix.
/// SATA/USB-Geräte (sd*, vd*, hd*) verwenden direkt angehängte Ziffern.
fn strip_partition_suffix(name: &str) -> &str {
    // NVMe-Stil: nvme0n1p2, mmcblk0p1 → Suffix `p[0-9]+` entfernen
    let uses_p_suffix = name.starts_with("nvme")
        || name.starts_with("mmcblk")
        || name.starts_with("loop");
    if uses_p_suffix {
        if let Some(pos) = name.rfind('p') {
            let suffix = &name[pos + 1..];
            let pre_p  = name[..pos].chars().last();
            // Gültiges Partitions-Suffix: vorheriges Zeichen ist Ziffer, Suffix nur Ziffern
            // Beispiel: nvme0n1[p]1  loop0[p]1 — aber NICHT: loo[p]0
            if !suffix.is_empty()
                && suffix.chars().all(|c| c.is_ascii_digit())
                && pre_p.is_some_and(|c| c.is_ascii_digit())
            {
                return &name[..pos];
            }
        }
        return name;  // kein Partitions-Suffix → Device-Name unverändert
    }

    // SATA/USB/Virtio: sda1 → sda, sdb → sdb, vda2 → vda
    // Nur strippen wenn verbleibender Teil ausschließlich aus Buchstaben besteht.
    let base = name.trim_end_matches(|c: char| c.is_ascii_digit());
    if !base.is_empty() && base.chars().all(|c| c.is_ascii_alphabetic()) {
        return base;
    }
    name
}

// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::strip_partition_suffix;

    #[test]
    fn sata_partition_stripped() {
        assert_eq!(strip_partition_suffix("sdb1"), "sdb");
        assert_eq!(strip_partition_suffix("sda"),  "sda");
        assert_eq!(strip_partition_suffix("sdc3"), "sdc");
        assert_eq!(strip_partition_suffix("vda2"), "vda");
        assert_eq!(strip_partition_suffix("hda"),  "hda");
    }

    #[test]
    fn nvme_partition_stripped() {
        assert_eq!(strip_partition_suffix("nvme0n1p1"), "nvme0n1");
        assert_eq!(strip_partition_suffix("nvme0n1p2"), "nvme0n1");
        assert_eq!(strip_partition_suffix("nvme0n1"),   "nvme0n1");
        assert_eq!(strip_partition_suffix("nvme1n2p3"), "nvme1n2");
    }

    #[test]
    fn mmc_partition_stripped() {
        assert_eq!(strip_partition_suffix("mmcblk0p1"), "mmcblk0");
        assert_eq!(strip_partition_suffix("mmcblk0"),   "mmcblk0");
    }

    #[test]
    fn loop_device_unchanged() {
        // loop0 hat kein Partitions-Suffix für unsere Zwecke
        assert_eq!(strip_partition_suffix("loop0"), "loop0");
        assert_eq!(strip_partition_suffix("loop0p1"), "loop0");
    }
}
