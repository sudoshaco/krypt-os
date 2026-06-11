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
    // Tracing ZUERST initialisieren — sonst gehen Config-Parse-Fehler verloren.
    // Log-Level kommt initial aus RUST_LOG (env), Config kann ihn später NICHT
    // mehr ändern (tracing-subscriber ist global initialisiert).
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let config_path = Path::new("/etc/krypt/daemon.toml");
    let cfg = if config_path.exists() {
        match config::KryptConfig::load(config_path) {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("config parse failed ({}): {e}", config_path.display());
                return Err(e.into());
            }
        }
    } else {
        tracing::warn!("no config at {}, using defaults", config_path.display());
        config::KryptConfig::default()
    };
    tracing::debug!("config log_level={} (note: only RUST_LOG is honored)", cfg.daemon.log_level);

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
    // umask 0o077 vor bind() — socket wird sofort mit perms 0600 erzeugt,
    // keine TOCTOU-Lücke zwischen bind() und set_permissions().
    // SAFETY: umask ist process-global. Wir restoren danach den alten Wert.
    let prev_umask = unsafe { libc::umask(0o077) };
    let ipc_server = match ipc::IpcServer::bind(ipc_path) {
        Ok(s) => {
            // Extra-Belt: chmod auch noch, falls eine FS-Implementierung
            // umask ignoriert (z. B. manche tmpfs-Setups).
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o600);
            let _ = std::fs::set_permissions(ipc_path, perms);
            tracing::info!("IPC server bound on {} (0600)", ipc_path.display());
            Some(s)
        }
        Err(e) => {
            tracing::error!("IPC bind failed ({}): VM management offline", e);
            None
        }
    };
    // SAFETY: umask wieder auf vorherigen Wert
    unsafe { libc::umask(prev_umask); }

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
                // Backoff bei transienten accept-Fehlern. EMFILE/ENFILE etc
                // sollten nicht den ganzen IPC-Service killen — kurz warten,
                // dann nochmal. Bei dauerhaftem Fehler steigt der Delay.
                let mut backoff_ms: u64 = 100;
                loop {
                    match server.accept().await {
                        Ok(mut conn) => {
                            backoff_ms = 100; // reset bei Erfolg
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
                            tracing::warn!("IPC accept error: {e} (retry in {backoff_ms}ms)");
                            tokio::time::sleep(
                                std::time::Duration::from_millis(backoff_ms)
                            ).await;
                            backoff_ms = (backoff_ms * 2).min(30_000);
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
                        trigger_panic(panic_level).await;
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
///
/// Bevorzugt /usr/bin/krypt-panic — diese Binary friert vor Suspend/Poweroff
/// erst alle AppVMs ein (xl pause/destroy), was die README-Garantie
/// "Stick raus = alle VMs sofort eingefroren" tatsächlich umsetzt.
///
/// Fallback (krypt-panic fehlt oder ExitCode≠0): inline systemctl/loginctl —
/// derselbe Code-Pfad wie vor Phase 11. Ohne VM-Freeze, aber besser als
/// nichts. Wird mit einer eindeutigen tracing-Warnung markiert damit Ops
/// im Journal sieht warum die Side-Effects fehlen.
///
/// Verwendet tokio::process — std::process::Command::status() würde sonst
/// die Tokio-Runtime blockieren.
async fn trigger_panic(level: config::PanicLevel) {
    use tokio::process::Command;

    let level_arg = match level {
        config::PanicLevel::Lock    => "lock",
        config::PanicLevel::Suspend => "suspend",
        config::PanicLevel::Nuke    => "nuke",
    };

    tracing::warn!("PANIC level={level_arg} — invoking /usr/bin/krypt-panic");
    let primary = Command::new("/usr/bin/krypt-panic")
        .arg(format!("--level={level_arg}"))
        .status()
        .await;

    match primary {
        Ok(s) if s.success() => return,
        Ok(s) => tracing::warn!("krypt-panic exited with {s} — falling back to inline panic"),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            tracing::warn!(
                "/usr/bin/krypt-panic not installed — falling back to inline panic \
                 (no VM-freeze step, install krypt-panic to get full coverage)"
            );
        }
        Err(e) => tracing::warn!("krypt-panic failed to spawn ({e}) — falling back to inline panic"),
    }

    // Fallback path — VM-Freeze inline + systemctl/loginctl. Vorher fehlte
    // hier der Freeze, weshalb der Fallback bei Suspend/Nuke laufende AppVMs
    // im Speicher hinterließ. Wir wollen einen minimalen aber funktional
    // analogen Pfad zu krypt-panic — ohne dessen libc::reboot-Hammer.
    freeze_running_appvms().await;

    let result = match level {
        config::PanicLevel::Lock => {
            tracing::warn!("PANIC fallback: locking all sessions (loginctl lock-sessions)");
            Command::new("loginctl").args(["lock-sessions"]).status().await
        }
        config::PanicLevel::Suspend => {
            tracing::warn!("PANIC fallback: suspending system (systemctl suspend)");
            Command::new("systemctl").args(["suspend"]).status().await
        }
        config::PanicLevel::Nuke => {
            tracing::warn!("PANIC fallback: emergency poweroff (systemctl poweroff --force --force)");
            Command::new("systemctl")
                .args(["poweroff", "--force", "--force"])
                .status()
                .await
        }
    };
    if let Err(e) = result {
        tracing::error!("PANIC ACTION FAILED: {e} — Auth-Stick wurde abgezogen, System bleibt aktiv!");
    }
}

/// Iteriert `xl list` und ruft `xl pause <name>` für jede Domain außer Domain-0.
///
/// Best-effort: einzelne pause-Fehler werden geloggt, blockieren aber nicht
/// den nachfolgenden Suspend/Poweroff. Wird im Fallback-Pfad von
/// `trigger_panic` aufgerufen, also wenn /usr/bin/krypt-panic nicht
/// verfügbar war. krypt-panic macht das gleiche selbst.
async fn freeze_running_appvms() {
    use tokio::process::Command;

    let listing = match Command::new("xl").arg("list").output().await {
        Ok(o) if o.status.success() => o.stdout,
        Ok(o) => {
            tracing::warn!(
                "fallback freeze: xl list exit={:?} stderr={}",
                o.status,
                String::from_utf8_lossy(&o.stderr).trim()
            );
            return;
        }
        Err(e) => {
            tracing::warn!("fallback freeze: xl list spawn failed: {e}");
            return;
        }
    };

    let text = String::from_utf8_lossy(&listing);
    let mut paused = 0;
    // Erste Zeile ist Header ("Name ID Mem VCPUs ..."), skip()n
    for line in text.lines().skip(1) {
        let mut parts = line.split_whitespace();
        let Some(name) = parts.next() else { continue };
        if name == "Domain-0" {
            continue;
        }
        match Command::new("xl").args(["pause", name]).status().await {
            Ok(s) if s.success() => paused += 1,
            Ok(s) => tracing::warn!("fallback freeze: xl pause {name} exit={s}"),
            Err(e) => tracing::warn!("fallback freeze: xl pause {name} spawn failed: {e}"),
        }
    }
    tracing::warn!("fallback freeze: paused {paused} AppVM(s) before system action");
}
