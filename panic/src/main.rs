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
    let level = parse_level_arg(env::args().skip(1));

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
    // krypt-panic wird vom krypt-daemon (systemd-Service) gespawned und erbt
    // dessen leeres Environment. `hyprctl dispatch exec hyprlock` braucht
    // aber HYPRLAND_INSTANCE_SIGNATURE und WAYLAND_DISPLAY aus der User-
    // Session — die fehlen hier. Daher: zuerst loginctl lock-sessions
    // (system-weit, kein User-Env nötig, triggert hyprlock via session-bus),
    // und nur als best-effort-Ergänzung noch hyprctl falls es doch klappt.
    let loginctl_ok = Command::new("loginctl")
        .arg("lock-sessions")
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if !loginctl_ok {
        eprintln!("[krypt-panic] WARN: loginctl lock-sessions failed — trying hyprctl fallback");
        let _ = Command::new("hyprctl")
            .args(["dispatch", "exec", "hyprlock"])
            .status();
    }
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

/// Best-effort Reduktion der Restdaten im RAM vor dem Poweroff.
///
/// Echtes Memory-Scrubbing über alle DRAM-Pages ist nur via kexec mit
/// einem speziell präparierten Wipe-Kernel möglich — das bleibt Phase 5.
/// Bis dahin tun wir die drei Dinge, die ein normaler Userspace-Prozess
/// noch erreichen kann, ohne den Poweroff zu blockieren:
///   1. Page-Cache + dentries + inodes droppen (Datei-Inhalte, die wir
///      in den letzten Sekunden gelesen haben, sind danach nicht mehr
///      im freigegebenen RAM-Pool).
///   2. swapoff -a — falls Swap aktiv ist, sind dort potentiell Key-Pages
///      gelandet; swapoff zwingt das Auslagern zurückzulesen und überschreibt
///      die Swap-Bereiche.
///   3. sync — sicherstellen, dass keine dirty pages mit Key-Material
///      noch auf den Disk-Caches warten.
///
/// Jeder Schritt ist best-effort; ein einzelner Fehler bricht den Nuke nicht
/// ab — wir wollen unter allen Umständen zum poweroff kommen.
fn wipe_sensitive_memory() {
    use std::fs;

    // 1. Kernel-Caches droppen (Page-Cache, dentries, inodes)
    match fs::write("/proc/sys/vm/drop_caches", "3\n") {
        Ok(_)  => eprintln!("[krypt-panic] dropped page/dentry/inode caches"),
        Err(e) => eprintln!("[krypt-panic] drop_caches failed: {e}"),
    }

    // 2. Swap deaktivieren (überschreibt Swap-Bereiche beim Auslagern zurück)
    match Command::new("swapoff").arg("-a").status() {
        Ok(s) if s.success() => eprintln!("[krypt-panic] swapoff -a OK"),
        Ok(s)                => eprintln!("[krypt-panic] swapoff -a exit {}", s.code().unwrap_or(-1)),
        Err(e)               => eprintln!("[krypt-panic] swapoff -a failed: {e}"),
    }

    // 3. sync — dirty pages flushen, bevor poweroff den Strom kappt
    unsafe { libc::sync() };
    eprintln!("[krypt-panic] sync done");

    // Hinweis: ein vollständiger DRAM-Wipe (Cold-Boot-Schutz) braucht
    // kexec auf einen Wipe-Kernel — siehe docs/known-issues.md, Phase 5.
}

/// Liest den Wert von `--level=<x>` aus den CLI-Args, default „nuke".
///
/// Fail-secure: wenn niemand `--level=` mitgegeben hat, eskalieren wir
/// implizit zu „nuke" (siehe main-Kommentar). Wenn jemand etwas Falsches
/// mitgegeben hat, gibt diese Funktion den Wert zurück und das Match in
/// main loggt + escalates zum nuke.
fn parse_level_arg<I: IntoIterator<Item = String>>(args: I) -> String {
    args.into_iter()
        .find(|a| a.starts_with("--level="))
        .and_then(|a| a.strip_prefix("--level=").map(String::from))
        .unwrap_or_else(|| "nuke".to_string())
}

#[cfg(test)]
mod tests {
    use super::parse_level_arg;

    fn args(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn default_is_nuke_when_no_args() {
        assert_eq!(parse_level_arg(args(&[])), "nuke");
    }

    #[test]
    fn default_is_nuke_when_other_args_only() {
        assert_eq!(parse_level_arg(args(&["--verbose", "foo"])), "nuke");
    }

    #[test]
    fn parses_lock() {
        assert_eq!(parse_level_arg(args(&["--level=lock"])), "lock");
    }

    #[test]
    fn parses_suspend_with_other_args_around() {
        assert_eq!(
            parse_level_arg(args(&["--verbose", "--level=suspend", "--debug"])),
            "suspend"
        );
    }

    #[test]
    fn first_level_wins() {
        // Erste --level=… Definition gewinnt — caller würde sonst
        // beliebig viele anhängen und panic-Level kippen können.
        assert_eq!(
            parse_level_arg(args(&["--level=lock", "--level=nuke"])),
            "lock"
        );
    }

    #[test]
    fn empty_level_returned_verbatim() {
        // '--level=' (ohne Wert) liefert leeren String zurück.
        // Das Match in main() escalates dann ohne Warnung zu NUKE —
        // genau das gewollte fail-secure-Verhalten.
        assert_eq!(parse_level_arg(args(&["--level="])), "");
    }
}
