// revoke.rs — Stick-Slot widerrufen via cryptsetup luksKillSlot
//
// WARNUNG: Irreversibel. Verbleibende Slots prüfen bevor du fortfährst.

pub fn slot(luks_dev: &str, n: u32) -> crate::luks::Result<()> {
    println!("Revoking slot {n} on {luks_dev}");

    // Vorher: Header "Remaining slots after revoke:" stand DIREKT VOR der
    // Liste der aktuell aktiven Slots — also vor dem Kill. Der User sah
    // also "Remaining: 0, 1, 2" obwohl Slot {n} noch drin war, und nach
    // dem Kill wurde nichts neu gelistet. Verwirrung garantiert.
    println!("Active slots before revoke:");
    crate::luks::list_slots(luks_dev)?;
    println!();

    // Slot entfernen (cryptsetup fragt interaktiv nach einer anderen gültigen Passphrase)
    crate::luks::kill_slot(luks_dev, n)?;

    println!();
    println!("Active slots after revoke:");
    crate::luks::list_slots(luks_dev)?;
    println!();

    println!("Done — remove the auth_sticks entry for slot {n} from /etc/krypt/daemon.toml");
    Ok(())
}
