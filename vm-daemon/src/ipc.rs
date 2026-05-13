// ipc.rs — Inter-VM IPC via Unix-Domain-Sockets
//
// cargo search xenvchan → keine stabilen Bindings (nur 0.0.0-pre Stubs auf crates.io).
// Transport: Unix-Domain-Socket mit 4-Byte-LE-Längen-Präfix + JSON-Body.
// Socket:    /run/krypt/ipc.sock (root:root 0600, erstellt vom Daemon beim Start).
//
// Erweiterbarkeit: VchanTransport kann später durch denselben Framing-Code realisiert
// werden — Callsites in main.rs müssen dann nicht geändert werden.

use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};

pub const SOCKET_PATH: &str = "/run/krypt/ipc.sock";

/// Maximale Framegröße — verhindert Speicherschöpfung durch defekte/böswillige Sender.
const MAX_FRAME: usize = 64 * 1024;

#[derive(Debug, Error)]
pub enum IpcError {
    #[error("i/o: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("connection closed")]
    Closed,
    #[error("frame too large: {0} bytes (max {MAX_FRAME})")]
    FrameTooLarge(usize),
}

/// Alle Nachrichten die zwischen krypt-daemon und AppVM-Agenten fließen.
///
/// Framing: JSON mit `"type"`-Feld als Discriminator.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IpcMessage {
    // Agent → Daemon
    PolicyCheck {
        src_vm: String,
        dst_vm: String,
        service: String,
    },
    VmStatusQuery {
        vm_name: String,
    },
    /// Fragt alle bekannten VMs mit Status + Trust-Level ab (z.B. für Waybar).
    ListVmsQuery {},
    /// Startet eine VM via xl create. Antwort: VmStartResponse oder Error.
    VmStartRequest {
        vm_name: String,
    },
    /// Stoppt eine VM. force=true → xl destroy, false → ACPI xl shutdown.
    VmStopRequest {
        vm_name: String,
        force:   bool,
    },

    // Daemon → Agent (Antworten)
    ListVmsResponse {
        vms: Vec<VmInfo>,
    },
    VmStartResponse {
        vm_name:   String,
        domain_id: Option<u32>,
    },
    VmStopResponse {
        vm_name: String,
    },
    PolicyResponse {
        decision: PolicyDecision,
        /// Optionale Erklärung für AskUser-Dialog
        reason: Option<String>,
    },
    VmStatusResponse {
        vm_name: String,
        state: String,
        domain_id: Option<u32>,
    },

    // Daemon → alle Agenten (Broadcast, keine Antwort erwartet)
    VmStateChanged {
        vm_name: String,
        state: String,
    },

    // Fehlerantwort des Daemons
    Error {
        message: String,
    },
}

/// VM-Info-Eintrag in ListVmsResponse — kombiniert State + Trust-Level.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmInfo {
    pub name:        String,
    pub state:       String,
    pub domain_id:   Option<u32>,
    /// Trust-Level als String: "red" | "orange" | "yellow" | "green" | "black"
    pub trust_level: String,
}

/// Policy-Entscheidung im IPC-Protokoll.
///
/// Entspricht `policy::PolicyAction` — getrennt gehalten damit das IPC-Protokoll
/// unabhängig von internen Rust-Typen serialisierbar bleibt.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyDecision {
    Allow,
    Deny,
    AskUser,
}

/// Framed send/recv über einen UnixStream.
///
/// Protokoll: u32 LE Länge (4 Bytes) gefolgt von JSON-Body (UTF-8).
async fn write_frame(stream: &mut UnixStream, msg: &IpcMessage) -> Result<(), IpcError> {
    let body = serde_json::to_vec(msg)?;
    if body.len() > MAX_FRAME {
        return Err(IpcError::FrameTooLarge(body.len()));
    }
    let len = (body.len() as u32).to_le_bytes();
    stream.write_all(&len).await?;
    stream.write_all(&body).await?;
    Ok(())
}

