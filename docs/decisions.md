# Architecture Decision Records — Krypt OS

Technische Entscheidungen werden hier dokumentiert damit zukünftige
Sessions (und Contributor) verstehen warum Dinge so sind wie sie sind.

---

## ADR-001: Xen statt KVM als Hypervisor
**Datum**: 2025-05-13
**Status**: Accepted

### Kontext
Krypt OS braucht echte VM-Isolation. Es gab zwei realistische Optionen: Xen (Typ-1)
oder KVM (Typ-2, Linux-integriert).

### Entscheidung
Xen Hypervisor (Typ-1).

### Begründung
KVM läuft im Linux-Kernel — ein Kernel-Exploit kompromittiert alle VMs.
Xen läuft direkt auf der Hardware. dom0 ist eine privilegierte VM aber kein
Host-OS im klassischen Sinne. QubesOS hat diese Architektur über Jahre bewiesen.
Für ein Security-first-OS gibt es keine Alternative.

### Konsequenzen
- Kein QEMU/KVM Tooling nutzbar
- Xen-APIs müssen in krypt-daemon implementiert werden
- Nvidia-GPU in dom0 ist problematisch (AMD bevorzugt)
- Build und Test sind komplexer

---

## ADR-002: Rust für krypt-daemon
**Datum**: 2025-05-13
**Status**: Accepted

### Kontext
Der VM-Daemon läuft in dom0 und hat Xen-Verwaltungsrechte über alle VMs.
Ein Memory-Safety-Bug ist ein kritisches Sicherheitsproblem.

### Entscheidung
Rust als primäre Sprache für krypt-daemon.

### Begründung
Memory-Safety ohne Garbage Collector. Ein Use-after-free oder Buffer-Overflow
im VM-Daemon könnte alle VM-Isolation kompromittieren. Rust macht diese
Klasse von Bugs unmöglich. Der Performanz-Overhead von GC (Go, Java) ist
in einem System-Daemon inakzeptabel.

### Konsequenzen
- Xen-C-Libraries brauchen FFI-Bindings (unsafe-Code begrenzt auf xen.rs)
- Lernkurve für Borrow-Checker in komplexen asynchronen Szenarien
- Kein crates.io-Crate für libxenvchan → eigene Bindings nötig

---

## ADR-003: Öffentliches GitHub-Repository
**Datum**: 2025-05-13
**Status**: Accepted

### Kontext
Krypt OS ist ein Sicherheitsprojekt. Die Frage: öffentlich oder privat?

### Entscheidung
Öffentliches Repository auf GitHub (github.com/sudoshaco/krypt-os).

### Begründung
Security through obscurity ist kein Sicherheitsmodell. QubesOS, Linux, Xen,
alle sicherheitskritischen Fundamente von Krypt sind open source.
Krypt's Sicherheit muss durch das Design beweisbar sein, nicht durch
Geheimhaltung des Codes. Zusätzlich: öffentliche Sichtbarkeit schafft
Accountability und macht externe Code-Reviews möglich.

Keine Secrets, Infrastruktur-Details oder Exploit-Code kommen ins Repo.

### Konsequenzen
- Jeder sieht den Code — kein Security-by-obscurity möglich
- Externe Contributors können beitragen
- Frühes Scheitern ist öffentlich sichtbar (akzeptabel)

---

## ADR-004: Arch Linux als Basis
**Datum**: 2025-05-13
**Status**: Accepted

### Kontext
Welche Linux-Distribution als Basis für dom0 und AppVM-Templates?

### Entscheidung
Arch Linux.

### Begründung
Rolling-Release bedeutet immer aktuelle Pakete ohne manuelle Dist-Upgrades.
Minimaler Footprint — pacman installiert genau was wir wollen, nichts mehr.
AUR gibt Zugang zu fast allem. Omarchy ist Arch-basiert, die UX-Ziele
sind darauf ausgerichtet. Alternativen (Fedora wie QubesOS, Debian) bringen
mehr Ballast und langsamere Updates.

### Konsequenzen
- Kein offizieller Xen-Support für Arch (QubesOS nutzt Fedora für dom0)
- Arch-Xen-Packages aus AUR oder eigenem Build
- Häufigere Abhängigkeits-Updates nötig

---

## Template für neue ADRs

```markdown
## ADR-[N]: [Titel]
**Datum**: [YYYY-MM-DD]
**Status**: Proposed | Accepted | Deprecated | Superseded by ADR-[N]

### Kontext
Warum mussten wir diese Entscheidung treffen?

### Entscheidung
Was haben wir entschieden?

### Begründung
Warum diese Option und nicht die Alternativen?

### Konsequenzen
Was wird dadurch schwieriger / einfacher?
```

