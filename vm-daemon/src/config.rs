// config.rs — TOML-Konfiguration für krypt-daemon
//
// Lädt /etc/krypt/daemon.toml. Fehlt die Datei → sicherer Default.
//
// Beispiel-Format:
//   [daemon]
//   log_level = "info"
//   panic_level = "suspend"    # lock | suspend | nuke
//
//   [[auth_sticks]]
//   serial = "ABC1234567"
//   luks_slot = 0
//
//   [[vms]]
//   name = "sys-gui"
//   memory_mb = 2048
//   vcpus = 2
//   kernel = "/boot/vmlinuz-lts"
//   trust_level = "green"
//
//   [[policy]]
//   source = "browser"
//   target = "vault"
//   action = "deny"

use serde::Deserialize;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("i/o error reading config: {0}")]
    Io(#[from] std::io::Error),
    #[error("toml parse error: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("invalid VM name '{0}' — must match [a-z0-9_-]{{1,32}}")]
    InvalidVmName(String),
    #[error("invalid stick serial '{0}' — must be printable ASCII, 1..=64 chars")]
    InvalidStickSerial(String),
    #[error("policy references unknown VM '{0}' — not defined in [[vms]]")]
    UnknownPolicyVm(String),
    #[error("duplicate VM name '{0}' — last [[vms]] block silently won before this check")]
    DuplicateVmName(String),
    #[error("duplicate auth_sticks serial '{0}' — last [[auth_sticks]] block silently won before this check")]
    DuplicateStickSerial(String),
    #[error("path field '{field}' for VM '{vm}' contains forbidden character (quote or newline): {value:?}")]
    InvalidPath { vm: String, field: &'static str, value: String },
}

/// Vollständige Daemon-Konfiguration.
#[derive(Debug, Deserialize, Default)]
pub struct KryptConfig {
    #[serde(default)]
    pub daemon: DaemonConfig,
    #[serde(default)]
    pub auth_sticks: Vec<AuthStickEntry>,
    #[serde(default)]
    pub vms: Vec<VmEntry>,
    #[serde(default)]
    pub policy: Vec<PolicyEntry>,
}

#[derive(Debug, Deserialize)]
pub struct DaemonConfig {
    #[serde(default = "default_log_level")]
    pub log_level: String,
    #[serde(default)]
    pub panic_level: PanicLevel,
}

fn default_log_level() -> String {
    "info".to_string()
}

/// Reaktion wenn der Auth-Stick abgezogen wird.
#[derive(Debug, Deserialize, Default, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum PanicLevel {
    /// Desktop sperren (loginctl lock-sessions)
    Lock,
    /// System suspenden (systemctl suspend)
    #[default]
    Suspend,
    /// Sofort herunterfahren
    Nuke,
}

/// Registrierter Auth-Stick.
#[derive(Debug, Deserialize, Clone)]
pub struct AuthStickEntry {
    pub serial: String,
    /// LUKS2-Key-Slot-Nummer für diesen Stick
    pub luks_slot: u32,
}

/// VM-Definition aus der TOML-Konfiguration.
#[derive(Debug, Deserialize, Clone)]
pub struct VmEntry {
    pub name: String,
    pub memory_mb: u32,
    pub vcpus: u32,
    pub kernel: String,
    pub ramdisk: Option<PathBuf>,
    pub root_disk: Option<PathBuf>,
    #[serde(default)]
    pub trust_level: TrustLevel,
}

#[derive(Debug, Deserialize, Default, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TrustLevel {
    #[default]
    Red,
    Orange,
    Yellow,
    Green,
    Black,
}

/// Explizite Policy-Regel zwischen zwei VMs.
#[derive(Debug, Deserialize, Clone)]
pub struct PolicyEntry {
    pub source: String,
    pub target: String,
    pub action: PolicyAction,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PolicyAction {
    Allow,
    Deny,
    Ask,
}

impl KryptConfig {
    /// Lädt die Konfiguration aus einer TOML-Datei.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)?;
        let cfg: Self = toml::from_str(&content)?;
        cfg.validate()?;
        Ok(cfg)
    }

