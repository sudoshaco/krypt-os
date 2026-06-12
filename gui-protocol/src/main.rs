// krypt-gui-protocol — Wayland-nativer GUI-Daemon für Krypt OS
//
// Läuft in sys-gui. Empfängt Pixel-Daten von AppVMs via Xen Shared Memory
// und erstellt Wayland-Surfaces (xdg_toplevel) im Hyprland-Compositor.
//
// Thread-Modell:
//   Main-Thread (tokio): SIGTERM/SIGINT warten, Shutdown-Flag setzen.
//   Wayland-Thread (blocking std::thread): besitzt Compositor + EventQueue (!Send),
//     führt den 60fps-Frame-Loop aus.
//
// Phase 10: wl_shm Pixel-Pipeline, XenInterface::poll_dirty_regions() Stub.
// Phase 11: echte Xen-Guests via accept_guest(), wl_frame_callback Pacing.

mod input;
mod wayland;
mod xen;

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, Instant};
use std::error::Error;
use tracing::error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("KRYPT_LOG")
                .unwrap_or_else(|_| "krypt_gui=info".into()),
        )
        .init();

    tracing::info!("krypt-gui-protocol starting");

    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_wl = Arc::clone(&shutdown);

    // Wayland-Thread — besitzt Compositor (EventQueue ist !Send)
    let wayland_thread = std::thread::spawn(move || {
        if let Err(e) = wayland_loop(shutdown_wl) {
            error!("wayland loop: {e}");
        }
    });

    // Auf SIGTERM oder Ctrl+C warten
    let mut sigterm = tokio::signal::unix::signal(
        tokio::signal::unix::SignalKind::terminate(),
    )?;
    tokio::select! {
        _ = sigterm.recv()          => tracing::info!("SIGTERM — shutting down"),
        _ = tokio::signal::ctrl_c() => tracing::info!("SIGINT — shutting down"),
    }

    shutdown.store(true, Ordering::Release);
    wayland_thread.join().ok();

    tracing::info!("krypt-gui-protocol stopped");
    Ok(())
}

// ---------------------------------------------------------------------------
// Wayland-Thread
// ---------------------------------------------------------------------------

/// Stub-Liste der AppVMs die beim Start simuliert werden.
///
/// Phase 11: durch `xen.accept_guest()` ersetzen.
const STUB_VMS: &[(&str, wayland::TrustLevel, u32, u32, &str)] = &[
    ("work",    wayland::TrustLevel::Green,  1280, 800, "Terminal"),
    ("browser", wayland::TrustLevel::Yellow, 1280, 800, "Firefox"),
    ("vault",   wayland::TrustLevel::Black,  960,  600, "KeePassXC"),
];

fn wayland_loop(shutdown: Arc<AtomicBool>) -> Result<(), Box<dyn Error>> {
    let xen = xen::XenInterface::open()?;

    let mut compositor = wayland::Compositor::connect()?;
    tracing::info!("wayland: compositor connected");

    // Surfaces für simulierte AppVMs erstellen
    let mut surfaces: Vec<(xen::DomId, wayland::AppVmSurface)> = Vec::new();
    for (i, (name, trust, w, h, title)) in STUB_VMS.iter().enumerate() {
        let cfg = wayland::SurfaceConfig {
            vm_name: name.to_string(),
            trust:   *trust,
            width:   *w,
            height:  *h,
            title:   title.to_string(),
        };
        match compositor.create_surface(cfg) {
            Ok(surf)  => {
                tracing::info!("wayland: surface '{}' created", name);
                surfaces.push((i as xen::DomId, surf));
            }
            Err(e) => tracing::warn!("wayland: create_surface '{}': {e}", name),
        }
    }

    tracing::info!("wayland: {} surface(s) active — entering frame loop", surfaces.len());

    while !shutdown.load(Ordering::Acquire) {
        let frame_start = Instant::now();

        // Pixel-Updates für jede Surface
        for (domid, surface) in &mut surfaces {
            if !surface.is_configured() { continue; }

            let dirty = xen.poll_dirty_regions(*domid);
            if dirty.is_empty() { continue; }

            // Phase 10: trust-farbige Testfläche rendern
            // Phase 11: xen.read_pixels(*domid, &dirty)
            let pixels = trust_colored_frame(
                surface.config.width,
                surface.config.height,
                &surface.config.trust,
            );
            if let Err(e) = surface.update_pixels(&pixels) {
                tracing::warn!("update_pixels '{}': {e}", surface.config.vm_name);
            }
        }

        // Flush + Wayland-Events verarbeiten (XdgWmBase-Ping, configure, release)
        compositor.dispatch()?;

        // Frame-Pacing: 16ms Budget — verbleibende Zeit schlafen
        let elapsed = frame_start.elapsed();
        let budget  = Duration::from_millis(16);
        if elapsed < budget {
            std::thread::sleep(budget - elapsed);
        } else {
            tracing::trace!("frame overrun: {}ms", elapsed.as_millis());
        }
    }

    tracing::info!("wayland loop: clean exit");
    Ok(())
}

