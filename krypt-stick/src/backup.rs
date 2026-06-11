// backup.rs — Backup-Stick hinzufügen und Slot promoten
//
// add():     Identischer Flow wie create::run_setup — neuer Slot, anderer Stick.
// promote(): Markiert einen Slot als "primär" in der Ausgabe.
//            LUKS2-Token-Tagging (tpm2-tools) ist Phase 6+.

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;

pub fn add(luks_dev: &str, stick_dev: &str) -> crate::luks::Result<()> {
    if !std::path::Path::new(stick_dev).exists() {
        return Err(format!("Stick device not found: {stick_dev}").into());
    }

    println!("Adding backup stick {stick_dev} → LUKS device {luks_dev}");

    let mut key = vec![0u8; 64];
    File::open("/dev/urandom")?.read_exact(&mut key)?;

    {
        let mut stick = OpenOptions::new().write(true).open(stick_dev)?;
        stick.seek(SeekFrom::Start(512))?;
        stick.write_all(&key)?;
        stick.flush()?;
        // fsync vor luksAddKey — sonst lebt der Key nur im Cache, und ein
        // Stromausfall zwischen luksAddKey und nächstem sync bedeutet:
        // LUKS-Slot existiert, Stick hat keinen Key → Backup tot.
        // Gleicher Schutz wie in create::run_setup.
        if unsafe { libc::fsync(stick.as_raw_fd()) } != 0 {
            let err = io::Error::last_os_error();
            return Err(format!("fsync auf {stick_dev} fehlgeschlagen: {err}").into());
        }
    }

    let tmp_path = "/tmp/.krypt-backup-key";
    {
        let mut tmp = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(tmp_path)?;
        tmp.write_all(&key)?;
        tmp.flush()?;
    }

    let slot_result = crate::luks::add_key_from_file(luks_dev, std::path::Path::new(tmp_path));
    let _ = std::fs::remove_file(tmp_path);
    let slot = slot_result?;

    println!("\nBackup stick added — LUKS slot {slot}");
    println!("Add to /etc/krypt/daemon.toml:");
    println!("  [[auth_sticks]]");
    println!("  serial = \"<udevadm info --query=property --name={stick_dev} | grep ID_SERIAL_SHORT>\"");
    println!("  luks_slot = {slot}");

    Ok(())
}

/// Notiert einen Slot als primär (LUKS2-Token-Support in Phase 6+).
pub fn promote(luks_dev: &str, slot: u32) -> crate::luks::Result<()> {
    // Validieren: Device erreichbar + Slot-Übersicht zeigen
    println!("Current key slots on {luks_dev}:");
    crate::luks::list_slots(luks_dev)?;
    println!("\nSlot {slot} noted as primary stick.");
    println!("LUKS2 token tagging (for initramfs priority) is Phase 6+.");
    println!("Update /etc/krypt/daemon.toml if the primary slot changed.");
    Ok(())
}
