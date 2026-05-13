// input.rs — Keyboard/Mouse-Event-Isolation für AppVM-Fenster
//
// Wenn der User in ein AppVM-Fenster klickt oder tippt, dürfen Events
// NUR an die zugehörige AppVM weitergeleitet werden — niemals an andere VMs.
//
// Isolationsprinzip:
//   - Focus-State ist dom0-exklusiv (krypt-gui-protocol entscheidet)
//   - AppVM erhält nur Events wenn ihr Fenster den Wayland-Focus hat
//   - Clipboard-Transfer braucht explizite Nutzer-Bestätigung (ADR: Phase 11)
//   - Keine automatische Clipboard-Synchronisation zwischen Trust-Levels
//
// Phase 9: Wayland-Input-Events (wl_seat, wl_keyboard, wl_pointer) empfangen.
// Phase 10: Events via Xen Event Channel an AppVM senden.
// Phase 8: Datenstrukturen + Routing-Logik definieren.
#![allow(dead_code)]

use thiserror::Error;
use crate::xen::DomId;

#[derive(Debug, Error)]
pub enum InputError {
    #[error("no focused window")]
    NoFocus,
    #[error("event channel send failed: domid={domid}: {msg}")]
    EventSend { domid: DomId, msg: String },
    #[error("clipboard transfer denied: {reason}")]
    ClipboardDenied { reason: String },
}

/// Keyboard-Event (Wayland → AppVM).
#[derive(Debug, Clone)]
pub struct KeyEvent {
    pub keycode:   u32,
    pub keysym:    u32,
    pub pressed:   bool,
    /// Modifiers (Shift/Ctrl/Alt/Super) als Bitmaske
    pub modifiers: u32,
}

/// Maus-Event (Wayland → AppVM).
#[derive(Debug, Clone)]
pub enum MouseEvent {
    Move   { x: f64, y: f64 },
    Button { button: u32, pressed: bool },
    Scroll { dx: f64, dy: f64 },
    Enter  { x: f64, y: f64 },
    Leave,
}

/// Clipboard-Transfer-Anfrage — braucht Nutzer-Bestätigung.
#[derive(Debug, Clone)]
pub struct ClipboardRequest {
    pub src_domid:  DomId,
    pub dst_domid:  DomId,
    pub src_trust:  String,
    pub dst_trust:  String,
    pub size_bytes: usize,
}

/// Aktuell fokussiertes AppVM-Fenster.
#[derive(Debug, Clone)]
pub struct FocusedWindow {
    pub domid:   DomId,
    pub vm_name: String,
}

/// Input-Router — leitet Events vom Wayland-Seat zur fokussierten AppVM.
pub struct InputRouter {
    focused: Option<FocusedWindow>,
}

impl InputRouter {
    pub fn new() -> Self {
        Self { focused: None }
    }

    /// Setzt das fokussierte Fenster (Wayland-Focus-Event).
    pub fn set_focus(&mut self, window: Option<FocusedWindow>) {
        if let Some(ref w) = window {
            tracing::debug!("input focus → {} (domid={})", w.vm_name, w.domid);
        } else {
            tracing::debug!("input focus → none");
        }
        self.focused = window;
    }

    /// Leitet ein Keyboard-Event an die fokussierte AppVM.
    ///
    /// Phase 10: xenevtchn_notify() mit serialisiertem KeyEvent.
    pub fn route_key(&self, event: &KeyEvent) -> Result<(), InputError> {
        let window = self.focused.as_ref().ok_or(InputError::NoFocus)?;
        tracing::trace!(
            "key {} → {} (domid={})",
            if event.pressed { "press" } else { "release" },
            window.vm_name, window.domid,
        );
        // Phase 10: serialize event → Xen Event Channel
        let _ = event;
        Ok(())
    }

    /// Leitet ein Maus-Event an die fokussierte AppVM.
    ///
    /// Phase 10: xenevtchn_notify() mit serialisiertem MouseEvent.
    pub fn route_mouse(&self, event: &MouseEvent) -> Result<(), InputError> {
        let window = self.focused.as_ref().ok_or(InputError::NoFocus)?;
        tracing::trace!("mouse event → {} (domid={})", window.vm_name, window.domid);
        // Phase 10: serialize event → Xen Event Channel
        let _ = event;
        Ok(())
    }

    /// Prüft ob ein Clipboard-Transfer erlaubt ist und fragt Nutzer.
    ///
    /// Krypt-Prinzip: Clipboard NIEMALS automatisch zwischen Trust-Levels.
    /// Nutzer muss explizit bestätigen (via krypt-notify Dialog in sys-gui).
    ///
    /// Phase 11: Dialog via D-Bus/Wayland-Protocol → Nutzer-Bestätigung.
    pub async fn request_clipboard(
        &self,
        req: &ClipboardRequest,
    ) -> Result<bool, InputError> {
        tracing::warn!(
            "clipboard transfer: {} [{}] → {} [{}] ({} bytes) — awaiting user confirmation",
            req.src_trust, req.src_domid,
            req.dst_trust, req.dst_domid,
            req.size_bytes,
        );

        // Sicherheitsregel: Red/Orange → Green/Black immer ablehnen ohne Dialog
        let src_level = trust_level_score(&req.src_trust);
        let dst_level = trust_level_score(&req.dst_trust);
        if src_level < dst_level {
            return Err(InputError::ClipboardDenied {
                reason: format!(
                    "trust escalation denied: {} → {}",
                    req.src_trust, req.dst_trust
                ),
            });
        }

        // Phase 11: Nutzer-Dialog → true wenn bestätigt
        tracing::warn!("clipboard dialog: Phase 11 stub — auto-deny for safety");
        Ok(false)
    }
}

impl Default for InputRouter {
    fn default() -> Self { Self::new() }
}

fn trust_level_score(level: &str) -> u8 {
    match level {
        "black"  => 4,
        "green"  => 3,
        "yellow" => 2,
        "orange" => 1,
        _        => 0, // red + unknown
    }
}