---

## ADR-005: Alpine Linux für AppVM-Templates
**Datum**: 2025-05-13
**Status**: Accepted

### Kontext
QubesOS nutzt Fedora und Debian als Template-Basis. Beide sind schwergewichtig.
Für das Lightweight-Ziel (8 GB RAM) brauchen wir leichtere Templates.

### Entscheidung
Alpine Linux als primäre Template-Basis für alle AppVMs.

### Begründung
Alpine nutzt musl libc statt glibc und BusyBox statt GNU Coreutils.
Base-Image ~10 MB vs. ~500 MB bei Fedora. Kleinere Angriffsfläche durch
weniger installierte Pakete. Perfekt für VMs die nur einen Zweck erfüllen.
Einschränkung: nicht alle Software läuft auf musl ohne Patches —
krypt-hacking-template bleibt Arch-basiert wegen Tool-Kompatibilität.

### Konsequenzen
- Einige Tools brauchen musl-kompatible Builds
- krypt-hacking-template Ausnahme: Arch-basiert
- Deutlich geringerer RAM-Footprint pro VM

---

## ADR-006: ChaCha20-Poly1305 für Inter-VM-Verschlüsselung
**Datum**: 2025-05-13
**Status**: Accepted

### Kontext
Xen vchan überträgt Daten zwischen VMs ohne Verschlüsselung.
Krypt-Prinzip: kein Plaintext zwischen Komponenten.

### Entscheidung
ChaCha20-Poly1305 (AEAD) für alle Inter-VM-Kanäle via `ring` crate.
Key-Exchange via X25519 beim VM-Start.

### Begründung
ChaCha20-Poly1305 ist schneller als AES-GCM ohne Hardware-AES-Beschleunigung
(ältere CPUs). Konstant-Zeit-Implementierung in `ring` verhindert Timing-Attacks.
X25519 ist state-of-the-art für ephemere Key-Exchange (forward secrecy).

### Konsequenzen
- Minimaler Performance-Overhead auf modernen CPUs
- Forward Secrecy: kompromittierter langfristiger Key enthüllt keine alten Sessions
- `ring` crate als einzige Crypto-Dependency (auditiert, keine OpenSSL-Abhängigkeit)

---

## ADR-007: Memory Ballooning für Lightweight-Ziel
**Datum**: 2025-05-13
**Status**: Accepted

### Kontext
QubesOS reserviert RAM statisch pro VM. Bei 5 laufenden VMs mit je 1 GB
sind 5 GB blockiert auch wenn VMs idle sind.

### Entscheidung
Xen Memory Ballooning aktivieren. VMs starten mit minimalem RAM (64 MB idle)
und bekommen dynamisch mehr bei Bedarf.

### Begründung
Ermöglicht mehr parallel laufende VMs bei gleichem physischem RAM.
Xen Ballooning ist ausgereift und seit Jahren in Produktivumgebungen erprobt.

### Konsequenzen
- RAM-Allokation reagiert verzögert bei plötzlichem Bedarf (~100ms)
- Balloon-Driver muss in alle Alpine-Templates integriert werden
- krypt-daemon monitort RAM-Nutzung und steuert Balloon-Policy

---

## ADR-008: USB-Stick als primärer Authentikator (Hardware Kill Switch)
**Datum**: 2025-05-13
**Status**: Accepted

### Kontext
Krypt braucht ein Authentifizierungskonzept das in Panik-Situationen
funktioniert — eine physische Aktion, keine kognitive.

### Entscheidung
Normaler USB-Stick mit verstecktem Schlüsselmaterial (`.krypt` Datei, 512 Byte)
als primärer Boot-Authentikator und Runtime-Kill-Switch.
Ziehen des Sticks triggert konfigurierbaren Panic-Level (Standard: NUKE).

### Begründung
Passwörter versagen unter Stress (vergessen, Schulter-Surfing, Zwang).
Ein physischer Stick ist: schnell (eine Handbewegung), intuitiv, eindeutig.
Der Stick sieht normal aus — kein "ich bin ein Security-Dongle" Aussehen.
LUKS2 Key-Slots erlauben beliebig viele Backup-Sticks mit individuellem Widerruf.

### Konsequenzen
- Kein Passwort-Fallback — bewusste Designentscheidung
- Verlorener Stick ohne Backup: System unzugänglich (Nutzer trägt Verantwortung)
- initramfs muss USB-Events verarbeiten können
- krypt-daemon braucht permanenten USB-Monitor Thread
- krypt-stick CLI für Backup-Verwaltung