    /// Prüft VM-Namen und Stick-Serials. Tippfehler im daemon.toml fallen
    /// so beim Start auf statt erst bei `xl create` oder USB-Match-Failure.
    /// Zusätzlich: policy-Regeln müssen auf in [[vms]] definierte Namen zeigen —
    /// sonst silent-no-op (matching scheitert, Fallback auf Trust-Level greift).
    pub fn validate(&self) -> Result<(), ConfigError> {
        let mut seen = std::collections::HashSet::with_capacity(self.vms.len());
        for vm in &self.vms {
            if !is_valid_vm_name(&vm.name) {
                return Err(ConfigError::InvalidVmName(vm.name.clone()));
            }
            if !seen.insert(vm.name.as_str()) {
                return Err(ConfigError::DuplicateVmName(vm.name.clone()));
            }
            // Pfade in kernel/ramdisk/root_disk gehen unescaped in die
            // generierte xl-Config (siehe vm::write_xl_cfg). xl akzeptiert
            // keine eingebetteten Quotes/Newlines — vorher würde ein gefakter
            // daemon.toml-Eintrag wie kernel = "x\"" hier silently passieren
            // und beim ersten VM-Start in xl-Parse-Fehlern enden.
            check_path(&vm.name, "kernel", &vm.kernel)?;
            if let Some(p) = vm.ramdisk.as_ref().and_then(|p| p.to_str()) {
                check_path(&vm.name, "ramdisk", p)?;
            }
            if let Some(p) = vm.root_disk.as_ref().and_then(|p| p.to_str()) {
                check_path(&vm.name, "root_disk", p)?;
            }
        }
        let mut seen_sticks = std::collections::HashSet::with_capacity(self.auth_sticks.len());
        for stick in &self.auth_sticks {
            if !is_valid_serial(&stick.serial) {
                return Err(ConfigError::InvalidStickSerial(stick.serial.clone()));
            }
            if !seen_sticks.insert(stick.serial.as_str()) {
                return Err(ConfigError::DuplicateStickSerial(stick.serial.clone()));
            }
        }
        let known: std::collections::HashSet<&str> =
            self.vms.iter().map(|v| v.name.as_str()).collect();
        for rule in &self.policy {
            if !is_valid_vm_name(&rule.source) {
                return Err(ConfigError::InvalidVmName(rule.source.clone()));
            }
            if !is_valid_vm_name(&rule.target) {
                return Err(ConfigError::InvalidVmName(rule.target.clone()));
            }
            if !known.contains(rule.source.as_str()) {
                return Err(ConfigError::UnknownPolicyVm(rule.source.clone()));
            }
            if !known.contains(rule.target.as_str()) {
                return Err(ConfigError::UnknownPolicyVm(rule.target.clone()));
            }
        }
        Ok(())
    }
}

/// Selbe Regel wie vm::is_valid_vm_name — dupliziert weil config.rs sonst
/// auf vm.rs angewiesen wäre, was zyklisch ist (vm.rs nutzt config.rs Typen).
fn is_valid_vm_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 32
        && name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

/// USB-Stick-Serien: drucker-ASCII, keine Quotes/Whitespace. Manche Sticks
/// haben Leerzeichen in der Serial — wir akzeptieren sie nicht, lieber soll
/// der User per `udevadm` die ID_SERIAL_SHORT (ohne Spaces) nehmen.
fn is_valid_serial(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.bytes().all(|b| b.is_ascii_graphic())
}

