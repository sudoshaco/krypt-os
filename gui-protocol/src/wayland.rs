// wayland.rs — Wayland-Surface-Management für AppVM-Fenster (Phase 10)
//
// Transport: xdg_toplevel — Hyprland windowrulev2 matcht Title + app_id (ADR-011).
// Pixel-Buffer: wl_shm_pool über anonyme Datei in /dev/shm.
//
// Framing: attach → damage_buffer → commit → roundtrip (flush + Events).
// Phase 11: wl_frame_callback für compositor-gesteuertes Pacing (kein Sleep).
//
// EventQueue<WaylandState> ist !Send — Compositor muss auf einem Thread bleiben.
#![allow(dead_code)]

use std::io::{Seek, SeekFrom, Write};
use std::os::unix::io::AsFd;
use std::sync::{
    atomic::{AtomicBool, AtomicU32, Ordering},
    Arc,
};
use thiserror::Error;
use wayland_client::{
    delegate_noop,
    protocol::{
        wl_buffer::{self, WlBuffer},
        wl_compositor::WlCompositor,
        wl_registry::{self, WlRegistry},
        wl_shm::{self, WlShm},
        wl_shm_pool::WlShmPool,
        wl_surface::{self, WlSurface},
    },
    Connection, Dispatch, EventQueue, QueueHandle,
};
use wayland_protocols::xdg::shell::client::{
    xdg_surface::{self, XdgSurface},
    xdg_toplevel::{self, XdgToplevel},
    xdg_wm_base::{self, XdgWmBase},
};

#[derive(Debug, Error)]
pub enum WaylandError {
    #[error("compositor connection failed: {0}")]
    Connect(String),
    #[error("global not advertised by compositor: {0}")]
    MissingGlobal(&'static str),
    #[error("surface operation failed: {0}")]
    Surface(String),
    #[error("shm buffer allocation failed: {0}")]
    Buffer(String),
}

impl From<std::io::Error> for WaylandError {
    fn from(e: std::io::Error) -> Self {
        WaylandError::Buffer(e.to_string())
    }
}

/// Trust-Level eines AppVM-Fensters — bestimmt Border-Farbe in Hyprland.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustLevel {
    Red,
    Orange,
    Yellow,
    Green,
    Black,
}

impl TrustLevel {
    pub fn as_tag(&self) -> &'static str {
        match self {
            TrustLevel::Red    => "red",
            TrustLevel::Orange => "orange",
            TrustLevel::Yellow => "yellow",
            TrustLevel::Green  => "green",
            TrustLevel::Black  => "black",
        }
    }
}

#[derive(Debug, Clone)]
pub struct SurfaceConfig {
    pub vm_name: String,
    pub trust:   TrustLevel,
    pub width:   u32,
    pub height:  u32,
    pub title:   String,
}

impl SurfaceConfig {
    /// "[<trust>] <vm-name>: <title>" — Hyprland windowrulev2 matcht dieses Format.
    pub fn krypt_title(&self) -> String {
        format!("[{}] {}: {}", self.trust.as_tag(), self.vm_name, self.title)
    }
}

// ---------------------------------------------------------------------------
// Shared-Memory-Buffer (wl_shm_pool + wl_buffer + anonyme Datei)
// ---------------------------------------------------------------------------

static SHM_COUNTER: AtomicU32 = AtomicU32::new(0);

/// Anonyme Datei in /dev/shm — kein tmpfs-Name nach dem open() (unlink sofort).
fn create_shm_file(size: usize) -> Result<std::fs::File, WaylandError> {
    let id  = SHM_COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let path = format!("/dev/shm/krypt-gui-{}-{}", pid, id);

    let file = std::fs::OpenOptions::new()
        .read(true).write(true).create(true).truncate(false)
        .open(&path)?;
    std::fs::remove_file(&path).ok();
    file.set_len(size as u64)?;
    Ok(file)
}

/// Hält den wl_shm_pool + wl_buffer für eine Surface.
struct ShmBuf {
    file:     std::fs::File,
    pool:     WlShmPool,
    buffer:   WlBuffer,
    released: Arc<AtomicBool>,
    stride:   u32,
    size:     usize,
}

