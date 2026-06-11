# installer/steps/install.py — Partitionierung, LUKS2-Format, Arch+Xen-Installation
from __future__ import annotations

import subprocess
import threading
from typing import TYPE_CHECKING

from textual.app import ComposeResult
from textual.screen import Screen
from textual.widgets import Button, Footer, Header, Log, ProgressBar, Static
from textual.containers import Vertical

if TYPE_CHECKING:
    from .disk import BlockDevice

# Installations-Phasen mit Gewichtung (für ProgressBar)
PHASES = [
    ("Partitionierung",              5),
    ("LUKS2 formatieren",           12),
    ("LUKS2 öffnen",                 3),
    ("Dateisystem anlegen",          5),
    ("Basis-System (pacstrap)",     38),
    ("Xen + Desktop-Pakete",        18),
    ("Bootloader (GRUB)",            7),
    ("Krypt-Daemon installieren",    4),
    ("System konfigurieren",         5),
    ("Initramfs generieren",         3),
]
TOTAL_WEIGHT = sum(w for _, w in PHASES)


class InstallScreen(Screen):
    """Führt die eigentliche Installation durch (partitionieren, LUKS2, pacstrap, Xen)."""

    CSS = """
    InstallScreen { background: $background; }
    #title        { color: #9d4edd; text-style: bold; margin: 1 2; }
    #phase-label  { color: #cdd6f4; margin: 0 2; }
    ProgressBar   { margin: 1 2; }
    ProgressBar > .bar--bar { color: #9d4edd; }
    #install-log  { margin: 0 2; height: 15; border: solid #313244; }
    #error        { color: #f38ba8; margin: 0 2; }
    Vertical      { margin: 1 2; }
    """

    def __init__(self, disk: "BlockDevice", passphrase: str) -> None:
        super().__init__()
        self.disk = disk
        self.passphrase = passphrase
        self._failed = False
        self._log: Log
        self._prog: ProgressBar
        self._label: Static

    def compose(self) -> ComposeResult:
        yield Header(show_clock=False)
        with Vertical():
            yield Static("Schritt 3 / 5 — Installation läuft", id="title")
            yield Static("Starte Installation…", id="phase-label")
            yield ProgressBar(total=100, show_eta=False, id="progress")
            yield Log(id="install-log", auto_scroll=True)
            yield Static("", id="error")
            yield Button("Weiter →", id="btn-next", variant="primary", disabled=True)
        yield Footer()

    def on_mount(self) -> None:
        self._log   = self.query_one("#install-log",  Log)
        self._prog  = self.query_one("#progress",     ProgressBar)
        self._label = self.query_one("#phase-label",  Static)
        threading.Thread(target=self._run_install, daemon=True).start()

    def _run_install(self) -> None:
        done_weight = 0

        def phase(name: str, weight: int) -> None:
            nonlocal done_weight
            self.app.call_from_thread(self._label.update, f"Phase: {name}…")
            self.app.call_from_thread(self._log.write_line, f"▶ {name}")
            done_weight += weight
            pct = int(done_weight / TOTAL_WEIGHT * 100)
            self.app.call_from_thread(self._prog.update, progress=pct)

        def run(cmd: list[str], **kw) -> subprocess.CompletedProcess:
            self.app.call_from_thread(self._log.write_line, "  $ " + " ".join(cmd))
            result = subprocess.run(
                cmd, capture_output=True, text=True, timeout=600, **kw
            )
            if result.stdout.strip():
                for line in result.stdout.strip().splitlines()[-5:]:
                    self.app.call_from_thread(self._log.write_line, f"  {line}")
            if result.returncode != 0:
                for line in result.stderr.strip().splitlines()[-3:]:
                    self.app.call_from_thread(self._log.write_line, f"  ERR: {line}")
                raise RuntimeError(f"Fehlgeschlagen: {' '.join(cmd[:3])}")
            return result

        def run_interactive(cmd: list[str], stdin_data: str, timeout: int = 120) -> None:
            self.app.call_from_thread(self._log.write_line, "  $ " + " ".join(cmd))
            proc = subprocess.Popen(cmd, stdin=subprocess.PIPE, text=True)
            try:
                proc.communicate(input=stdin_data, timeout=timeout)
            except subprocess.TimeoutExpired:
                # Python's communicate() killt das Kind bei TimeoutExpired NICHT.
                # Ohne kill+wait würde cryptsetup ewig hängen und im Hintergrund
                # auf der LUKS-Partition arbeiten während wir glauben, der
                # Install sei abgebrochen.
                proc.kill()
                proc.wait(timeout=5)
                raise RuntimeError(
                    f"Timeout ({timeout}s) erreicht: {' '.join(cmd[:3])}"
                )
            if proc.returncode != 0:
                raise RuntimeError(f"Fehlgeschlagen (exit {proc.returncode}): {' '.join(cmd[:3])}")

        try:
            disk = self.disk.path
            # NVMe: /dev/nvme0n1 → Partitionen /dev/nvme0n1p1
            part_sep  = "p" if disk[-1].isdigit() else ""
            efi_part  = f"{disk}{part_sep}1"
            luks_part = f"{disk}{part_sep}2"

            # ── 1. Partitionierung ────────────────────────────────────────────
            phase("Partitionierung", 5)
            run(["sgdisk", "--zap-all", disk])
            run(["sgdisk", "-n", "1:0:+512M", "-t", "1:ef00", "-c", "1:EFI",  disk])
            run(["sgdisk", "-n", "2:0:0",     "-t", "2:8309", "-c", "2:LUKS", disk])
            run(["partprobe", disk])

            # ── 2. LUKS2 formatieren ──────────────────────────────────────────
            phase("LUKS2 formatieren", 12)
            run_interactive(
                [
                    "cryptsetup", "luksFormat",
                    "--type=luks2",
                    "--cipher=aes-xts-plain64",
                    "--key-size=512",
                    "--hash=sha512",
                    "--pbkdf=argon2id",
                    "--pbkdf-memory=1048576",
                    "--pbkdf-parallel=4",
                    "--iter-time=3000",
                    "--label=krypt-root",
                    "--batch-mode",
                    luks_part,
                ],
                stdin_data=f"{self.passphrase}\n{self.passphrase}\n",
                timeout=300,
            )

            # ── 3. LUKS öffnen ────────────────────────────────────────────────
            phase("LUKS2 öffnen", 3)
            run_interactive(
                ["cryptsetup", "open", luks_part, "krypt-root"],
                stdin_data=f"{self.passphrase}\n",
            )

            # ── 4. Dateisystem ────────────────────────────────────────────────
            phase("Dateisystem anlegen", 5)
            run(["mkfs.fat",  "-F32", "-n", "EFI",       efi_part])
            run(["mkfs.ext4", "-L",   "krypt-root",      "/dev/mapper/krypt-root"])
            run(["mount",  "/dev/mapper/krypt-root",      "/mnt"])
            run(["mkdir",  "-p",                          "/mnt/boot/efi"])
            run(["mount",  efi_part,                      "/mnt/boot/efi"])

            # ── 5. Basis-System ───────────────────────────────────────────────
            phase("Basis-System (pacstrap)", 38)
            run(["pacstrap", "-K", "/mnt",
                 "base", "linux-lts", "linux-lts-headers", "linux-firmware",
                 "cryptsetup", "lvm2", "device-mapper",
                 "mkinitcpio", "sudo", "git", "neovim",
                 "python", "python-pip", "less",
                 "networkmanager", "iproute2"])
            # bridge-utils ist NICHT in extra/core (siehe packages.x86_64) —
            # Bridge-Setup läuft über iproute2: ip link add xenbr0 type bridge
            # genfstab braucht direktes stdout (kein capture_output)
            self.app.call_from_thread(self._log.write_line, "  $ genfstab -U /mnt >> /mnt/etc/fstab")
            with open("/mnt/etc/fstab", "a") as fstab:
                subprocess.run(["genfstab", "-U", "/mnt"], stdout=fstab, check=True, timeout=30)

            # ── 6. Xen + Desktop-Pakete ───────────────────────────────────────
            phase("Xen + Desktop-Pakete", 18)
            run(["arch-chroot", "/mnt", "pacman", "-S", "--noconfirm",
                 "grub", "efibootmgr",
                 "hyprland", "waybar", "foot", "rofi-wayland",
                 "ttf-jetbrains-mono-nerd",
                 "pipewire", "pipewire-pulse", "wireplumber",
                 "python-textual", "python-rich", "python-psutil"])

            # ── 6b. Krypt-Repo + Xen ──────────────────────────────────────────
            # Eigenes Pacman-Repo für Pakete die NICHT in extra/core sind (xen).
            # Defensiv: wenn Repo nicht erreichbar oder xen-Paket fehlt,
            # bricht der Install NICHT ab — User bekommt klare Anleitung.
            xen_installed = _try_install_krypt_repo_and_xen(run, log_fn=lambda s:
                self.app.call_from_thread(self._log.write_line, f"  {s}")
            )

            # ── 7. GRUB ──────────────────────────────────────────────────────
            phase("Bootloader (GRUB)", 7)
            luks_uuid = run(["blkid", "-s", "UUID", "-o", "value", luks_part]).stdout.strip()
            _write_grub_config("/mnt/etc/default/grub", luks_uuid, xen=xen_installed)
            run(["arch-chroot", "/mnt", "grub-install",
                 "--target=x86_64-efi", "--efi-directory=/boot/efi",
                 "--bootloader-id=krypt"])
            run(["arch-chroot", "/mnt", "grub-mkconfig", "-o", "/boot/grub/grub.cfg"])
            if not xen_installed:
                self.app.call_from_thread(self._log.write_line,
                    "  ⚠ Xen wurde nicht installiert — `xl` Befehle werden nach Reboot")
                self.app.call_from_thread(self._log.write_line,
                    "    nicht funktionieren. Siehe docs/post-install.md für manuelle Installation.")

            # ── 8. Krypt-Binaries + Systemd-Units ────────────────────────────
            # Ziel ist /mnt/usr/bin/, NICHT /usr/local/bin/ — die systemd-Unit
            # in init/krypt-daemon.service hat ExecStart=/usr/bin/krypt-daemon
            # hardcoded. Würden wir nach /usr/local/bin/ kopieren, würde der
            # Service nach dem ersten Reboot mit "no such file or directory"
            # scheitern und der USB-Kill-Switch nie aktiv werden.
            phase("Krypt-Daemon installieren", 4)
            for binary in ("krypt-daemon", "krypt-stick", "krypt-gui", "krypt-panic"):
                src = f"/usr/bin/{binary}"
                run(["bash", "-c",
                     f"[ -f {src} ] && install -Dm755 {src} /mnt/usr/bin/{binary} || true"])

            # Systemd-Units kopieren (aus Live-ISO airootfs)
            for unit in ("krypt-daemon.service", "krypt-boot-agent.service"):
                run(["bash", "-c",
                     f"[ -f /etc/systemd/system/{unit} ] && "
                     f"install -Dm644 /etc/systemd/system/{unit} "
                     f"/mnt/etc/systemd/system/{unit} || true"])

            run(["arch-chroot", "/mnt", "systemctl", "enable", "krypt-daemon"])

            # krypt-boot-agent.sh
            run(["bash", "-c",
                 "[ -f /usr/lib/krypt/krypt-boot-agent.sh ] && "
                 "install -Dm755 /usr/lib/krypt/krypt-boot-agent.sh "
                 "/mnt/usr/lib/krypt/krypt-boot-agent.sh || true"])

            # Installer + krypt-install Wrapper für installed system
            run(["bash", "-c",
                 "[ -d /usr/share/krypt-installer ] && "
                 "cp -rT /usr/share/krypt-installer /mnt/usr/share/krypt-installer || true"])
            run(["bash", "-c",
                 "[ -f /usr/bin/krypt-install ] && "
                 "install -Dm755 /usr/bin/krypt-install /mnt/usr/bin/krypt-install || true"])

            # krypt-vm-open Debug-Wrapper (öffnet LUKS + xl create)
            run(["bash", "-c",
                 "[ -f /usr/local/bin/krypt-vm-open ] && "
                 "install -Dm755 /usr/local/bin/krypt-vm-open "
                 "/mnt/usr/local/bin/krypt-vm-open || true"])

            # ── 9. System konfigurieren ───────────────────────────────────────
            phase("System konfigurieren", 5)

            # Locale + Hostname
            run(["bash", "-c", "echo 'en_US.UTF-8 UTF-8' >> /mnt/etc/locale.gen"])
            run(["arch-chroot", "/mnt", "locale-gen"])
            run(["bash", "-c", "echo 'LANG=en_US.UTF-8' > /mnt/etc/locale.conf"])
            run(["bash", "-c", "echo 'krypt-os' > /mnt/etc/hostname"])

            # dom0 Netzwerk-Isolation: kein IP für physische Interfaces
            # NetworkManager in dom0 deaktivieren (dom0 soll kein Netzwerk haben)
            run(["arch-chroot", "/mnt", "systemctl", "disable", "NetworkManager"])
            run(["arch-chroot", "/mnt", "systemctl", "enable",  "systemd-networkd"])

            import os
            os.makedirs("/mnt/etc/systemd/network", exist_ok=True)
            # Physische NICs: unmanaged (dom0 bekommt kein IP)
            _write_file("/mnt/etc/systemd/network/20-dom0-eth.network", _DOM0_ETH_NETWORK)
            # Loopback
            _write_file("/mnt/etc/systemd/network/10-dom0-lo.network", _DOM0_LO_NETWORK)

            # mkinitcpio.conf + krypt-Hooks kopieren
            run(["bash", "-c",
                 "[ -f /etc/mkinitcpio.conf ] && cp /etc/mkinitcpio.conf /mnt/etc/mkinitcpio.conf || true"])
            run(["bash", "-c",
                 "[ -f /etc/initcpio/hooks/krypt ] && "
                 "install -Dm644 /etc/initcpio/hooks/krypt /mnt/etc/initcpio/hooks/krypt || true"])
            run(["bash", "-c",
                 "[ -f /etc/initcpio/install/krypt ] && "
                 "install -Dm644 /etc/initcpio/install/krypt /mnt/etc/initcpio/install/krypt || true"])

            # Root-Passwort + User anlegen — verwendet die LUKS-Passphrase als
            # initialen Default. Nach erstem Login: `passwd` ändern.
            self.app.call_from_thread(
                self._log.write_line,
                "  ℹ Root-Passwort und User 'krypt' werden mit LUKS-Passphrase initialisiert."
            )
            self.app.call_from_thread(
                self._log.write_line,
                "    Nach erstem Login mit `passwd` und `passwd krypt` ändern!"
            )
            run_interactive(
                ["arch-chroot", "/mnt", "chpasswd"],
                stdin_data=f"root:{self.passphrase}\n",
            )
            run(["arch-chroot", "/mnt", "useradd", "-m", "-G", "wheel", "-s", "/bin/bash", "krypt"])
            run_interactive(
                ["arch-chroot", "/mnt", "chpasswd"],
                stdin_data=f"krypt:{self.passphrase}\n",
            )
            # wheel-Gruppe via sudoers freischalten (mit Passwort)
            run(["bash", "-c",
                 "echo '%wheel ALL=(ALL:ALL) ALL' > /mnt/etc/sudoers.d/10-wheel && "
                 "chmod 440 /mnt/etc/sudoers.d/10-wheel"])

            # ── 10. Initramfs generieren ──────────────────────────────────────
            phase("Initramfs generieren", 3)
            run(["arch-chroot", "/mnt", "mkinitcpio", "-P"])

            # Abschluss
            self.app.call_from_thread(self._label.update, "Installation abgeschlossen ✓")
            self.app.call_from_thread(self._log.write_line, "")
            self.app.call_from_thread(self._log.write_line, "✓ Installation erfolgreich abgeschlossen")
            self.app.call_from_thread(
                self.query_one("#btn-next", Button).set, disabled=False
            )

        except Exception as exc:
            self._failed = True
            self.app.call_from_thread(
                self.query_one("#error", Static).update, f"✗ Fehler: {exc}"
            )
            self.app.call_from_thread(self._log.write_line, f"✗ FEHLER: {exc}")

    def on_button_pressed(self, event: Button.Pressed) -> None:
        if event.button.id == "btn-next" and not self._failed:
            from .usb import UsbScreen
            self.app.push_screen(UsbScreen())