---

## ADR-009: IPC-Protokoll — Unix-Domain-Socket + JSON-Framing
**Datum**: 2026-05-13
**Status**: Accepted

### Kontext
krypt-daemon muss mit AppVM-Agenten, dem Waybar-Modul und internen Tools
kommunizieren. Die Entscheidung: welches Transport- und Serialisierungsformat?

Kandidaten waren:
- **Xen vchan** (native Xen-Inter-VM-Kanal)
- **D-Bus** (systemd-Standard)
- **Unix-Domain-Socket + Protobuf / Cap'n Proto**
- **Unix-Domain-Socket + JSON**

### Entscheidung
Unix-Domain-Socket `/run/krypt/ipc.sock` (root:root, 0600) mit
4-Byte-Little-Endian-Längenprefix + UTF-8-JSON-Body. Max Frame: 64 KiB.
Nachrichtentyp via `"type"`-Feld als Serde-Tag-Discriminator.

### Begründung

**Warum nicht vchan:**
`cargo search xenvchan` ergab nur `0.0.0-pre` Stubs auf crates.io.
Stabile Rust-Bindings fehlen. libxenvchan direkt via FFI wäre unsafe-Code
der tief in Xen-Interna greift — zu viel Aufwand für Phase 1-7.
vchan bleibt die langfristige Option für Inter-VM-Kanäle (ADR-006),
aber für dom0-lokale Kommunikation ist es Overkill.

**Warum nicht D-Bus:**
D-Bus ist für Desktop-Daemons konzipiert, nicht für Security-kritische
System-Daemons. Abhängigkeit von dbus-daemon in dom0 widerspricht dem
Minimal-dom0-Prinzip. D-Bus bietet keine kontrolliertere Auth als
Unix-Socket-Permissions.

**Warum nicht Protobuf/Cap'n Proto:**
Schemaevolution mit `.proto`-Dateien und Codegen-Step wäre ein
unnötiger Build-Complexity-Overhead in Phase 1-7. JSON mit serde ist
schema-los erweiterbar und in Rust ohne externe Toolchain einsetzbar.

**Warum JSON:**
- Menschenlesbar: `socat - UNIX:/run/krypt/ipc.sock` für manuelles Debugging
- `serde_json` ist der meistauditierte Rust-Serializer
- `#[serde(tag = "type", rename_all = "snake_case")]` gibt selbstbeschreibende Nachrichten ohne Boilerplate
- Performance ist irrelevant: VM-Management-RPCs sind selten (< 100/s)

**Warum 4-Byte-Längenprefix:**
- Stream-Framing ohne Delimiter-Scanning (kein `\n`-Split der JSON splitten kann)
- LE weil x86-64 nativ LE ist — kein Byte-Swap nötig
- 64-KiB-Maximum schützt vor Memory-Exhaustion durch kompromittierte AppVM

### Konsequenzen
- Protokoll ist dom0-lokal: AppVM-Agenten brauchen einen separaten Kanal
  (Phase 8+: vchan-Bridge die lokale IPC-Nachrichten weiterleitet)
- JSON ist nicht zero-copy — bei großen VM-Listen (> 100 VMs) messbar
  langsamer als Protobuf (akzeptabel für Krypt-Ziel: < 20 VMs)
- Socket auf 0600 locked AppVM-Agenten aus (die in dom0 als User laufen müssten)
  → das ist gewollt: direkter Daemon-Zugriff nur für root/krypt-agent

---

## ADR-010: mkinitcpio statt dracut für initramfs
**Datum**: 2026-05-13
**Status**: Accepted

### Kontext
Krypt OS braucht einen eigenen initramfs-Hook für den USB-Stick-Auth-Flow.
Zwei verbreitete initramfs-Frameworks auf Linux: mkinitcpio (Arch-nativ)
und dracut (Red Hat / systemd-Ökosystem, inzwischen auch auf anderen Distros).

### Entscheidung
mkinitcpio als initramfs-Framework.

### Begründung

**Arch-Kanonisch:**
Krypt-dom0 ist Arch-basiert (ADR-004). mkinitcpio ist das Standard-Framework
auf Arch: offizielle Pakete, offizielle Hooks (`encrypt`, `lvm2`, `udev`),
pacman-alpm-Hooks für automatischen Rebuild nach Kernel-Update.
dracut existiert in den Arch-Repos, ist aber nicht der kanonische Pfad.