impl ShmBuf {
    fn new(
        shm:    &WlShm,
        width:  u32,
        height: u32,
        qh:     &QueueHandle<WaylandState>,
    ) -> Result<Self, WaylandError> {
        // width/height kommen aus GuestMetadata einer AppVM — untrusted.
        // Ohne checked_mul könnte width=0xFFFF_FFFF einen winzigen Buffer
        // erzeugen und nachfolgende Pixel-Schreibvorgänge zu Heap-Overflow
        // führen. wayland-spec verlangt zudem dass size in i32 passt.
        let stride: u32 = width
            .checked_mul(4)
            .ok_or_else(|| WaylandError::Buffer(format!("stride overflow: width={width}")))?;
        let size_u32: u32 = stride
            .checked_mul(height)
            .ok_or_else(|| WaylandError::Buffer(format!("size overflow: {width}x{height}")))?;
        let size_i32: i32 = size_u32
            .try_into()
            .map_err(|_| WaylandError::Buffer(format!("size > i32::MAX: {size_u32}")))?;
        let stride_i32: i32 = stride
            .try_into()
            .map_err(|_| WaylandError::Buffer(format!("stride > i32::MAX: {stride}")))?;
        let width_i32: i32 = width
            .try_into()
            .map_err(|_| WaylandError::Buffer(format!("width > i32::MAX: {width}")))?;
        let height_i32: i32 = height
            .try_into()
            .map_err(|_| WaylandError::Buffer(format!("height > i32::MAX: {height}")))?;

        let size = size_u32 as usize;
        let file = create_shm_file(size)?;

        let released = Arc::new(AtomicBool::new(true));
        let pool = shm.create_pool(file.as_fd(), size_i32, qh, ());
        let buffer = pool.create_buffer(
            0,
            width_i32,
            height_i32,
            stride_i32,
            wl_shm::Format::Argb8888,
            qh,
            Arc::clone(&released),
        );

        Ok(Self { file, pool, buffer, released, stride, size })
    }

    /// Schreibt Pixel-Daten in die Shared-Memory-Datei.
    fn write_pixels(&mut self, data: &[u8]) -> Result<(), WaylandError> {
        self.file.seek(SeekFrom::Start(0))?;
        let len = self.size.min(data.len());
        self.file.write_all(&data[..len])?;
        Ok(())
    }

    /// True wenn der Compositor den Buffer freigegeben hat (WlBuffer::Release).
    fn is_released(&self) -> bool {
        self.released.load(Ordering::Acquire)
    }

    fn mark_in_flight(&self) {
        self.released.store(false, Ordering::Release);
    }
}

// ---------------------------------------------------------------------------
// Interne Wayland-Dispatch-State
// ---------------------------------------------------------------------------

struct WaylandState {
    compositor:  Option<WlCompositor>,
    xdg_wm_base: Option<XdgWmBase>,
    shm:         Option<WlShm>,
}

impl WaylandState {
    fn new() -> Self {
        Self { compositor: None, xdg_wm_base: None, shm: None }
    }
}

impl Dispatch<WlRegistry, ()> for WaylandState {
    fn event(
        state: &mut Self,
        registry: &WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        let wl_registry::Event::Global { name, interface, version } = event else { return };
        match interface.as_str() {
            "wl_compositor" => {
                state.compositor = Some(registry.bind(name, version.min(4), qh, ()));
            }
            "xdg_wm_base" => {
                state.xdg_wm_base = Some(registry.bind(name, version.min(2), qh, ()));
            }
            "wl_shm" => {
                state.shm = Some(registry.bind(name, version.min(1), qh, ()));
            }
            _ => {}
        }
    }
}

// wl_compositor — keine Events
delegate_noop!(WaylandState: WlCompositor);

// xdg_wm_base: Ping muss mit Pong beantwortet werden (sonst trennt Compositor)
impl Dispatch<XdgWmBase, ()> for WaylandState {
    fn event(
        _: &mut Self,
        xdg: &XdgWmBase,
        event: xdg_wm_base::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_wm_base::Event::Ping { serial } = event {
            xdg.pong(serial);
        }
    }
}

