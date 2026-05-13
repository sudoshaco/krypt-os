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
        Ok(toml::from_str(&content)?)
    }
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            log_level: default_log_level(),
            panic_level: PanicLevel::default(),
        }
    }
}
