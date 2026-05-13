// luks.rs — LUKS2 Slot-Verwaltung via cryptsetup subprocess
//
// Alle Operationen rufen `cryptsetup` als Child-Prozess auf und
// setzen Root-Rechte voraus (Caller: main.rs prüft via nix::unistd::Uid).
//
// Key-Layout auf dem Auth-Stick (raw device, kein Filesystem nötig):
//   Offset   0 –  511: MBR / Partition-Table (unangetastet)
//   Offset 512 –  575: 64-Byte LUKS-Key (von krypt-stick geschrieben)

use std::collections::HashSet;
use std::path::Path;
use std::process::Command;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Zeigt aktive Key-Slots eines LUKS2-Devices auf stdout.
pub fn list_slots(luks_dev: &str) -> Result<()> {
    let out = Command::new("cryptsetup")
        .args(["luksDump", luks_dev])
        .output()?;

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(format!("cryptsetup luksDump: {}", stderr.trim()).into());
    }

    let dump = String::from_utf8_lossy(&out.stdout);
    let active = active_slots_from_dump(&dump);

    println!("LUKS2 device: {luks_dev}");
    if active.is_empty() {
        println!("  (no active key slots)");
    } else {
        let mut sorted: Vec<u32> = active.into_iter().collect();
        sorted.sort_unstable();
        for slot in sorted {
            println!("  Slot {slot}: ENABLED");
        }
    }
    Ok(())
}

/// Fügt einen neuen 64-Byte-Key aus einer Datei zu einem freien Slot hinzu.
///
/// cryptsetup fragt interaktiv nach dem bestehenden Passwort (oder Key-File via
/// `--key-file` wenn der Caller das Device bereits per Key entsperrt hat).
/// Gibt die neu belegte Slot-Nummer zurück.
pub fn add_key_from_file(luks_dev: &str, new_key_file: &Path) -> Result<u32> {
    let slot = next_free_slot(luks_dev)?;
    let slot_str = slot.to_string();

    // cryptsetup luksAddKey --key-slot <n> <device> <new-keyfile>
    // Bestehende Passphrase wird via Terminal abgefragt.
    let status = Command::new("cryptsetup")
        .args([
            "luksAddKey",
            "--key-slot", &slot_str,
            luks_dev,
            &new_key_file.to_string_lossy(),
        ])
        .status()?;

    if !status.success() {
        return Err(format!("cryptsetup luksAddKey failed (slot {slot})").into());
    }

    println!("Key added to slot {slot} on {luks_dev}");
    Ok(slot)
}

/// Entfernt einen Key-Slot via `cryptsetup luksKillSlot`.
///
/// Cryptsetup fragt zur Bestätigung nach einer anderen gültigen Passphrase.
/// WARNUNG: Wenn kein anderer Slot existiert, ist das Device danach verschlossen.
pub fn kill_slot(luks_dev: &str, slot: u32) -> Result<()> {
    let slot_str = slot.to_string();
    let status = Command::new("cryptsetup")
        .args(["luksKillSlot", luks_dev, &slot_str])
        .status()?;

    if !status.success() {
        return Err(format!("cryptsetup luksKillSlot failed for slot {slot}").into());
    }

    println!("Slot {slot} removed from {luks_dev}");
    Ok(())
}

/// Gibt die erste freie Slot-Nummer zurück (0–31).
pub fn next_free_slot(luks_dev: &str) -> Result<u32> {
    let out = Command::new("cryptsetup")
        .args(["luksDump", luks_dev])
        .output()?;

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(format!("cryptsetup luksDump: {}", stderr.trim()).into());
    }

    let dump = String::from_utf8_lossy(&out.stdout);
    let used = active_slots_from_dump(&dump);

    (0..32u32)
        .find(|s| !used.contains(s))
        .ok_or_else(|| "all LUKS2 key slots occupied (max 32)".into())
}

/// Parst aktive Slot-Nummern aus `cryptsetup luksDump` output.
///
/// Unterstützt LUKS2 ("Keyslots:"-Sektion) und LUKS1 ("Key Slot N: ENABLED").
pub(crate) fn active_slots_from_dump(dump: &str) -> HashSet<u32> {
    let mut slots = HashSet::new();
    let mut in_keyslots_section = false;

    for line in dump.lines() {
        let trimmed = line.trim();

        // LUKS2: Sektion "Keyslots:" beginnt
        if trimmed == "Keyslots:" {
            in_keyslots_section = true;
            continue;
        }
        // Sektion endet bei nicht-eingerückter Zeile (weder Space noch Tab)
        if in_keyslots_section
            && !line.starts_with(' ')
            && !line.starts_with('\t')
            && !trimmed.is_empty()
        {
            in_keyslots_section = false;
        }
        // LUKS2 format: "  0: luks2"
        if in_keyslots_section {
            if let Some(colon) = trimmed.find(':') {
                if let Ok(n) = trimmed[..colon].trim().parse::<u32>() {
                    slots.insert(n);
                }
            }
        }

        // LUKS1 fallback: "Key Slot 0: ENABLED"
        if let Some(rest) = trimmed.strip_prefix("Key Slot ") {
            if rest.contains("ENABLED") {
                if let Some(colon) = rest.find(':') {
                    if let Ok(n) = rest[..colon].parse::<u32>() {
                        slots.insert(n);
                    }
                }
            }
        }
    }

    slots
}

