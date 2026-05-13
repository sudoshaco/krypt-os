// krypt-daemon — VM Management Daemon für Krypt OS
// Ersetzt qubesd. Geschrieben in Rust für Memory-Safety.
//
// Architektur:
//   - Policy Engine: welche VM darf mit welcher kommunizieren
//   - VM Lifecycle: start / stop / create / destroy
//   - IPC: Unix-Domain-Socket basierter Inter-VM-Kanal (vchan Phase 6+)
//   - USB Monitor: Auth-Stick-Überwachung, Kill-Switch bei Entfernung
//
// tokio-udev's AsyncMonitorSocket ist !Send (wraps *mut udev_monitor) →
// USB-Monitor läuft als spawn_local-Task in einer LocalSet.

mod config;
mod ipc;
mod policy;
mod usb;
mod vm;

use std::sync::Arc;
use std::error::Error;
use std::path::Path;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let config_path = Path::new("/etc/krypt/daemon.toml");
    let cfg = if config_path.exists() {
        config::KryptConfig::load(config_path)?
    } else {
        eprintln!("krypt-daemon: no config at {}, using defaults", config_path.display());
        config::KryptConfig::default()
    };

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_new(&cfg.daemon.log_level)
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tracing::info!(
        "krypt-daemon starting (panic_level={:?}, vms={}, sticks={})",
        cfg.daemon.panic_level,
        cfg.vms.len(),
        cfg.auth_sticks.len(),
    );

    // Policy Engine: Trust-Level + explizite Regeln aus Config laden
    let mut policy_engine = policy::PolicyEngine::new();
    policy_engine.load_from_config(&cfg);
    tracing::debug!(
        "policy engine ready: {} rule(s), {} VM trust entries",
        cfg.policy.len(),
        cfg.vms.len(),
    );

    // VM Manager aus Config aufbauen
    let mut vm_manager = vm::VmManager::new();
    for vm_entry in &cfg.vms {
        vm_manager.register(vm::VmConfig {
            name:      vm_entry.name.clone(),
            memory_mb: vm_entry.memory_mb,
            vcpus:     vm_entry.vcpus,
            kernel:    vm_entry.kernel.clone(),
            ramdisk:   vm_entry.ramdisk.as_ref().map(|p| p.to_string_lossy().into_owned()),
            root_disk: vm_entry.root_disk.as_ref().map(|p| p.to_string_lossy().into_owned()),
            xl_cfg:    None,
        });
    }
    tracing::debug!("registered {} VM(s)", cfg.vms.len());

    // USB Monitor
    let mut usb_monitor = usb::UsbMonitor::new();
    for stick in &cfg.auth_sticks {
        usb_monitor.register_stick(stick.serial.clone(), stick.luks_slot);
        tracing::debug!("registered auth stick: serial={}", stick.serial);
    }

    let panic_level = cfg.daemon.panic_level;
    let (usb_tx, mut usb_rx) = tokio::sync::mpsc::channel::<usb::UsbEvent>(32);

    // Shared state für IPC-Dispatch (Arc<RwLock<>> — mehrere Connections parallel)
    let policy_engine = Arc::new(tokio::sync::RwLock::new(policy_engine));
    let vm_manager    = Arc::new(tokio::sync::RwLock::new(vm_manager));

    // IPC-Server starten — /run/krypt/ anlegen (systemd RuntimeDirectory=krypt)
    let ipc_path = Path::new(ipc::SOCKET_PATH);
    if let Some(parent) = ipc_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let ipc_server = match ipc::IpcServer::bind(ipc_path) {
        Ok(s) => {
            // Socket auf root-only beschränken
            {
                use std::os::unix::fs::PermissionsExt;
                let perms = std::fs::Permissions::from_mode(0o600);
                let _ = std::fs::set_permissions(ipc_path, perms);
            }
            tracing::info!("IPC server bound on {} (0600)", ipc_path.display());
            Some(s)
        }
        Err(e) => {
            tracing::warn!("IPC bind failed ({}): running without IPC", e);
            None
        }
    };

    let local = tokio::task::LocalSet::new();

    local.run_until(async move {
        // USB Monitor als lokaler Task (!Send)
        tokio::task::spawn_local(async move {
            if let Err(e) = usb_monitor.run(usb_tx).await {
                tracing::error!("USB monitor exited with error: {e}");
            }
        });

        // IPC accept-Loop — UnixListener ist Send
        if let Some(server) = ipc_server {
            let pe = Arc::clone(&policy_engine);
            let vm = Arc::clone(&vm_manager);
            tokio::spawn(async move {
                loop {
                    match server.accept().await {
                        Ok(mut conn) => {
                            let pe = Arc::clone(&pe);
                            let vm = Arc::clone(&vm);
                            tokio::spawn(async move {
                                loop {
                                    match conn.recv().await {
                                        Ok(msg) => {
                                            let response = dispatch_ipc(msg, &pe, &vm).await;
                                            if let Err(e) = conn.send(&response).await {
                                                tracing::warn!("IPC send error: {e}");
                                                break;
                                            }
                                        }
                                        Err(ipc::IpcError::Closed) => break,
                                        Err(e) => {
                                            tracing::warn!("IPC conn error: {e}");
                                            break;
                                        }
                                    }
                                }
                            });
                        }
                        Err(e) => {
                            tracing::error!("IPC accept error: {e}");
                            break;
                        }
                    }
                }
            });
        }

        let mut sigterm = tokio::signal::unix::signal(
            tokio::signal::unix::SignalKind::terminate(),
        )
        .expect("failed to register SIGTERM handler");

        tracing::info!("krypt-daemon ready");

        loop {
            tokio::select! {
                event = usb_rx.recv() => match event {
                    Some(usb::UsbEvent::AuthStickRemoved(dev)) => {
                        tracing::warn!(
                            "AUTH STICK REMOVED (serial={:?}) — executing panic level: {:?}",
                            dev.serial,
                            panic_level,
                        );
                        trigger_panic(panic_level);
                    }
                    Some(usb::UsbEvent::AuthStickConnected(dev)) => {
                        tracing::info!("auth stick connected: serial={:?}", dev.serial);
                    }
                    Some(usb::UsbEvent::Unknown(_)) => {}
                    None => {
                        tracing::error!("USB monitor channel closed — monitor may have crashed");
                        break;
                    }
                },
                _ = sigterm.recv() => {
                    tracing::info!("SIGTERM received — shutting down");
                    break;
                },
                result = tokio::signal::ctrl_c() => {
                    if result.is_ok() {
                        tracing::info!("SIGINT received — shutting down");
                    }
                    break;
                },
            }
        }
    })
    .await;

    tracing::info!("krypt-daemon stopped");
    Ok(())
}