**Einfachheit:**
Unser Hook (`/etc/initcpio/hooks/krypt`) ist 80 Zeilen POSIX-sh.
mkinitcpio-Hooks haben eine klare Struktur: `build()` für den Initramfs-Bau,
`run_hook()` für den Boot. Kein Framework-Overhead, kein "module discovery magic".
Die `HOOKS=(base udev krypt encrypt filesystems)` Reihenfolge ist explizit
und deterministisch.

**Kontrolle über das initramfs:**
mkinitcpio gibt vollständige Kontrolle über was ins Image kommt (`add_binary`,
`add_module`, `add_file`). Für ein Security-OS ist ein minimales initramfs
mit explizit gelisteten Binaries besser als dracuts automatisches
Dependency-Pulling.

**Warum nicht dracut:**
Dracut hat bessere Modularität und systemd-Integration — sinnvoll wenn
das initramfs ein vollständiges systemd-früh-System sein soll.
Für Krypt ist das Gegenteil gewollt: minimales initramfs, kein systemd
in der frühen Boot-Phase, klare Kontrolle.

### Konsequenzen
- Bei Migration auf non-Arch-Basis (unwahrscheinlich) muss der Hook portiert werden
- mkinitcpio BusyBox-ash statt bash: Hook nutzt POSIX-kompatibles sh
  (`${param#prefix}`, `case`, keine Arrays) → portabel ✓
- `mkinitcpio -p linux` muss nach jedem Kernel-Update ausgeführt werden
  (pacman-alpm-Hook übernimmt das auf Arch automatisch)

---

## ADR-011: xdg_toplevel statt wl_subsurface / Custom-Protokoll für AppVM-Fenster
**Datum**: 2026-05-13
**Status**: Accepted

### Kontext
gui-protocol rendert AppVM-Pixel-Daten als Wayland-Surfaces in sys-gui's
Hyprland-Compositor. Drei Ansätze wurden evaluiert:

1. **wl_subsurface** — AppVM-Fenster als Subsurface unter einem Container-Window
2. **xdg_toplevel** — Vollständige Top-Level-Fenster mit Titel und App-ID
3. **Custom Wayland Protocol** — Neues Protokoll für krypt-spezifische Metadaten

### Entscheidung
xdg_toplevel für alle AppVM-Fenster.

### Begründung

**Kompatibilität mit bestehenden Hyprland-Regeln (Hauptgrund):**
Hyprlands `windowrule` (Hyprland 0.46+ vereinheitlichte Syntax, vorher
`windowrulev2`) matcht auf `title` und `initialClass`.
Wir setzen:
- `set_title("[<trust>] <vm-name>: <original-title>")` — Trust-Präfix + VM-Name
- `set_app_id("krypt.<vm-name>")` — VM-spezifischer App-Identifier

Das bedeutet alle windowrule-Regeln aus `hyprland.conf` greifen ohne
Änderungen am Compositor:
```
windowrule   = bordercolor rgba(ff5555ff), title:^\[red\]
windowrule   = workspace 1, title:^\[red\]
windowrule   = noblur, title:^\[(red|orange)\]
```
Null Compositor-Modifikation nötig — das ist ein erheblicher Vorteil.

**Warum nicht wl_subsurface:**
wl_subsurface hat keinen eigenen Titel oder App-ID. Hyprlands Fensterverwaltung
(workspace-Zuordnung, border-color, blur-control) würde den Compositor-Eingriff
erfordern oder eine separate Metadaten-Sidecar-Connection.
wl_subsurface wäre sinnvoll wenn wir mehrere AppVM-Fenster in einem Parent-Window
compositen wollen — das ist explizit NICHT unser Ziel (jede AppVM = eigenes Fenster).

**Warum nicht Custom Protocol:**
Ein eigenes Wayland-Protokoll (à la `ext-session-lock` oder `wlr-layer-shell`)
hätte die meiste Flexibilität, aber:
- Hyprland müsste das Protokoll implementieren — erhebliche Upstream-Arbeit
- Kein xdg-portal, kein xwayland, kein Screensharing ohne weitere Protokolle
- Maintenance-Last für Krypt OS
xdg_toplevel ist der Standard — "don't design new protocols when existing ones work"

**wayland-client 0.31:**
Stabile Rust-Bindings für Wayland (Smithay-Projekt). Verlinkt gegen
`libwayland-client.so` (system, kein vendoring). Verfügbar auf Arch (1.25.0).