// wl_surface: enter/leave (Output-Zuordnung ignoriert)
impl Dispatch<WlSurface, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &WlSurface,
        _: wl_surface::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

// xdg_surface: configure-Event + ack_configure (Pflicht laut xdg-shell-Protokoll)
impl Dispatch<XdgSurface, Arc<AtomicBool>> for WaylandState {
    fn event(
        _: &mut Self,
        xdg_surf: &XdgSurface,
        event: xdg_surface::Event,
        configured: &Arc<AtomicBool>,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_surface::Event::Configure { serial } = event {
            xdg_surf.ack_configure(serial);
            configured.store(true, Ordering::Release);
        }
    }
}

// xdg_toplevel: configure/close (Phase 11: echtes Resize + graceful close)
impl Dispatch<XdgToplevel, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &XdgToplevel,
        _: xdg_toplevel::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

// wl_shm: Format-Events (welche Pixel-Formate unterstützt sind) — ARGB8888 immer dabei
impl Dispatch<WlShm, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &WlShm,
        _: wl_shm::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

// wl_shm_pool: keine Events
delegate_noop!(WaylandState: WlShmPool);

// wl_buffer: Release-Event → Buffer wieder verfügbar
impl Dispatch<WlBuffer, Arc<AtomicBool>> for WaylandState {
    fn event(
        _: &mut Self,
        _: &WlBuffer,
        event: wl_buffer::Event,
        released: &Arc<AtomicBool>,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_buffer::Event::Release = event {
            released.store(true, Ordering::Release);
        }
    }
}

// ---------------------------------------------------------------------------
// Öffentliche API
// ---------------------------------------------------------------------------

/// Eine aktive Wayland-Surface für ein AppVM-Fenster.
///
/// Phase 9:  wl_surface + xdg_surface + xdg_toplevel mit Krypt-Titel.
/// Phase 10: wl_shm_pool + wl_buffer — Pixel-Updates via attach/damage/commit.
pub struct AppVmSurface {
    pub config:  SurfaceConfig,
    surface:     WlSurface,
    xdg_surface: XdgSurface,
    toplevel:    XdgToplevel,
    configured:  Arc<AtomicBool>,
    shm_buf:     Option<ShmBuf>,
}

impl AppVmSurface {
    /// Rendert neuen Pixel-Frame.
    ///
    /// Wartet nicht auf Buffer-Release — überschreibt direkt (unkritisch bei 60fps
    /// da der Compositor schnell genug releast). Phase 11: echter Frame-Callback.
    ///
    /// `data`: ARGB8888 LE (je Pixel: [B, G, R, A]), Row-Major, width*height*4 Bytes.
    pub fn update_pixels(&mut self, data: &[u8]) -> Result<(), WaylandError> {
        let buf = match self.shm_buf.as_mut() {
            Some(b) => b,
            None => return Ok(()),
        };

        buf.write_pixels(data)?;
        buf.mark_in_flight();

        self.surface.attach(Some(&buf.buffer), 0, 0);
        self.surface.damage_buffer(
            0, 0,
            self.config.width  as i32,
            self.config.height as i32,
        );
        self.surface.commit();
        Ok(())
    }

    fn resize_internal(
        &mut self,
        width:  u32,
        height: u32,
        shm:    &WlShm,
        qh:     &QueueHandle<WaylandState>,
    ) -> Result<(), WaylandError> {
        self.shm_buf = None;
        self.config.width  = width;
        self.config.height = height;
        if width > 0 && height > 0 {
            self.shm_buf = Some(ShmBuf::new(shm, width, height, qh)?);
        }
        Ok(())
    }

    /// True sobald der Compositor die Surface konfiguriert hat.
    pub fn is_configured(&self) -> bool {
        self.configured.load(Ordering::Acquire)
    }

    /// True wenn der laufende Frame vom Compositor freigegeben wurde.
    pub fn buffer_released(&self) -> bool {
        self.shm_buf.as_ref().is_none_or(|b| b.is_released())
    }
}

/// Verbindung zum Wayland-Compositor (sys-gui Hyprland).
///
/// Nicht Send — EventQueue<WaylandState> ist an einen Thread gebunden.
/// Starte einen dedizierten Thread: `std::thread::spawn(|| Compositor::connect_and_run(...))`.
pub struct Compositor {
    conn:  Connection,
    state: WaylandState,
    queue: EventQueue<WaylandState>,
    qh:    QueueHandle<WaylandState>,
}