/// IPC-Nachricht verarbeiten und synchron eine Antwort zurückgeben.
async fn dispatch_ipc(
    msg: ipc::IpcMessage,
    policy: &Arc<tokio::sync::RwLock<policy::PolicyEngine>>,
    vms: &Arc<tokio::sync::RwLock<vm::VmManager>>,
) -> ipc::IpcMessage {
    match msg {
        ipc::IpcMessage::PolicyCheck { src_vm, dst_vm, service } => {
            let engine = policy.read().await;
            let decision = match engine.check(&src_vm, &dst_vm) {
                policy::PolicyAction::Allow   => ipc::PolicyDecision::Allow,
                policy::PolicyAction::Deny    => ipc::PolicyDecision::Deny,
                policy::PolicyAction::AskUser => ipc::PolicyDecision::AskUser,
            };
            let reason = format!("{src_vm} → {dst_vm} [{service}]");
            tracing::debug!("policy check: {reason} → {decision:?}");
            ipc::IpcMessage::PolicyResponse { decision, reason: Some(reason) }
        }

        ipc::IpcMessage::VmStatusQuery { vm_name } => {
            let manager = vms.read().await;
            match manager.get(&vm_name) {
                Some(vm) => ipc::IpcMessage::VmStatusResponse {
                    vm_name:   vm_name.clone(),
                    state:     format!("{:?}", vm.state),
                    domain_id: vm.domain_id,
                },
                None => ipc::IpcMessage::Error {
                    message: format!("VM '{vm_name}' not found"),
                },
            }
        }

        ipc::IpcMessage::ListVmsQuery {} => {
            let manager = vms.read().await;
            let engine  = policy.read().await;
            let vms_list: Vec<ipc::VmInfo> = manager
                .list()
                .map(|vm| ipc::VmInfo {
                    name:        vm.config.name.clone(),
                    state:       format!("{:?}", vm.state),
                    domain_id:   vm.domain_id,
                    trust_level: engine.get_trust(&vm.config.name).to_str().to_owned(),
                })
                .collect();
            tracing::debug!("ListVmsQuery → {} VM(s)", vms_list.len());
            ipc::IpcMessage::ListVmsResponse { vms: vms_list }
        }

        ipc::IpcMessage::VmStartRequest { vm_name } => {
            let mut manager = vms.write().await;
            match manager.get_mut(&vm_name) {
                Some(vm) => match vm.start().await {
                    Ok(()) => {
                        let domain_id = vm.domain_id;
                        tracing::info!("VM '{}' started (domid={:?})", vm_name, domain_id);
                        ipc::IpcMessage::VmStartResponse { vm_name, domain_id }
                    }
                    Err(e) => {
                        tracing::warn!("VM '{}' start failed: {e}", vm_name);
                        ipc::IpcMessage::Error { message: format!("start failed: {e}") }
                    }
                },
                None => ipc::IpcMessage::Error {
                    message: format!("VM '{vm_name}' not found"),
                },
            }
        }

        ipc::IpcMessage::VmStopRequest { vm_name, force } => {
            let mut manager = vms.write().await;
            match manager.get_mut(&vm_name) {
                Some(vm) => {
                    let result = if force { vm.destroy().await } else { vm.shutdown().await };
                    match result {
                        Ok(()) => {
                            tracing::info!("VM '{}' stopped (force={force})", vm_name);
                            ipc::IpcMessage::VmStopResponse { vm_name }
                        }
                        Err(e) => {
                            tracing::warn!("VM '{}' stop failed: {e}", vm_name);
                            ipc::IpcMessage::Error { message: format!("stop failed: {e}") }
                        }
                    }
                }
                None => ipc::IpcMessage::Error {
                    message: format!("VM '{vm_name}' not found"),
                },
            }
        }

        // Daemon empfängt keine Antwort-Nachrichten — das sind Client-only-Typen
        other => {
            tracing::warn!("IPC: unexpected inbound message: {:?}", other);
            ipc::IpcMessage::Error {
                message: "unexpected message type for daemon endpoint".into(),
            }
        }
    }
}

/// Führt die konfigurierte Panik-Aktion aus wenn der Auth-Stick entfernt wird.
fn trigger_panic(level: config::PanicLevel) {
    use std::process::Command;
    match level {
        config::PanicLevel::Lock => {
            tracing::warn!("PANIC: locking all sessions (loginctl lock-sessions)");
            let _ = Command::new("loginctl").args(["lock-sessions"]).status();
        }
        config::PanicLevel::Suspend => {
            tracing::warn!("PANIC: suspending system (systemctl suspend)");
            let _ = Command::new("systemctl").args(["suspend"]).status();
        }
        config::PanicLevel::Nuke => {
            tracing::warn!("PANIC: emergency poweroff (systemctl poweroff --force --force)");
            let _ = Command::new("systemctl")
                .args(["poweroff", "--force", "--force"])
                .status();
        }
    }
}