### Konsequenzen
- AppVM-Fenster erscheinen als normale Toplevel-Windows in sys-gui
- Hyprland-Fensterverwaltung (scratchpad, move, resize) funktioniert für AppVMs
- xdg_toplevel hat keinen eigenen Pixel-Buffer-Transport: der kommt von Xen
  Shared Memory via wl_shm (Phase 10) → leicht unterschiedliche Roundtrip-Kosten
- Trust-Präfix im Titel ist sichtbar in Hyprlands Titelleiste — gewollt
  (visuelles Vertrauens-Signal für den Benutzer)

---

## ADR-012: Installer Threading — std::thread + call_from_thread statt Worker-API
**Datum**: 2026-05-13
**Status**: Accepted

### Kontext
Der TUI-Installer führt zeitintensive Operationen durch (cryptsetup, pacstrap, Xen-Install).
Diese müssen in einem Background-Thread laufen damit die Textual-TUI responsive bleibt.
Textual bietet zwei Ansätze für Background-Arbeit:

1. **`threading.Thread` + `app.call_from_thread()`** — Standard-Python-Thread, UI-Updates
   über Textual's Thread-Bridge
2. **Textual Worker API** (`@work`, `run_worker()`)** — Textual-native async Workers mit
   automatischem Lifecycle-Management und Cancel-Support

### Entscheidung
`threading.Thread` + `app.call_from_thread()` für alle Install-Steps.

### Begründung

**Blockierende subprocess-Calls:**
Installation nutzt `subprocess.run(timeout=600)` für `pacstrap`, `cryptsetup`, etc.
Diese sind blocking I/O — sie lassen sich nicht einfach in async-Code einbetten ohne
`asyncio.to_thread()` + `run_in_executor()`. Die Worker-API ist primär für async-Coroutinen
designed.

**Einfachheit:**
Der Installer ist kein lang-laufender Service sondern ein linearer Ablauf
(Step 1 → 2 → 3 → 4 → 5). Kein Cancel-Support nötig (halbinstalliertes System ist
schlechter als ein abgebrochener Prozess).

**Widget-Referenzen:**
Widgets werden in `on_mount()` als Instanzvariablen gespeichert (bevor Thread startet).
Thread-seitige UI-Updates gehen ausschließlich über `app.call_from_thread(widget.method, arg)`.
Das ist das von Textual dokumentierte Pattern für blocking threads.

**Kein `call_from_thread` für Widget-Lookup:**
Früherer Code nutzte `widget = app.call_from_thread(self.query_one, "#id", Type)` für den
initialen Widget-Lookup. Das ist korrekt (call_from_thread ist synchron aus Thread-Sicht),
aber unnötig — `on_mount()` läuft im Main-Thread, Widget-Referenzen können dort gespeichert
werden.

### Konsequenzen
- Kein Cancel-Button möglich (bewusste Entscheidung — halb-installiert ist schlimmer)
- `subprocess.run(timeout=600)` als Sicherheitsnetz gegen hängende Prozesse
- UI friert nicht ein (Thread separiert von Textual's Event-Loop)
- Fehler im Thread werden über `call_from_thread` in die UI propagiert

---

## ADR-013: build.sh — --skip-rust Flag für CI-Trennung von Build und Test
**Datum**: 2026-05-13
**Status**: Accepted

### Kontext
Der ISO-Build-Workflow in GitHub Actions hat zwei Phasen:
1. `cargo test + clippy` (rust-ci Job, kein archiso nötig)
2. `mkarchiso` (build-iso Job, Arch-Container nötig)

Im build-iso Job laufen wir nochmals `cargo build --release`, haben die Binaries dann im
`target/release/`-Verzeichnis und rufen `build.sh` auf. Problem: build.sh würde ohne Flag
nochmals `cargo build --release` aufrufen — redundant und fehleranfällig (build_user detection).

### Entscheidung
`--skip-rust` Flag in `build.sh` das `cargo build` überspringt und die vorhandenen Binaries
aus `target/release/` direkt nutzt.

### Begründung
- CI baut explizit: `cargo build --release` → dann `build.sh --skip-rust`
- Lokales Bauen ohne Flag: build.sh macht alles in einem Schritt
- Kein doppelter Build in CI (spart ~2 Minuten)
- Klare Trennung: CI-Job verantwortet Rust, build.sh verantwortet ISO-Struktur

### Konsequenzen
- CI-Workflow muss `cargo build --release` explizit aufrufen bevor `build.sh --skip-rust`
- Lokale Entwickler: `sudo ./build/build.sh` reicht (kein Flag nötig)
- Binary-Pfade in build.sh sind hardcoded auf `target/release/` (Cargo workspace default)