impl Compositor {
    /// Verbindet mit dem Wayland-Compositor ($WAYLAND_DISPLAY).
    pub fn connect() -> Result<Self, WaylandError> {
        let conn = Connection::connect_to_env()
            .map_err(|e| WaylandError::Connect(e.to_string()))?;

        let mut queue = conn.new_event_queue::<WaylandState>();
        let qh      = queue.handle();
        let display = conn.display();

        let mut state = WaylandState::new();
        display.get_registry(&qh, ());
        queue
            .roundtrip(&mut state)
            .map_err(|e| WaylandError::Connect(e.to_string()))?;

        if state.compositor.is_none() {
            return Err(WaylandError::MissingGlobal("wl_compositor"));
        }
        if state.xdg_wm_base.is_none() {
            return Err(WaylandError::MissingGlobal("xdg_wm_base"));
        }
        if state.shm.is_none() {
            return Err(WaylandError::MissingGlobal("wl_shm"));
        }

        tracing::info!("wayland: connected (compositor + xdg_wm_base + wl_shm ready)");
        Ok(Self { conn, state, queue, qh })
    }

    /// Erstellt eine Surface + wl_shm_pool für eine AppVM.
    pub fn create_surface(&mut self, config: SurfaceConfig) -> Result<AppVmSurface, WaylandError> {
        // Clone proxies — Wayland-Proxies sind Arc-backed, Clone ist O(1).
        // Borrows auf self.state enden sofort → roundtrip kann &mut self.state nehmen.
        let compositor  = self.state.compositor.clone()
            .ok_or(WaylandError::MissingGlobal("wl_compositor"))?;
        let xdg_wm_base = self.state.xdg_wm_base.clone()
            .ok_or(WaylandError::MissingGlobal("xdg_wm_base"))?;
        let shm = self.state.shm.clone()
            .ok_or(WaylandError::MissingGlobal("wl_shm"))?;

        let surface    = compositor.create_surface(&self.qh, ());
        let configured = Arc::new(AtomicBool::new(false));

        let xdg_surface = xdg_wm_base.get_xdg_surface(
            &surface, &self.qh, Arc::clone(&configured),
        );
        let toplevel = xdg_surface.get_toplevel(&self.qh, ());

        toplevel.set_title(config.krypt_title());
        toplevel.set_app_id(format!("krypt.{}", config.vm_name));

        // Initiales commit → xdg_surface-Rolle gesetzt → Compositor sendet configure
        surface.commit();
        self.queue
            .roundtrip(&mut self.state)
            .map_err(|e| WaylandError::Surface(e.to_string()))?;

        // wl_shm_pool + wl_buffer anlegen (nach configure, damit Größe bekannt)
        let shm_buf = if config.width > 0 && config.height > 0 {
            Some(ShmBuf::new(&shm, config.width, config.height, &self.qh)?)
        } else {
            None
        };

        tracing::debug!(
            "wayland: surface '{}' [{}] {}x{} + shm_buf={}",
            config.vm_name, config.trust.as_tag(), config.width, config.height,
            shm_buf.is_some(),
        );

        Ok(AppVmSurface { config, surface, xdg_surface, toplevel, configured, shm_buf })
    }

    /// Flush + ausstehende Events verarbeiten (Frame-Loop-Tick).
    pub fn dispatch(&mut self) -> Result<(), WaylandError> {
        self.queue
            .roundtrip(&mut self.state)
            .map_err(|e| WaylandError::Surface(e.to_string()))?;
        Ok(())
    }

    /// Ändert die Fenstergröße einer Surface — dealloziiert + re-alloziert wl_shm_pool.
    pub fn resize_surface(
        &mut self,
        surface: &mut AppVmSurface,
        width:   u32,
        height:  u32,
    ) -> Result<(), WaylandError> {
        let shm = self.state.shm.clone()
            .ok_or(WaylandError::MissingGlobal("wl_shm"))?;
        surface.resize_internal(width, height, &shm, &self.qh)
    }
}
