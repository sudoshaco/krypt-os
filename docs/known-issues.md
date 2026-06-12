# Krypt OS — Bekannte Lücken und Einschränkungen (v0.1.0-alpha)

Dieses Dokument listet alle bekannten Lücken, Einschränkungen und noch nicht implementierten
Komponenten vor dem ersten alpha-Release. Es ist kein Bug-Tracker — konkrete Bugs kommen
nach GitHub Issues. Hier stehen **Design-Lücken**, **fehlende Integrationen** und
**unverifikate Annahmen**, die vor einem produktiven Einsatz beachtet werden müssen.

---

## 1. gui-protocol — Xen Grant-Table (Phase 13)

**Status:** Stub-Implementierung (trust_colored_frame — Farbraster statt echte Pixel)

**Problem:** `gui-protocol/src/xen.rs` simuliert Pixel durch farbige Frames pro Trust-Level.
Die echte Integration via `xengnttab_map_grant_refs()` ist nicht implementiert.

**Voraussetzungen für Phase 13:**
- `libxengnttab.so` muss auf dem Build-System vorhanden sein (`xen` oder `xen-devel` Paket)
- Dom0-Privileg: Grant-Table-Mapping braucht Xen-spezifische Rechte
- AppVM muss ein kompatibles Guest-Agent-Protokoll implementieren (steht noch aus)

**Auswirkung:** krypt-gui zeigt nur Farbraster, keine echten AppVM-Inhalte.
Das System ist für echten Einsatz ohne diese Integration nicht nutzbar.

---

## 2. gui-protocol — Kein Frame-Callback (compositor-driven vsync)

**Status:** sleep-basiertes Frame-Budget (`sleep(budget - elapsed)`)

**Problem:** `gui-protocol/src/main.rs` nutzt `std::thread::sleep` für 60fps-Pacing
statt Wayland `wl_frame_callback`. Beim compositor-getriebenen vsync-Signal wird
die Frame-Rate korrekt angepasst (Minimierung, Hintergrund, Systemlast).

**Auswirkung:** Geringfügig erhöhter CPU-Verbrauch wenn sys-gui minimiert/versteckt ist.
Kein funktionaler Block.

---

## 3. gui-protocol — Kein Input-Forwarding

**Status:** `gui-protocol/src/input.rs` ist definiert, aber nicht mit dem Event-Loop verdrahtet.

**Problem:** Tastatur- und Mauseingaben werden nicht an AppVMs weitergeleitet.
`wl_seat`, `wl_keyboard`, `wl_pointer` Events werden in `wayland.rs` nicht abonniert.

**Auswirkung:** AppVM-Fenster sind nicht interaktiv. Nur Anzeige (Phase 12 Ziel: Stub-Darstellung).
Input-Forwarding ist Phase 13+.

---

## 4. gui-protocol — Kein Inter-VM Clipboard

**Status:** `InputRouter::request_clipboard()` gibt immer `Err("clipboard denied: trust escalation required")`

**Problem:** Clipboard-Transfers zwischen AppVMs mit unterschiedlichem Trust-Level benötigen
einen Trust-Eskalations-Dialog. Der Dialog ist nicht implementiert.

**Auswirkung:** Kein Copy-Paste zwischen VMs. Explizit by design für v0.1.0-alpha
(sichere Default = Deny). Dialog kommt in Phase 13.

---

## 5. installer — python-textual Verfügbarkeit im ISO

**Status:** Unverifikate Annahme

**Problem:** `build/packages.x86_64` enthält `python-textual` mit `[PRÜFEN]`-Annotation.
`python-textual` ist in den offiziellen Arch-Repos verfügbar (seit 2024-01 in `extra/`),
aber die Versionskompatibilität mit `textual>=0.70.0` (in `installer/requirements.txt`)
ist nicht manuell geprüft worden.

**Worst Case:** ISO bootet, Installer-TUI startet nicht auf tty1.

**Workaround:** Im Live-System: `pip install textual>=0.70.0` oder `pacman -U python-textual`.
Oder in `build.sh` ein `pip install -r installer/requirements.txt` ins airootfs einbinden.

---

## 6. installer — krypt-stick benötigt offenen LUKS-Mapper

**Status:** Bekannte Einschränkung

**Problem:** `krypt-stick setup --luks-dev /dev/mapper/krypt-root` im USB-Screen des Installers
setzt voraus, dass `/dev/mapper/krypt-root` zum Zeitpunkt des USB-Screens offen ist.

Der Installer öffnet LUKS im `install.py`-Schritt über `cryptsetup open` — dieses Mapping
bleibt offen solange der Installer läuft. Nach Reboot ist es wieder geschlossen.