// ---------------------------------------------------------------------------
// Test-Pixel-Generator (Phase 10 stub)
// ---------------------------------------------------------------------------

/// Erzeugt einen einfarbigen ARGB8888-LE-Buffer in der Trust-Level-Farbe.
///
/// Speicher-Layout: je Pixel [B, G, R, A] (little-endian 0xAARRGGBB).
/// Phase 11: ersetzen durch echte Xen Grant-Memory-Daten.
fn trust_colored_frame(width: u32, height: u32, trust: &wayland::TrustLevel) -> Vec<u8> {
    // Catppuccin Mocha Farben (RGB)
    let (r, g, b): (u8, u8, u8) = match trust {
        wayland::TrustLevel::Red    => (0xf3, 0x8b, 0xa8),  // MOCHA_RED   #f38ba8
        wayland::TrustLevel::Orange => (0xfa, 0xb3, 0x87),  // MOCHA_PEACH #fab387
        wayland::TrustLevel::Yellow => (0xf9, 0xe2, 0xaf),  // MOCHA_YELLOW
        wayland::TrustLevel::Green  => (0xa6, 0xe3, 0xa1),  // MOCHA_GREEN
        wayland::TrustLevel::Black  => (0x11, 0x11, 0x1b),  // CRUST
    };

    // width/height kommen aus untrusted Guest-Metadaten — checked_mul
    // verhindert dass count*4 zu Heap-OOM oder Wraparound führt.
    // Bei Overflow oder absurder Größe (>256 MiB) → leerer Buffer,
    // ShmBuf::new würde eh fehlschlagen.
    const MAX_BYTES: usize = 256 * 1024 * 1024;
    let count_bytes = match width
        .checked_mul(height)
        .and_then(|c| c.checked_mul(4))
        .map(|c| c as usize)
    {
        Some(n) if n <= MAX_BYTES => n,
        _ => {
            tracing::warn!(
                "trust_colored_frame: oversized/overflow {}x{} — leeren Buffer zurück",
                width, height
            );
            return Vec::new();
        }
    };

    let mut buf = Vec::with_capacity(count_bytes);
    for _ in 0..(count_bytes / 4) {
        buf.extend_from_slice(&[b, g, r, 0xff]);  // BGRA → ARGB8888 LE
    }
    buf
}

#[cfg(test)]
mod tests {
    use super::{trust_colored_frame, wayland::TrustLevel};

    #[test]
    fn produces_4_bytes_per_pixel() {
        let buf = trust_colored_frame(4, 3, &TrustLevel::Green);
        assert_eq!(buf.len(), 4 * 3 * 4);
    }

    #[test]
    fn bgra_layout_for_mocha_red() {
        // MOCHA_RED = #f38ba8 → in BGRA LE: B=0xa8, G=0x8b, R=0xf3, A=0xff
        let buf = trust_colored_frame(1, 1, &TrustLevel::Red);
        assert_eq!(buf, vec![0xa8, 0x8b, 0xf3, 0xff]);
    }

    #[test]
    fn bgra_layout_for_mocha_peach() {
        // MOCHA_PEACH = #fab387 → B=0x87, G=0xb3, R=0xfa, A=0xff
        // Regressionsschutz: hier hatte trust_colored_frame über lange Zeit
        // versehentlich #febd96 zurückgegeben (anderer Orange-Ton als der
        // Border-Farbe von Hyprland). Drift war optisch sichtbar, aber im
        // Code unschuldig.
        let buf = trust_colored_frame(1, 1, &TrustLevel::Orange);
        assert_eq!(buf, vec![0x87, 0xb3, 0xfa, 0xff]);
    }

    #[test]
    fn oversized_returns_empty() {
        // width * height * 4 > 256 MiB → leerer Buffer (statt OOM)
        let buf = trust_colored_frame(20_000, 20_000, &TrustLevel::Green);
        assert!(buf.is_empty());
    }

    #[test]
    fn overflow_returns_empty() {
        // checked_mul Overflow → leerer Buffer (statt Panic)
        let buf = trust_colored_frame(u32::MAX, u32::MAX, &TrustLevel::Black);
        assert!(buf.is_empty());
    }

    #[test]
    fn black_is_crust_not_pure_black() {
        // Vault-Farbe MOCHA_CRUST = #11111b — NICHT #000000. Es kam in
        // einem CSS-Refactor mal vor dass jemand #000000 hineingeschrieben
        // hätte; dieser Test fixiert die Catppuccin-Crust-Wahl im Code.
        let buf = trust_colored_frame(1, 1, &TrustLevel::Black);
        assert_eq!(buf, vec![0x1b, 0x11, 0x11, 0xff]);
    }
}