// ---------------------------------------------------------------------------
// Tests — keine echte Hardware nötig: reine String-Parsing-Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::active_slots_from_dump;

    // LUKS2-Dump mit Slots 0, 1, 3 aktiv (Slot 2 fehlt in der Keyslots-Sektion)
    const LUKS2_DUMP: &str = r#"LUKS header information
Version:       	2
Epoch:         	5
UUID:          	a1b2c3d4-e5f6-7890-abcd-ef1234567890
Flags:       	(no flags)

Data segments:
  0: crypt
	offset: 16777216 [bytes]
	cipher: aes-xts-plain64

Keyslots:
  0: luks2
	Key:        512 bits
	Priority:   normal
	Cipher:     aes-xts-plain64
	PBKDF:      argon2id
  1: luks2
	Key:        512 bits
	Priority:   normal
  3: luks2
	Key:        512 bits
	Priority:   normal
Tokens:
Digests:
  0: pbkdf2
	Hash:       sha256
"#;

    // LUKS1-Dump: Slots 0 und 2 aktiv, Rest DISABLED
    const LUKS1_DUMP: &str = r#"LUKS header information for /dev/sda2

Version:       	1
Cipher name:   	aes
Cipher mode:   	xts-plain64
UUID:          	a1b2c3d4-1234-5678-abcd-ef0987654321

Key Slot 0: ENABLED
	Iterations:         	819200
	AF stripes:            4000
Key Slot 1: DISABLED
Key Slot 2: ENABLED
	Iterations:         	819200
Key Slot 3: DISABLED
Key Slot 4: DISABLED
Key Slot 5: DISABLED
Key Slot 6: DISABLED
Key Slot 7: DISABLED
"#;

    // Leeres Dump (frisches LUKS-Device, noch kein Slot gesetzt)
    const EMPTY_DUMP: &str = r#"LUKS header information
Version:       	2
Keyslots:
Tokens:
Digests:
"#;

    // Alle 32 LUKS2-Slots belegt (für next_free_slot-Grenzfall)
    fn full_luks2_dump() -> String {
        let mut s = String::from("LUKS header information\nVersion:\t2\n\nKeyslots:\n");
        for i in 0..32u32 {
            s.push_str(&format!("  {i}: luks2\n\tKey: 512 bits\n"));
        }
        s.push_str("Tokens:\n");
        s
    }

    #[test]
    fn luks2_parses_three_active_slots() {
        let slots = active_slots_from_dump(LUKS2_DUMP);
        assert_eq!(slots.len(), 3);
        assert!(slots.contains(&0));
        assert!(slots.contains(&1));
        assert!(slots.contains(&3));
        assert!(!slots.contains(&2));
    }

    #[test]
    fn luks1_parses_enabled_slots() {
        let slots = active_slots_from_dump(LUKS1_DUMP);
        assert_eq!(slots.len(), 2);
        assert!(slots.contains(&0));
        assert!(slots.contains(&2));
        assert!(!slots.contains(&1));
        assert!(!slots.contains(&7));
    }

    #[test]
    fn empty_dump_returns_no_slots() {
        let slots = active_slots_from_dump(EMPTY_DUMP);
        assert!(slots.is_empty());
    }

    #[test]
    fn all_32_slots_occupied() {
        let dump = full_luks2_dump();
        let slots = active_slots_from_dump(&dump);
        assert_eq!(slots.len(), 32);
        // Kein freier Slot — (0..32).find(|s| !slots.contains(s)) gibt None
        let free = (0..32u32).find(|s| !slots.contains(s));
        assert!(free.is_none());
    }

    #[test]
    fn next_free_slot_skips_used() {
        // Slots 0 und 1 belegt → nächster freier ist 2
        let dump = "LUKS header information\nVersion:\t2\n\nKeyslots:\n  0: luks2\n  1: luks2\nTokens:\n";
        let slots = active_slots_from_dump(dump);
        let next = (0..32u32).find(|s| !slots.contains(s));
        assert_eq!(next, Some(2));
    }

    #[test]
    fn luks2_section_ends_at_non_indented_line() {
        // "Tokens:" ist nicht eingerückt → Keyslots-Sektion endet dort
        // Zeilen danach dürfen nicht als Slots geparst werden
        let dump = "Version:\t2\n\nKeyslots:\n  0: luks2\nTokens:\n  0: some_token\nDigests:\n";
        let slots = active_slots_from_dump(dump);
        // Nur Slot 0 aus der Keyslots-Sektion — "0: some_token" unter Tokens zählt nicht
        assert_eq!(slots.len(), 1);
        assert!(slots.contains(&0));
    }

    #[test]
    fn ignores_luks1_disabled_slots() {
        let dump = "Key Slot 0: DISABLED\nKey Slot 1: ENABLED\nKey Slot 2: DISABLED\n";
        let slots = active_slots_from_dump(dump);
        assert_eq!(slots.len(), 1);
        assert!(slots.contains(&1));
    }

    #[test]
    fn handles_mixed_luks2_and_luks1_format_gracefully() {
        // Beide Parser-Pfade im selben Dump — LUKS2 Keyslots-Sektion hat Prio
        let dump = "Keyslots:\n  2: luks2\nKey Slot 5: ENABLED\n";
        let slots = active_slots_from_dump(dump);
        // Slot 2 aus LUKS2-Sektion, Slot 5 aus LUKS1-Fallback (außerhalb Sektion)
        assert!(slots.contains(&2));
        assert!(slots.contains(&5));
    }
}
