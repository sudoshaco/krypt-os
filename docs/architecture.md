# Krypt OS — Architektur

## Vision

Drei Säulen, keine Kompromisse:

```
KRYPTOGRAFIE    Crypto-first by design. Kein Plaintext zwischen Komponenten.
ISOLATION       Xen-Hypervisor. Jede Anwendung in eigener VM.
LIGHTWEIGHT     Alpine-Templates. Memory Ballooning. 8 GB RAM Ziel.
```

---

## Schichtenmodell

```
AppVMs (Alpine-Templates)
  work │ browser │ hacking │ personal │ vault
──────────────────────────────────────────────
sys-gui: Arch + Hyprland + krypt-gui-protocol
sys-net │ sys-firewall │ sys-usb
──────────────────────────────────────────────
dom0: Arch minimal + krypt-daemon (Rust)
──────────────────────────────────────────────
Xen Hypervisor (Typ-1)
──────────────────────────────────────────────
Hardware: VT-d/AMD-Vi · TPM2 · Secure Boot
```

---

## krypt-daemon (Rust)

Herzstück von Krypt. Ersetzt qubesd.

**v0.1.0-alpha Status** — implementiert:

```
vm-daemon/src/             (Crate-Name: krypt-daemon, Binary: /usr/bin/krypt-daemon)
├── policy.rs     Trust-Level-System, Kommunikationsregeln (PolicyEngine)
├── vm.rs         VM-Lifecycle (start/shutdown/destroy via xl)
├── ipc.rs        JSON-over-Unix-Domain-Socket (/run/krypt/ipc.sock)
├── config.rs     /etc/krypt/daemon.toml Parser inkl. validate()
├── usb.rs        tokio-udev USB-Monitor + AuthStickRemoved-Event
└── main.rs       Event-Loop, trigger_panic
```

Geplant (nicht in v0.1.0-alpha):
  - `tpm.rs` — TPM2 Key-Sealing (siehe ADR-013)
  - `crypto.rs` — ChaCha20-Poly1305 für Xen vchan Inter-VM-Channel
  - Phase 11+ siehe `docs/known-issues.md` und `PROGRESS.md`

---

## krypt-gui-protocol (C + Rust)

Ersetzt qubes-gui-daemon. Wayland-native.

```
AppVM rendert Fenster
  └── krypt-gui-agent (in AppVM)
        │ Xen Shared Memory (Pixel-Buffer)
        ▼
  krypt-gui-daemon (in sys-gui)
        └── Hyprland (wl_surface pro AppVM-Fenster)
              └── Border-Farbe = Trust-Level der Quell-VM
```

---

## AppVM-Templates (Alpine-Basis)

Warum Alpine statt Fedora/Debian:
- musl libc: kleinerer Footprint als glibc
- Base-Image: ~10 MB (Fedora: ~500 MB)
- Reduzierte Angriffsfläche

```
krypt-base-template (Alpine)
├── krypt-work-template      LibreOffice, Thunderbird
├── krypt-browser-template   Firefox, DispVM-fähig
├── krypt-hacking-template   Security-Tools
├── krypt-personal-template
└── krypt-vault-template     KeePassXC, GPG — kein Netz
```

---

## Lightweight: Memory Ballooning

Xen dynamische RAM-Zuweisung statt statischer Reservierung.

```
8 GB RAM Beispiel:
  dom0:          512 MB (statisch)
  sys-net:       128 MB
  sys-firewall:  128 MB
  sys-usb:       128 MB
  sys-gui:       1.024 MB
  AppVM aktiv:   512–2.048 MB (dynamisch)
  AppVM idle:    64 MB (Balloon)
```

Ziel: 50% weniger RAM-Bedarf als QubesOS durch Ballooning + Alpine.

---

## Kryptografie-Layer

### Prinzip: Zero Plaintext
Kein Secret verlässt seine VM unverschlüsselt.

### Inter-VM-Kommunikation
Xen vchan + ChaCha20-Poly1305. Key-Exchange via X25519 beim VM-Start.

### TPM2-Key-Sealing
LUKS2-Key versiegelt gegen PCR-Werte. Boot schlägt fehl bei:
- Verändertem Bootloader (PCR 4)
- Verändertem Kernel (PCR 8)
- Verändertem dom0 (PCR 11)

### Clipboard-Protokoll
```
Ctrl+Shift+C in AppVM-A
  → Popup: "Transfer zu AppVM-B erlauben?"
  → Bei Ja: AES-256-GCM über krypt-daemon
  → Bei Nein: nichts
```

### Hardware-Keys
YubiKey, FIDO2, OpenPGP-Smartcards — via sys-usb (Isolation bleibt erhalten).

---

## Trust-Level

```
black  (4)  Vault — Kein Netz. Crypto-Keys, Passwörter.        [violett]
green  (3)  Trusted — Work, Persönlich.                         [grün]
yellow (2)  Medium — Unbekannte Software, Tests.                [gelb]
orange (1)  Low-trust — Social, Streaming.                      [orange]
red    (0)  Untrusted — Browser, Downloads.                     [rot]
```

Niedrigeres Trust-Level kann niemals höheres kontaktieren (Policy Engine).
dom0 hat keinen Internetzugang — niemals.

---

## Sicherheitskritische Entwicklungsentscheidungen

- Rust für krypt-daemon: Memory-Safety ohne GC, kein Buffer-Overflow möglich
- Kein unsafe-Code außerhalb von xen.rs und wayland.rs (FFI-Grenzen)
- clippy -D warnings: Lint-Fehler brechen den Build
- cargo audit: Dependency-Vulnerabilities werden geprüft
- Phase 3 (krypt-gui-protocol) ist kritischer Pfad — X11-Bridge als Fallback geplant