**Was es bedeutet:** `krypt-stick setup` im Post-Install-Setup (nach Reboot, ohne Installer)
braucht:
```bash
cryptsetup open /dev/sda2 krypt-root  # interaktive Passphrase
sudo krypt-stick --luks-dev /dev/mapper/krypt-root setup --stick-dev /dev/sdb
```

---

## 7. installer — AppVM Disk-Images nicht erstellt

**Status:** Fehlende Implementierung

**Problem:** `installer/steps/vms.py` schreibt XL-Config-Dateien nach `/mnt/etc/xen/krypt/`
und eine `daemon.toml` nach `/mnt/etc/krypt/`. Es werden aber **keine verschlüsselten
Disk-Images** für die AppVMs erstellt.

Beim ersten `xl create /etc/xen/krypt/work.cfg` nach dem Reboot schlägt Xen fehl,
weil die in der XL-Config referenzierten Disk-Images (`/var/lib/krypt/vms/work.img`) fehlen.

**Fix (manuell):**
```bash
# Pro VM auf dem installierten System:
cryptsetup luksFormat --type luks2 /var/lib/krypt/vms/work.img
cryptsetup open /var/lib/krypt/vms/work.img work-root
mkfs.ext4 /dev/mapper/work-root
# Bootstrap Alpine oder Arch als AppVM-Template
```

**Geplant:** Phase 13 — `_create_vm_disk_images()` in vms.py als letzter Install-Schritt.

---

## 8. installer — NVMe-Gerätepfade nicht im Installer-Disk-Screen

**Status:** Teilweise implementiert

**Problem:** `installer/steps/disk.py` listet alle `lsblk`-Disks, aber NVMe-Geräte
(`/dev/nvme0n1`) werden als Pfad korrekt dargestellt. Beim Install-Schritt (`install.py`)
wird `part_sep = "p" if disk[-1].isdigit() else ""` korrekt gehandhabt.

**Nicht getestet:** Virtuelle QEMU-NVMe-Geräte. Nur SATA-Simulation (`/dev/vda`) wurde
konzeptionell validiert.

---

## 9. daemon.toml — socket_path nicht geparst

**Status:** Inkonsistenz zwischen Doku und Code

**Problem:** Frühere Versionen von `daemon.toml` enthielten `socket_path = "/run/krypt/ipc.sock"`.
Das Feld existiert nicht in `vm-daemon/src/config.rs` (`KryptConfig` Struct).

Der Socket-Pfad ist in `vm-daemon/src/main.rs` hardcoded:
```rust
const SOCKET_PATH: &str = "/run/krypt/ipc.sock";
```

**Status:** `build/airootfs/etc/krypt/daemon.toml` enthält `socket_path` nicht mehr (Phase 12 fix).
Aber `vm-daemon/daemon.toml.example` muss ebenfalls geprüft werden.

---

## 10. initramfs — Kein Fallback auf Passphrase

**Status:** By Design, aber dokumentiert

**Problem:** Der mkinitcpio-Hook `initramfs/hooks/krypt` hat **keinen Passphrase-Fallback**.
Wenn kein Auth-Stick gefunden wird, wartet der Hook in einer Endlosschleife mit ASCII-Banner.

**Auswirkung:** Wer den Stick verliert (ohne Backup-Stick), kann das System nicht entsperren.

**Backup-Stick einrichten (vor Deployment):**
```bash
sudo krypt-stick --luks-dev /dev/sda2 add-backup --stick-dev /dev/sdc
```

**Bekannte Lücke:** Auch Backup-Stick-Boot ist ungetestet auf echter Hardware.

---

## 11. GRUB — JetBrainsMono.pf2 Font nicht generiert

**Status:** ✅ Behoben — `build.sh` generiert Regular + Bold PF2 separat

**War (Iteration 1):** Das GRUB-Theme (`dotfiles/grub/krypt-grub/theme.txt`)
referenziert `JetBrainsMono Nerd Font Regular {10,11,13,14}`. PF2-Dateien
wurden vom `build.sh` nicht generiert; GRUB fiel auf `unicode.pf2` zurück.

**War (Iteration 2):** Erste Fix-Version hat NUR die Regular-TTF konvertiert.
theme.txt nutzt aber zusätzlich `Bold 14` (selected_item_font) und `Bold 28`
(KRYPT OS Title) — diese Strings hatten weiterhin keinen passenden PF2 und
fielen still auf den Default-Font (≈12pt Regular) zurück. Das Boot-Menü-
Layout (`item_height = 38`) wirkte dadurch verschoben.