def _write_file(path: str, content: str) -> None:
    with open(path, "w") as f:
        f.write(content)


def _write_grub_config(path: str, luks_uuid: str, xen: bool = False) -> None:
    """Schreibt /mnt/etc/default/grub mit krypt-LUKS-Cmdline und optional Xen.

    Wenn xen=True wurde das xen-Paket installiert. Dann setzt grub-mkconfig
    automatisch Multiboot2-Entries via /etc/grub.d/20_linux_xen (xen.gz +
    vmlinuz-linux-lts + initramfs als module2). Wir setzen zusätzlich
    GRUB_CMDLINE_XEN für die dom0 Memory- und CPU-Limits.
    """
    import re
    try:
        with open(path, "r") as f:
            content = f.read()
    except FileNotFoundError:
        content = ""

    cmdline = (
        f"cryptdevice=UUID={luks_uuid}:krypt-root "
        f"root=/dev/mapper/krypt-root "
        f"krypt_luks_uuid={luks_uuid} krypt_luks_name=krypt-root "
        f"quiet splash"
        # krypt_timeout nicht gesetzt = Produktions-Default (unendlich warten)
        # Für QEMU-Tests: GRUB-Editor 'e', dann krypt_timeout=15 anhängen
        # quiet+splash sorgen dafür dass Plymouth während Boot sichtbar bleibt
    )
    if "GRUB_CMDLINE_LINUX=" in content:
        content = re.sub(
            r'^GRUB_CMDLINE_LINUX=.*$',
            f'GRUB_CMDLINE_LINUX="{cmdline}"',
            content, flags=re.MULTILINE,
        )
    else:
        content += f'\nGRUB_CMDLINE_LINUX="{cmdline}"\n'

    krypt_grub = (
        "\nGRUB_TIMEOUT=8\n"
        "GRUB_TIMEOUT_STYLE=menu\n"
        "GRUB_DISTRIBUTOR=\"Krypt OS\"\n"
        # GRUB_DEFAULT=saved: letzte Wahl merken (wichtig für QEMU-Tests)
        "GRUB_DEFAULT=saved\n"
        "GRUB_SAVEDEFAULT=true\n"
        "GRUB_DISABLE_OS_PROBER=true\n"  # Andere OSes nicht erkennen (Sicherheit)
    )
    if xen:
        # dom0 bekommt 2 GB RAM und 2 vCPUs — Rest geht an AppVMs.
        # Anpassbar je nach Host: GRUB_CMDLINE_XEN_DEFAULT in /etc/default/grub.
        krypt_grub += (
            "GRUB_CMDLINE_XEN_DEFAULT=\""
            "dom0_mem=2048M,max:2048M "
            "dom0_max_vcpus=2 "
            "dom0_vcpus_pin "
            "console=vga"
            "\"\n"
        )
    content += krypt_grub

    with open(path, "w") as f:
        f.write(content)