async fn read_frame(stream: &mut UnixStream) -> Result<IpcMessage, IpcError> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::UnexpectedEof {
            IpcError::Closed
        } else {
            IpcError::Io(e)
        }
    })?;

    let len = u32::from_le_bytes(len_buf) as usize;
    if len > MAX_FRAME {
        return Err(IpcError::FrameTooLarge(len));
    }

    let mut body = vec![0u8; len];
    stream.read_exact(&mut body).await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::UnexpectedEof {
            IpcError::Closed
        } else {
            IpcError::Io(e)
        }
    })?;

    Ok(serde_json::from_slice(&body)?)
}

// ---------------------------------------------------------------------------
// Server-Seite (krypt-daemon in dom0)
// ---------------------------------------------------------------------------

/// Lauscht auf eingehende Agent-Verbindungen auf dem Unix-Domain-Socket.
pub struct IpcServer {
    listener: UnixListener,
}

impl IpcServer {
    /// Bindet `/run/krypt/ipc.sock`. Entfernt einen veralteten Socket automatisch.
    ///
    /// Caller muss sicherstellen dass `/run/krypt/` existiert (via `mkdir -p`).
    pub fn bind(path: &Path) -> Result<Self, IpcError> {
        // Veralteten Socket entfernen — sonst schlägt bind() mit EADDRINUSE fehl
        let _ = std::fs::remove_file(path);
        let listener = UnixListener::bind(path)?;
        tracing::info!("IPC server listening on {}", path.display());
        Ok(Self { listener })
    }

    /// Wartet auf die nächste eingehende Verbindung.
    ///
    /// Typischer Aufruf: `tokio::spawn(async move { loop { server.accept().await } })`.
    pub async fn accept(&self) -> Result<IpcConn, IpcError> {
        let (stream, addr) = self.listener.accept().await?;
        tracing::debug!("IPC: new connection from {:?}", addr);
        Ok(IpcConn { stream })
    }
}

/// Eine einzelne Agent-Verbindung mit framed send/recv.
pub struct IpcConn {
    stream: UnixStream,
}

impl IpcConn {
    /// Empfängt eine Nachricht vom verbundenen Agenten.
    pub async fn recv(&mut self) -> Result<IpcMessage, IpcError> {
        read_frame(&mut self.stream).await
    }

    /// Sendet eine Nachricht an den verbundenen Agenten.
    pub async fn send(&mut self, msg: &IpcMessage) -> Result<(), IpcError> {
        write_frame(&mut self.stream, msg).await
    }
}

// ---------------------------------------------------------------------------
// Client-Seite (AppVM-Agent oder interne Tests)
// ---------------------------------------------------------------------------

/// Verbindet sich mit dem krypt-daemon IPC-Socket.
///
/// Nur verfügbar im Test-Build (`cargo test`) oder mit Feature `agent`
/// (`cargo build --features agent`). Im krypt-daemon Binary selbst wird
/// IpcClient nie instantiiert — er lebt in AppVM-Agenten-Crates.
#[cfg(any(test, feature = "agent"))]
pub struct IpcClient {
    stream: UnixStream,
}

#[cfg(any(test, feature = "agent"))]
impl IpcClient {
    pub async fn connect(path: &Path) -> Result<Self, IpcError> {
        let stream = UnixStream::connect(path).await?;
        Ok(Self { stream })
    }

    pub async fn send(&mut self, msg: &IpcMessage) -> Result<(), IpcError> {
        write_frame(&mut self.stream, msg).await
    }

    pub async fn recv(&mut self) -> Result<IpcMessage, IpcError> {
        read_frame(&mut self.stream).await
    }

    /// Sendet eine Anfrage und wartet auf genau eine Antwort.
    pub async fn request(&mut self, msg: &IpcMessage) -> Result<IpcMessage, IpcError> {
        self.send(msg).await?;
        self.recv().await
    }