**Fix (final):** build.sh löst Regular- und Bold-TTF separat auf und ruft

```bash
grub-mkfont --size=${size} --output=jbm-regular-${size}.pf2 <regular-ttf>  # 10/11/13/14
grub-mkfont --size=${size} --output=jbm-bold-${size}.pf2    <bold-ttf>     # 14/28
```

Fehlt eine der beiden TTFs, wird gewarnt aber NICHT abgebrochen — GRUB
nutzt dann weiter den Default-Font für die fehlende Variante.

**Voraussetzung im Build-System:** `pacman -S grub ttf-jetbrains-mono-nerd`.
In der CI ist beides Teil des `archlinux:latest` Container-Setups.

---

## 12. Plymouth — Script-Array-Syntax nicht auf echter Plymouth-Instanz geprüft

**Status:** Unverifikate Annahme

**Problem:** `dotfiles/plymouth/krypt/krypt.script` nutzt `logo_images[i]`-Array-Syntax.
Plymouth-Script-Engine-Versionen variieren. Die Syntax wurde nicht auf einer echten
Plymouth-Instanz (Arch Linux, plymouth ≥ 22.02) validiert.

**Risiko:** Plymouth-Splash startet nicht korrekt — kein blockierendes Problem (Boot läuft trotzdem),
aber visuell defekt.

---

## 13. IOMMU — Ohne IOMMU keine echte Isolation

**Status:** Voraussetzung, nicht prüfbar im QEMU-Test

**Problem:** Xen PCI-Passthrough (für GPU, USB) benötigt VT-d (Intel) oder AMD-Vi.
QEMU-Tests können IOMMU nicht simulieren.

**Konsequenz:** Xen-VMs ohne IOMMU können nicht sicher auf Hardware-Geräte zugreifen.
sys-gui (GPU-Passthrough) funktioniert ohne IOMMU nicht produktiv.

**Prüfbefehl auf echter Hardware:**
```bash
xl dmesg | grep -i iommu
# Erwartet: "IOMMU: ... enabled"
```

---

## 14. dom0 Netzwerkisolation — nicht automatisch konfiguriert

**Status:** Manuelle Maßnahme erforderlich

**Problem:** Krypt OS setzt voraus, dass dom0 keinen Netzwerkzugang hat
(nur AppVMs haben Netzwerk über sys-net/sys-firewall). Dieses Routing wird
**nicht automatisch durch den Installer konfiguriert**.

**Nach Installation manuell:**
```bash
# In dom0 nach Xen-Boot:
ip link set eth0 down
ip route flush table main
# Besser: NetworkManager in dom0 deaktivieren
systemctl disable --now NetworkManager
```

---

## 15. Hyprland col.shadow Syntax (Versionsabhängig)

**Status:** Mögliche Inkompatibilität

**Problem:** `dotfiles/hyprland/hyprland.conf` nutzt `col.shadow` für Trust-Level-Schatten.
In Hyprland ≥ 0.40 wurde das zu `shadow_color` umbenannt.

**Prüfen:** `hyprctl version | grep -i hyprland` und Syntax mit Release-Notes vergleichen.

---

## 16. krypt-Hook Infinity-Loop (Phase 14 behoben)

**Status:** ✅ Behoben in Phase 14

**War:** `initramfs/hooks/krypt` hatte keinen Timeout — bei fehlendem USB-Stick unendliche Schleife,
`encrypt`-Hook lief nie.

**Fix:** `krypt_timeout=<N>` Kernel-Cmdline-Parameter. Bei N>0 läuft Hook maximal N Sekunden,
dann Fallback auf `encrypt`-Hook (Passphrase-Prompt).
- Produktion: kein `krypt_timeout` → unendlich warten (by design, kein Passwort-Fallback)
- QEMU-Test: `krypt_timeout=15` in GRUB-Editor eintragen

---

## 17. Xen nicht in offiziellen Arch-Repos (ISO-Build-Blocker)

**Status:** Bekannte Architektur-Einschränkung

**Problem:** `xen` ist nicht in `extra`, `core` oder `community` — nur über AUR verfügbar.
`pacman -Ss "^xen"` gibt keine Ergebnisse. `mkarchiso` kann nur offizielle Repo-Pakete einbinden.
Desgleichen `bridge-utils` (Funktionalität ist in `iproute2` enthalten).

**Konsequenz für ISO:** `xen` und `bridge-utils` wurden aus `build/packages.x86_64` entfernt.
Das ISO enthält keinen Xen-Hypervisor. Der Installer installiert Xen ebenfalls nicht automatisch.

