// revoke.rs — Stick-Slot widerrufen via cryptsetup luksKillSlot
//
// WARNUNG: Irreversibel. Verbleibende Slots prüfen bevor du fortfährst.

pub fn slot(luks_dev: &str, n: u32) -> crate::luks::Result<()> {
    println!("Revoking slot {n} on {luks_dev}");
    println!("Remaining slots after revoke:");

    // Aktuelle Slots zur Info anzeigen
    crate::luks::list_slots(luks_dev)?;
    println!();

    // Slot entfernen (cryptsetup fragt interaktiv nach einer anderen gültigen Passphrase)
    crate::luks::kill_slot(luks_dev, n)?;

    println!("Done — remove the auth_sticks entry for slot {n} from /etc/krypt/daemon.toml");
    Ok(())
}
