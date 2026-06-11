// vm.rs — VM Lifecycle Management
//
// Abstrahiert Xen-Domain-Operationen: create, start, shutdown, destroy.
// Nutzt `xl` CLI via tokio::process::Command — stabiler als libxl FFI-Bindings.
// xl-Config: wird entweder aus xl_cfg-Pfad geladen oder aus VmConfig generiert.
#![allow(dead_code)]

use std::collections::HashMap;
use std::path::PathBuf;
use thiserror::Error;
use tokio::process::Command;

#[derive(Debug, Error)]
pub enum VmError {
    #[error("VM '{0}' not found")]
    NotFound(String),
    #[error("VM '{0}' already running")]
    AlreadyRunning(String),
    #[error("VM '{0}' is not running")]
    NotRunning(String),
    #[error("xl command failed: {0}")]
    XlFailed(String),
    #[error("invalid VM name '{0}' — must match [a-z0-9_-]{{1,32}}")]
    InvalidName(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Erlaubt nur kleine Buchstaben, Ziffern, `-`, `_` und Länge 1..=32.
/// Schützt vor Path-Traversal in /run/krypt/krypt-<name>.cfg und vor
/// xl-Config-Parser-Fehlern (xl akzeptiert keine Quotes/Newlines).
pub fn is_valid_vm_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 32
        && name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VmState {
    Halted,
    Running,
    Paused,
    Crashed,
}

#[derive(Debug, Clone)]
pub struct VmConfig {
    pub name: String,
    pub memory_mb: u32,
    pub vcpus: u32,
    pub kernel: String,
    pub ramdisk: Option<String>,
    pub root_disk: Option<String>,
    /// Pfad zu einer existierenden xl .cfg-Datei. None → wird aus VmConfig generiert.
    pub xl_cfg: Option<PathBuf>,
}

#[derive(Debug)]
pub struct Vm {
    pub config: VmConfig,
    pub state: VmState,
    /// Xen Domain-ID, None wenn halted
    pub domain_id: Option<u32>,
}

impl Vm {
    pub fn new(config: VmConfig) -> Self {
        Self {
            config,
            state: VmState::Halted,
            domain_id: None,
        }
    }

    /// Schreibt eine minimale xl-Config nach /run/krypt/krypt-<name>.cfg.
    async fn write_xl_cfg(&self) -> Result<PathBuf, VmError> {
        // Path-Traversal-Schutz: name geht in den Dateinamen ein.
        // Auch wenn die Config heute aus trusted daemon.toml stammt, schützt
        // das vor Tippfehlern (z. B. "vm/test") die sonst /run/vm/test.cfg
        // erzeugen würden statt /run/krypt/krypt-vm/test.cfg.
        if !is_valid_vm_name(&self.config.name) {
            return Err(VmError::InvalidName(self.config.name.clone()));
        }
        let mut cfg = format!(
            "name = \"{}\"\nmemory = {}\nvcpus = {}\nkernel = \"{}\"\n",
            self.config.name, self.config.memory_mb, self.config.vcpus, self.config.kernel,
        );
        if let Some(rd) = &self.config.ramdisk {
            cfg.push_str(&format!("ramdisk = \"{rd}\"\n"));
        }
        if let Some(disk) = &self.config.root_disk {
            cfg.push_str(&format!("disk = [ \"{disk},raw,xvda,rw\" ]\n"));
        }
        let path = PathBuf::from(format!("/run/krypt/krypt-{}.cfg", self.config.name));
        tokio::fs::write(&path, cfg).await?;
        Ok(path)
    }

    /// Startet die VM via `xl create`.
    pub async fn start(&mut self) -> Result<(), VmError> {
        if self.state == VmState::Running {
            return Err(VmError::AlreadyRunning(self.config.name.clone()));
        }

        let cfg_path = match &self.config.xl_cfg {
            Some(p) => p.clone(),
            None => self.write_xl_cfg().await?,
        };

        let out = Command::new("xl")
            .args(["create", "-q", &cfg_path.to_string_lossy()])
            .output()
            .await?;

        if !out.status.success() {
            return Err(VmError::XlFailed(
                String::from_utf8_lossy(&out.stderr).trim().to_owned(),
            ));
        }

        // Domain-ID via `xl domid <name>` nachschlagen
        let domid_out = Command::new("xl")
            .args(["domid", &self.config.name])
            .output()
            .await?;

        if domid_out.status.success() {
            if let Ok(id) = String::from_utf8_lossy(&domid_out.stdout)
                .trim()
                .parse::<u32>()
            {
                self.domain_id = Some(id);
            }
        }

        self.state = VmState::Running;
        Ok(())
    }

    /// ACPI-Shutdown — sendet Shutdown-Signal an die VM, kein erzwungenes Kill.
    pub async fn shutdown(&mut self) -> Result<(), VmError> {
        if self.state != VmState::Running {
            return Err(VmError::NotRunning(self.config.name.clone()));
        }

        let out = Command::new("xl")
            .args(["shutdown", &self.config.name])
            .output()
            .await?;

        if !out.status.success() {
            return Err(VmError::XlFailed(
                String::from_utf8_lossy(&out.stderr).trim().to_owned(),
            ));
        }

        self.state = VmState::Halted;
        self.domain_id = None;
        Ok(())
    }

    /// Sofortiger Kill — vernichtet die Domain ohne ACPI-Shutdown.
    /// Funktioniert auch bei Paused/Crashed. Halted → kein xl-Aufruf nötig.
    pub async fn destroy(&mut self) -> Result<(), VmError> {
        if self.state == VmState::Halted {
            return Ok(());
        }

        let out = Command::new("xl")
            .args(["destroy", &self.config.name])
            .output()
            .await?;

        if !out.status.success() {
            return Err(VmError::XlFailed(
                String::from_utf8_lossy(&out.stderr).trim().to_owned(),
            ));
        }

        self.state = VmState::Halted;
        self.domain_id = None;
        Ok(())
    }
}

pub struct VmManager {
    vms: HashMap<String, Vm>,
}

impl VmManager {
    pub fn new() -> Self {
        Self {
            vms: HashMap::new(),
        }
    }

    pub fn register(&mut self, config: VmConfig) {
        let name = config.name.clone();
        self.vms.insert(name, Vm::new(config));
    }

    pub fn get(&self, name: &str) -> Option<&Vm> {
        self.vms.get(name)
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut Vm> {
        self.vms.get_mut(name)
    }

    pub fn list(&self) -> impl Iterator<Item = &Vm> {
        self.vms.values()
    }
}

impl Default for VmManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::is_valid_vm_name;

    #[test]
    fn accepts_typical_names() {
        for ok in ["sys-gui", "work", "browser", "vault", "x", "a1_2-3"] {
            assert!(is_valid_vm_name(ok), "should accept: {ok}");
        }
    }

    #[test]
    fn rejects_traversal_and_special_chars() {
        for bad in [
            "",
            "../etc/passwd",
            "foo/bar",
            "Foo",        // uppercase verboten — eindeutige IDs
            "name with space",
            "name\nwith\nnewline",
            "name\"quote",
            &"x".repeat(33),
        ] {
            assert!(!is_valid_vm_name(bad), "should reject: {bad:?}");
        }
    }
}
