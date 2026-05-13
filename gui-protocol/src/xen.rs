// xen.rs — Xen Shared Memory Interface für Pixel-Daten
//
// AppVMs schreiben ihren Framebuffer in Xen Shared Memory Pages (Grant Table).
// krypt-gui liest diese Pages und übergibt die Pixel an wayland.rs.
//
// Xen Grant Table Ablauf:
//   1. AppVM: xenstore schreibt grant-ref + domid
//   2. dom0: xengnttab_map_grant_refs() mapped die Pages in dom0-Adressraum
//   3. dom0: Pixels lesen, an Wayland-Buffer übergeben
//   4. AppVM: Framebuffer-Update → Event via xenevtchn
//
// Phase 10: Frame-Pacing-Stub (16ms/60fps) + DirtyRect.
//           poll_dirty_regions() non-blocking — gibt geänderte Regionen zurück.
// Phase 11: libxengnttab FFI + xenevtchn Event-Channel.
#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum XenError {
    #[error("xenstore connection failed: {0}")]
    Xenstore(String),
    #[error("grant table mapping failed: domid={domid} gref={gref}: {msg}")]
    GrantMap { domid: u32, gref: u32, msg: String },
    #[error("event channel error: {0}")]
    EventChannel(String),
    #[error("VM disconnected: domid={0}")]
    Disconnected(u32),
}

/// Xen Domain-ID einer AppVM.
pub type DomId = u32;

/// Grant-Reference — Zeiger auf eine geteilte Memory-Page.
pub type GrantRef = u32;

/// Pixel-Format der AppVM (entspricht qubes-gui-daemon Protokoll).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    Argb32,
    Xrgb32,
}

/// Geänderte Bildschirmregion — nur dieser Bereich muss neu gerendert werden.
///
/// Phase 11: aus xenevtchn-Protokoll der AppVM extrahiert.
/// Phase 10: poll_dirty_regions() gibt vollen Frame zurück (konservativ).
#[derive(Debug, Clone, Copy)]
pub struct DirtyRect {
    pub x:      u32,
    pub y:      u32,
    pub width:  u32,
    pub height: u32,
}

impl DirtyRect {
    /// Vollständiger Frame — gesamte Surface ist dirty.
    pub fn full(width: u32, height: u32) -> Self {
        Self { x: 0, y: 0, width, height }
    }
}

/// Metadata-Block den die AppVM via XenStore publiziert.
#[derive(Debug, Clone)]
pub struct GuestMetadata {
    pub domid:        DomId,
    pub vm_name:      String,
    pub grant_refs:   Vec<GrantRef>,
    pub width:        u32,
    pub height:       u32,
    pub pixel_format: PixelFormat,
}

/// Gemappter Shared Memory Buffer einer AppVM.
///
/// Phase 11: wraps gemappte Kernel-Pages via xengnttab_map_grant_refs().
pub struct SharedBuffer {
    pub meta: GuestMetadata,
    // Phase 11: ptr: *mut u8, len: usize, _mapping: XenGrantMapping,
}

impl SharedBuffer {
    /// Liest den aktuellen Framebuffer-Inhalt.
    ///
    /// Phase 11: unsafe { slice::from_raw_parts(self.ptr, self.len) }
    pub fn pixels(&self) -> &[u8] {
        &[]
    }

    pub fn width(&self)  -> u32 { self.meta.width }
    pub fn height(&self) -> u32 { self.meta.height }
    pub fn stride(&self) -> u32 { self.meta.width * 4 }
}

/// Xen-Interface — Verbindung zu Xenstore und Grant Table.
///
/// Phase 10: Stub mit 16ms Frame-Pacing.
/// Phase 11: xenstore_open() + xengnttab_open() + xenevtchn_open().
pub struct XenInterface {
    /// Letzter Frame-Zeitpunkt pro DomId — für 60fps-Pacing in der Stub-Implementierung.
    frame_times: Mutex<HashMap<DomId, Instant>>,
}

/// Ziel-Frame-Intervall: 16ms ≈ 60fps.
const FRAME_INTERVAL: Duration = Duration::from_millis(16);

impl XenInterface {
    pub fn open() -> Result<Self, XenError> {
        tracing::info!("xen: interface open (Phase 10 stub — xengnttab FFI pending)");
        Ok(Self { frame_times: Mutex::new(HashMap::new()) })
    }

    /// Prüft ob neue Framebuffer-Daten vorliegen (non-blocking).
    ///
    /// Gibt geänderte Regionen zurück. Leer = kein Update seit letztem Aufruf.
    ///
    /// Phase 10 Stub: gibt alle 16ms einen vollen Frame zurück (60fps-Pacing).
    /// Phase 11: xenevtchn_pending() + dirty-region-Paket aus AppVM-Protokoll.
    pub fn poll_dirty_regions(&self, domid: DomId) -> Vec<DirtyRect> {
        let mut map = match self.frame_times.lock() {
            Ok(m)  => m,
            Err(_) => return Vec::new(),
        };
        let last = map.entry(domid).or_insert_with(|| {
            // Erster Aufruf: sofort dirty damit die erste Surface sichtbar wird
            Instant::now() - FRAME_INTERVAL
        });
        if last.elapsed() >= FRAME_INTERVAL {
            *last = Instant::now();
            // Vollständigen Frame als dirty melden — Phase 11: echte Dirty-Regionen
            vec![DirtyRect::full(u32::MAX, u32::MAX)]
        } else {
            Vec::new()
        }
    }

    /// Wartet auf die nächste AppVM-Verbindung (publiziert via XenStore).
    ///
    /// Phase 11: XenStore-Watch auf /local/domain/*/krypt-gui/metadata
    pub async fn accept_guest(&self) -> Result<SharedBuffer, XenError> {
        futures::future::pending::<()>().await;
        unreachable!()
    }

    /// Wartet auf Framebuffer-Update-Events einer laufenden AppVM.
    ///
    /// Phase 11: xenevtchn_pending() auf dem Event-Channel der AppVM.
    pub async fn wait_for_update(&self, _domid: DomId) -> Result<(), XenError> {
        futures::future::pending::<()>().await;
        unreachable!()
    }

    /// Liest die aktuellen Pixel einer AppVM für die angegebenen Dirty-Regionen.
    ///
    /// Phase 11: memcpy aus Xen Grant Memory Pages.
    /// Phase 10: gibt leeren Buffer zurück (Caller generiert Test-Pixel).
    pub fn read_pixels(&self, _domid: DomId, _dirty: &[DirtyRect]) -> Vec<u8> {
        Vec::new()
    }
}