**Konsequenz für Installer:** `installer/steps/install.py` ruft `pacman -S xen` nicht mehr auf.

**Xen post-install (manuell nach erstem Boot):**
```bash
# Voraussetzung: yay oder paru installieren
sudo pacman -S --needed base-devel git
git clone https://aur.archlinux.org/yay.git && cd yay && makepkg -si

# Xen aus AUR installieren
yay -S xen

# Xen-Einträge in GRUB ergänzen
sudo grub-mkconfig -o /boot/grub/grub.cfg

# dom0 booten: im GRUB-Menü "Xen" auswählen
```

**Langfristige Lösung:** Eigenes Pacman-Repo mit signiertem `xen`-Paket aufbauen,
oder ein offizielles Arch-Paket beantragen / maintainen.

**Auswirkung auf v0.1.0-alpha:** Xen muss post-install manuell eingerichtet werden.
Das ISO bootet als reguläres Arch Linux — Xen wird erst nach manuellem AUR-Install aktiv.

---

## Zusammenfassung — Blockt v0.1.0-alpha?

Phase 13+14 Status:

| Lücke | Blockt Alpha? | Status |
|---|---|---|
| AppVM Disk-Images nicht erstellt | Ja | ✅ Behoben Phase 13: `_create_vm_disk_images()` |
| python-textual nicht im installed system | Ja | ✅ Behoben Phase 13: pacman-Install ergänzt |
| dom0 Netzwerkisolation manuell | Ja | ✅ Behoben Phase 13: NM disabled, networkd+Unmanaged |
| daemon.toml ungültige Felder | Ja | ✅ Behoben Phase 12: socket_path etc. entfernt |
| krypt-Hook Infinity-Loop (QEMU) | Ja (QEMU-Test) | ✅ Behoben Phase 14: krypt_timeout Parameter |
| genfstab stdout-Bug | Ja | ✅ Behoben Phase 13 |
| mkinitcpio -P nicht ausgeführt | Ja | ✅ Behoben Phase 13 |
| gui-protocol Xen Grant-Table | Nein (Stub reicht) | Phase 15 |
| Input-Forwarding fehlt | Nein (Demo) | Phase 15 |
| Inter-VM Clipboard fehlt | Nein (Deny=default) | Phase 15 |
| GRUB PF2-Font fehlt | Nein (Fallback-Font) | ✅ Behoben: build.sh grub-mkfont 10/11/13/14pt |
| Plymouth Script-Syntax unvalidiert | Nein (kein Boot-Blocker) | Vor v0.1.0 |
| IOMMU Voraussetzung | Nein (Docs reichen) | Docs |
| Hyprland col.shadow Syntax | Nein (graceful) | Patch |
| Xen nicht in Arch-Repos (Issue 17) | Nein (post-install via AUR) | Manuell |

### Offene Alpha-Blocker

Alle Alpha-Blocker sind behoben. Verbleibende Aufgaben vor v0.1.0-alpha:

1. **QEMU-Boot-Test durchführen** — ISO bauen (`sudo pacman -S archiso && sudo ./build/build.sh --clean`), docs/qemu-boot-log.md ausfüllen
2. **AppVM-Template Bootstrap** — `sys-gui.img` enthält leeres ext4, braucht Alpine/Arch Base-System für erste VM
3. **GRUB PF2-Font** — `grub-mkfont JetBrainsMono → JetBrainsMono.pf2` in build.sh
4. **Plymouth validieren** — Script-Array-Syntax auf echter Plymouth-Instanz

---

## Roadmap bis v0.1.0-alpha

Die minimalen Anforderungen für einen ersten alpha-Release:

1. **ISO bootet** — GRUB → Arch Live-System (QEMU-validiert) ← **QEMU-Test ausstehend**
2. **Installer läuft durch** — Welcome → Disk → LUKS2 → pacstrap → VMs → Finish ← **QEMU-Test ausstehend**
3. **krypt-daemon startet** — systemctl active, Socket 0600, Config geladen ← **QEMU-Test ausstehend**
4. **AppVM Disk-Images** — Installer legt LUKS2+ext4 Images an ← **Implementiert, ungetestet**
5. **dom0 Netzwerk isoliert** — NM disabled, kein IP auf physischen NICs ← **Implementiert**
6. **USB Kill-Switch** — Stick abziehen → Panic (auf echter Hardware) ← **Hardware-Test ausstehend**

**Was explizit NICHT in v0.1.0-alpha sein muss:**
- Echtes Pixel-Sharing (gui-protocol Phase 15)
- Input-Forwarding
- Inter-VM Clipboard
- TPM2-Integration