# ── Krypt-Repo (für Xen u.ä. Pakete die NICHT in extra/core sind) ────────────
KRYPT_REPO_URL = "https://github.com/sudoshaco/krypt-pkgs/releases/download/latest"
# Wenn dein Repo woanders liegt: hier den Server-Pfad anpassen.
# Erwartete Layout: krypt.db, krypt.db.tar.gz, *.pkg.tar.zst

_KRYPT_REPO_PACMAN_CONF = f"""

# ── Krypt-Repo ──────────────────────────────────────────────────────────────
# Hostet xen + custom Pakete die nicht in extra/core sind.
# Bei Repo-Migration nur die Server-Zeile anpassen.
[krypt]
SigLevel = Optional TrustAll
Server = {KRYPT_REPO_URL}
"""


def _try_install_krypt_repo_and_xen(run, log_fn) -> bool:
    """Setzt [krypt]-Repo in pacman.conf und installiert xen — best-effort.

    Rückgabe: True wenn xen-Paket erfolgreich installiert, sonst False.
    Bei Fehler wird KEIN RuntimeError geworfen — der Install soll weiterlaufen.
    """
    import subprocess as _sp

    pacman_conf = "/mnt/etc/pacman.conf"
    try:
        with open(pacman_conf, "r") as f:
            existing = f.read()
        if "[krypt]" not in existing:
            with open(pacman_conf, "a") as f:
                f.write(_KRYPT_REPO_PACMAN_CONF)
            log_fn("✓ [krypt]-Repo in pacman.conf eingetragen")
    except OSError as exc:
        log_fn(f"⚠ pacman.conf nicht beschreibbar ({exc}) — überspringe Xen-Install")
        return False

    # Repo-DB syncen — DARF fehlschlagen (Repo könnte noch nicht live sein)
    sync_res = _sp.run(
        ["arch-chroot", "/mnt", "pacman", "-Sy", "--noconfirm"],
        capture_output=True, text=True, timeout=300,
    )
    if sync_res.returncode != 0:
        log_fn("⚠ pacman -Sy fehlgeschlagen — Repo nicht erreichbar?")
        log_fn("  Setze KRYPT_REPO_URL in installer/steps/install.py")
        return False

    # Xen installieren — DARF fehlschlagen (Repo könnte xen nicht haben)
    xen_res = _sp.run(
        ["arch-chroot", "/mnt", "pacman", "-S", "--noconfirm", "xen"],
        capture_output=True, text=True, timeout=600,
    )
    if xen_res.returncode != 0:
        last_err = xen_res.stderr.strip().splitlines()[-1:] if xen_res.stderr else []
        log_fn(f"⚠ xen-Install fehlgeschlagen — {last_err[0] if last_err else 'unbekannt'}")
        log_fn(f"  Manuell nach Reboot: pacman -S xen (aus [krypt] Repo)")
        return False

    log_fn("✓ xen-Hypervisor installiert (multiboot2 GRUB-Entry wird generiert)")
    return True


_DOM0_LO_NETWORK = """\
# dom0 Loopback
[Match]
Name=lo

[Network]
Address=127.0.0.1/8
Address=::1/128
"""

_DOM0_ETH_NETWORK = """\
# dom0 physische Ethernet-Interfaces — KEIN IP-Zugang für dom0
# Netzwerk wird durch sys-net VM via Xen-Bridge verwaltet.
# dom0 darf keinen direkten Internetzugang haben (Sicherheitsprinzip).
[Match]
Type=ether

[Link]
Unmanaged=yes
"""