/// Lehnt eingebettete `"` und `\n` ab — würden sonst die xl-Config-Generation
/// in vm::write_xl_cfg zerschießen. Empty Strings sind ok (kein VM-Pfad gesetzt).
fn check_path(vm: &str, field: &'static str, value: &str) -> Result<(), ConfigError> {
    if value.contains('"') || value.contains('\n') {
        return Err(ConfigError::InvalidPath {
            vm: vm.to_owned(),
            field,
            value: value.to_owned(),
        });
    }
    Ok(())
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            log_level: default_log_level(),
            panic_level: PanicLevel::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vm(name: &str) -> VmEntry {
        VmEntry {
            name: name.into(),
            memory_mb: 1024,
            vcpus: 1,
            kernel: "/boot/vmlinuz".into(),
            ramdisk: None,
            root_disk: None,
            trust_level: TrustLevel::Red,
        }
    }

    fn pol(src: &str, tgt: &str) -> PolicyEntry {
        PolicyEntry {
            source: src.into(),
            target: tgt.into(),
            action: PolicyAction::Deny,
        }
    }

    #[test]
    fn validate_accepts_policy_referring_to_known_vms() {
        let cfg = KryptConfig {
            vms: vec![vm("work"), vm("vault")],
            policy: vec![pol("work", "vault")],
            ..Default::default()
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn validate_rejects_policy_with_unknown_source() {
        let cfg = KryptConfig {
            vms: vec![vm("work"), vm("vault")],
            policy: vec![pol("wrk", "vault")], // Tippfehler
            ..Default::default()
        };
        match cfg.validate() {
            Err(ConfigError::UnknownPolicyVm(n)) => assert_eq!(n, "wrk"),
            other => panic!("expected UnknownPolicyVm, got {:?}", other),
        }
    }

    #[test]
    fn validate_rejects_policy_with_unknown_target() {
        let cfg = KryptConfig {
            vms: vec![vm("work")],
            policy: vec![pol("work", "vaullt")],
            ..Default::default()
        };
        match cfg.validate() {
            Err(ConfigError::UnknownPolicyVm(n)) => assert_eq!(n, "vaullt"),
            other => panic!("expected UnknownPolicyVm, got {:?}", other),
        }
    }

    #[test]
    fn validate_rejects_duplicate_vm_names() {
        // Zwei [[vms]]-Blöcke mit identischem name → vorher hat VmManager::register
        // den ersten still überschrieben (HashMap::insert), und der User wunderte
        // sich warum seine ersten Settings (memory, trust_level) ignoriert wurden.
        let cfg = KryptConfig {
            vms: vec![vm("work"), vm("work")],
            ..Default::default()
        };
        match cfg.validate() {
            Err(ConfigError::DuplicateVmName(n)) => assert_eq!(n, "work"),
            other => panic!("expected DuplicateVmName, got {:?}", other),
        }
    }

    #[test]
    fn validate_rejects_kernel_with_quote() {
        let mut bad = vm("work");
        bad.kernel = "/boot/vmlinuz\"".into();
        let cfg = KryptConfig { vms: vec![bad], ..Default::default() };
        match cfg.validate() {
            Err(ConfigError::InvalidPath { vm: v, field, .. }) => {
                assert_eq!(v, "work");
                assert_eq!(field, "kernel");
            }
            other => panic!("expected InvalidPath, got {:?}", other),
        }
    }

    #[test]
    fn validate_rejects_root_disk_with_newline() {
        let mut bad = vm("work");
        bad.root_disk = Some("/dev/vg0/work\nbad".into());
        let cfg = KryptConfig { vms: vec![bad], ..Default::default() };
        match cfg.validate() {
            Err(ConfigError::InvalidPath { vm: v, field, .. }) => {
                assert_eq!(v, "work");
                assert_eq!(field, "root_disk");
            }
            other => panic!("expected InvalidPath, got {:?}", other),
        }
    }

    #[test]
    fn validate_rejects_duplicate_stick_serials() {
        let cfg = KryptConfig {
            auth_sticks: vec![
                AuthStickEntry { serial: "AB12".into(), luks_slot: 1 },
                AuthStickEntry { serial: "AB12".into(), luks_slot: 2 },
            ],
            ..Default::default()
        };
        match cfg.validate() {
            Err(ConfigError::DuplicateStickSerial(s)) => assert_eq!(s, "AB12"),
            other => panic!("expected DuplicateStickSerial, got {:?}", other),
        }
    }
}