    /// Fragt krypt-daemon nach allen bekannten VMs (Status + Trust-Level).
    pub async fn list_vms(&mut self) -> Result<Vec<VmInfo>, IpcError> {
        match self.request(&IpcMessage::ListVmsQuery {}).await? {
            IpcMessage::ListVmsResponse { vms } => Ok(vms),
            IpcMessage::Error { message } => {
                Err(IpcError::Io(std::io::Error::other(message)))
            }
            other => Err(IpcError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("unexpected response to ListVmsQuery: {:?}", other),
            ))),
        }
    }

    /// Startet eine VM und gibt die Domain-ID zurück (None wenn nicht ermittelbar).
    pub async fn start_vm(&mut self, vm_name: &str) -> Result<Option<u32>, IpcError> {
        let req = IpcMessage::VmStartRequest { vm_name: vm_name.to_owned() };
        match self.request(&req).await? {
            IpcMessage::VmStartResponse { domain_id, .. } => Ok(domain_id),
            IpcMessage::Error { message } => Err(IpcError::Io(std::io::Error::other(message))),
            other => Err(IpcError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("unexpected response to VmStartRequest: {:?}", other),
            ))),
        }
    }

    /// Stoppt eine VM. `force=true` → xl destroy, `force=false` → ACPI xl shutdown.
    pub async fn stop_vm(&mut self, vm_name: &str, force: bool) -> Result<(), IpcError> {
        let req = IpcMessage::VmStopRequest { vm_name: vm_name.to_owned(), force };
        match self.request(&req).await? {
            IpcMessage::VmStopResponse { .. } => Ok(()),
            IpcMessage::Error { message } => Err(IpcError::Io(std::io::Error::other(message))),
            other => Err(IpcError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("unexpected response to VmStopRequest: {:?}", other),
            ))),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn tmp_socket() -> (TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("test-ipc.sock");
        (dir, path)
    }

    #[tokio::test]
    async fn roundtrip_policy_check() {
        let (_dir, path) = tmp_socket();

        let server = IpcServer::bind(&path).expect("bind");

        let path_clone = path.clone();
        let client_task = tokio::spawn(async move {
            let mut client = IpcClient::connect(&path_clone).await.expect("connect");
            let response = client
                .request(&IpcMessage::PolicyCheck {
                    src_vm: "browser".into(),
                    dst_vm: "vault".into(),
                    service: "qubes.CopyToVM".into(),
                })
                .await
                .expect("request");
            response
        });

        let mut conn = server.accept().await.expect("accept");
        let msg = conn.recv().await.expect("recv");

        // Prüfen dass die Nachricht korrekt deserialisiert wurde
        assert!(matches!(msg, IpcMessage::PolicyCheck { .. }));

        conn.send(&IpcMessage::PolicyResponse {
            decision: PolicyDecision::Deny,
            reason: Some("Red→Black".into()),
        })
        .await
        .expect("send response");

        let response = client_task.await.expect("client task");
        assert!(matches!(response, IpcMessage::PolicyResponse { .. }));
    }

    #[tokio::test]
    async fn frame_too_large_rejected() {
        let (_dir, path) = tmp_socket();
        let server = IpcServer::bind(&path).expect("bind");

        let path_clone = path.clone();
        tokio::spawn(async move {
            // Sende einen künstlich großen Frame
            let stream = UnixStream::connect(&path_clone).await.expect("connect");
            let mut stream = stream;
            let huge_len = (MAX_FRAME + 1) as u32;
            stream.write_all(&huge_len.to_le_bytes()).await.ok();
        });

        let mut conn = server.accept().await.expect("accept");
        let result = conn.recv().await;
        assert!(matches!(result, Err(IpcError::FrameTooLarge(_))));
    }

    #[tokio::test]
    async fn roundtrip_list_vms() {
        let (_dir, path) = tmp_socket();
        let server = IpcServer::bind(&path).expect("bind");

        let path_clone = path.clone();
        let client_task = tokio::spawn(async move {
            let mut client = IpcClient::connect(&path_clone).await.expect("connect");
            client.list_vms().await.expect("list_vms")
        });

        let mut conn = server.accept().await.expect("accept");
        let msg = conn.recv().await.expect("recv");
        assert!(matches!(msg, IpcMessage::ListVmsQuery {}));

        conn.send(&IpcMessage::ListVmsResponse {
            vms: vec![
                VmInfo {
                    name:        "work".into(),
                    state:       "Running".into(),
                    domain_id:   Some(42),
                    trust_level: "green".into(),
                },
                VmInfo {
                    name:        "vault".into(),
                    state:       "Halted".into(),
                    domain_id:   None,
                    trust_level: "black".into(),
                },
            ],
        })
        .await
        .expect("send ListVmsResponse");

        let vms = client_task.await.expect("client task");
        assert_eq!(vms.len(), 2);

        let work  = vms.iter().find(|v| v.name == "work").expect("work vm");
        let vault = vms.iter().find(|v| v.name == "vault").expect("vault vm");

        assert_eq!(work.state,       "Running");
        assert_eq!(work.domain_id,   Some(42));
        assert_eq!(work.trust_level, "green");

        assert_eq!(vault.state,       "Halted");
        assert_eq!(vault.domain_id,   None);
        assert_eq!(vault.trust_level, "black");
    }

    #[tokio::test]
    async fn list_vms_daemon_error_propagates() {
        let (_dir, path) = tmp_socket();
        let server = IpcServer::bind(&path).expect("bind");

        let path_clone = path.clone();
        let client_task = tokio::spawn(async move {
            let mut client = IpcClient::connect(&path_clone).await.expect("connect");
            client.list_vms().await
        });

        let mut conn = server.accept().await.expect("accept");
        let _ = conn.recv().await.expect("recv");
        conn.send(&IpcMessage::Error {
            message: "daemon overloaded".into(),
        })
        .await
        .expect("send error");

        let result = client_task.await.expect("client task");
        assert!(result.is_err());
    }

    #[test]
    fn messages_serialize_with_type_tag() {
        let msg = IpcMessage::PolicyCheck {
            src_vm: "work".into(),
            dst_vm: "personal".into(),
            service: "clipboard".into(),
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        assert!(json.contains("\"type\":\"policy_check\""));
    }

    #[tokio::test]
    async fn roundtrip_vm_start() {
        let (_dir, path) = tmp_socket();
        let server = IpcServer::bind(&path).expect("bind");

        let path_clone = path.clone();
        let client_task = tokio::spawn(async move {
            let mut client = IpcClient::connect(&path_clone).await.expect("connect");
            client.start_vm("work").await.expect("start_vm")
        });

        let mut conn = server.accept().await.expect("accept");
        let msg = conn.recv().await.expect("recv");
        assert!(matches!(msg, IpcMessage::VmStartRequest { ref vm_name } if vm_name == "work"));

        conn.send(&IpcMessage::VmStartResponse {
            vm_name:   "work".into(),
            domain_id: Some(7),
        })
        .await
        .expect("send response");

        let domain_id = client_task.await.expect("client task");
        assert_eq!(domain_id, Some(7));
    }

    #[tokio::test]
    async fn roundtrip_vm_stop() {
        let (_dir, path) = tmp_socket();
        let server = IpcServer::bind(&path).expect("bind");

        let path_clone = path.clone();
        let client_task = tokio::spawn(async move {
            let mut client = IpcClient::connect(&path_clone).await.expect("connect");
            client.stop_vm("work", false).await.expect("stop_vm")
        });

        let mut conn = server.accept().await.expect("accept");
        let msg = conn.recv().await.expect("recv");
        assert!(matches!(
            msg,
            IpcMessage::VmStopRequest { ref vm_name, force: false } if vm_name == "work"
        ));

        conn.send(&IpcMessage::VmStopResponse { vm_name: "work".into() })
            .await
            .expect("send response");

        client_task.await.expect("client task");
    }

    #[tokio::test]
    async fn vm_start_error_propagates() {
        let (_dir, path) = tmp_socket();
        let server = IpcServer::bind(&path).expect("bind");

        let path_clone = path.clone();
        let client_task = tokio::spawn(async move {
            let mut client = IpcClient::connect(&path_clone).await.expect("connect");
            client.start_vm("unknown-vm").await
        });

        let mut conn = server.accept().await.expect("accept");
        let _ = conn.recv().await.expect("recv");
        conn.send(&IpcMessage::Error { message: "VM 'unknown-vm' not found".into() })
            .await
            .expect("send error");

        let result = client_task.await.expect("client task");
        assert!(result.is_err());
    }
}
