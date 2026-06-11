// krypt-panic — Emergency Shutdown Handler
//
// Wird aufgerufen wenn der Auth-Stick entfernt wird.
// Läuft mit minimalen Dependencies damit es IMMER funktioniert.
// Kein tokio, kein async — direkter syscall-naher Code.
//
// Panic-Level:
//   lock    → Bildschirm sperren, VMs suspenden
//   suspend → VMs einfrieren, System suspenden
//   nuke    → VMs killen, RAM wipen, shutdown

use std::env;
use std::process::Command;
use std::fs;

fn main() {
    let level = env::args()
        .find(|a| a.starts_with("--level="))
        .and_then(|a| a.strip_prefix("--level=").map(String::from))
        .unwrap_or_else(|| "nuke".to_string());

    eprintln!("[krypt-panic] TRIGGERED — level: {}", level);

    match level.as_str() {
        "lock"    => panic_lock(),
        "suspend" => panic_suspend(),
        "nuke"    => panic_nuke(),
        other => {
            // Bewusst fail-secure: Unbekannte Level (Tippfehler, leerer
            // Wert, böswillige Args) sollen den Auth-Stick-Schutz NIE
            // schwächer machen. Aber wir loggen die Eskalation laut, damit
            // im Journal nachvollziehbar ist warum es zum nuke kam — ohne
            // den Hinweis war eine "--level=fooled-the-config" Eingabe vom
            // tatsächlich gewollten "--level=nuke" nicht unterscheidbar.
            eprintln!(
                "[krypt-panic] WARN: unknown level '{other}' — escalating to NUKE \
                 (valid levels: lock | suspend | nuke). Check the caller."
            );
            panic_nuke();
        }
    }
}

fn panic_lock() {
    // Hyprland sperren
    let _ = Command::new("hyprctl").args(["dispatch", "exec", "hyprlock"]).status();
    eprintln!("[krypt-panic] LOCKED");
}

fn panic_suspend() {
    // Alle AppVMs einfrieren
    freeze_all_vms();
    // System suspenden — "mem" = Suspend-to-RAM (S3).
    // "disk" wäre Hibernate (S4): schreibt RAM auf Swap, wo der LUKS-Key
    // landen würde — genau das Gegenteil von dem was Panic-Suspend will.
    let _ = fs::write("/sys/power/state", "mem");
    eprintln!("[krypt-panic] SUSPENDED");
}

fn panic_nuke() {
    // Reihenfolge ist kritisch — kein Schritt überspringen

    // 1. Alle AppVMs sofort killen (kein graceful shutdown)
    kill_all_vms();

    // 2. In-Memory Keys überschreiben (soweit möglich)
    wipe_sensitive_memory();

    // 3. Sofort herunterfahren
    eprintln!("[krypt-panic] NUKE — shutting down NOW");
    let _ = Command::new("systemctl").arg("poweroff").arg("--force").status();

    // Fallback wenn systemctl fehlschlägt
    unsafe {
        libc::sync();
        libc::reboot(libc::LINUX_REBOOT_CMD_POWER_OFF);
    }
}

fn freeze_all_vms() {
    // xl pause für alle laufenden Domains außer dom0
    if let Ok(output) = Command::new("xl").arg("list").output() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 && parts[0] != "Domain-0" {
                let _ = Command::new("xl").args(["pause", parts[0]]).status();
            }
        }
    }
}

fn kill_all_vms() {
    // xl destroy für alle laufenden Domains außer dom0
    if let Ok(output) = Command::new("xl").arg("list").output() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 && parts[0] != "Domain-0" {
                let _ = Command::new("xl").args(["destroy", parts[0]]).status();
                eprintln!("[krypt-panic] killed VM: {}", parts[0]);
            }
        }
    }
}

fn wipe_sensitive_memory() {
    // /dev/mem schreiben soweit möglich (privilegiert)
    // In Produktion: spezifische Key-Material-Adressen überschreiben
    // Placeholder — vollständige Implementierung in Phase 5
    eprintln!("[krypt-panic] memory wipe: placeholder (Phase 5)");
}
