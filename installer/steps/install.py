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
            proc.communicate(input=stdin_data, timeout=timeout)
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
                 "networkmanager", "iproute2", "bridge-utils"])
            # genfstab braucht direktes stdout (kein capture_output)
            self.app.call_from_thread(self._log.write_line, "  $ genfstab -U /mnt >> /mnt/etc/fstab")
            with open("/mnt/etc/fstab", "a") as fstab:
                subprocess.run(["genfstab", "-U", "/mnt"], stdout=fstab, check=True, timeout=30)

            # ── 6. Desktop-Pakete ─────────────────────────────────────────────
            # xen ist nicht in offiziellen Arch-Repos — post-install via AUR
            phase("Desktop-Pakete", 18)
            run(["arch-chroot", "/mnt", "pacman", "-S", "--noconfirm",
                 "grub", "efibootmgr",
                 "hyprland", "waybar", "foot", "rofi-wayland",
                 "ttf-jetbrains-mono-nerd",
                 "pipewire", "pipewire-pulse", "wireplumber",
                 "python-textual", "python-rich", "python-psutil"])

            # ── 7. GRUB ──────────────────────────────────────────────────────
            phase("Bootloader (GRUB)", 7)
            luks_uuid = run(["blkid", "-s", "UUID", "-o", "value", luks_part]).stdout.strip()
            _write_grub_config("/mnt/etc/default/grub", luks_uuid)
            run(["arch-chroot", "/mnt", "grub-install",
                 "--target=x86_64-efi", "--efi-directory=/boot/efi",
                 "--bootloader-id=krypt"])
            run(["arch-chroot", "/mnt", "grub-mkconfig", "-o", "/boot/grub/grub.cfg"])

            # ── 8. Krypt-Binaries + Systemd-Units ────────────────────────────
            phase("Krypt-Daemon installieren", 4)
            for binary in ("krypt-daemon", "krypt-stick", "krypt-gui"):
                src = f"/usr/bin/{binary}"
                run(["bash", "-c",
                     f"[ -f {src} ] && install -Dm755 {src} /mnt/usr/local/bin/{binary} || true"])

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


def _write_grub_config(path: str, luks_uuid: str) -> None:
    import re
    try:
        with open(path, "r") as f:
            content = f.read()
    except FileNotFoundError:
        content = ""

    cmdline = (
        f"cryptdevice=UUID={luks_uuid}:krypt-root "
        f"root=/dev/mapper/krypt-root "
        f"krypt_luks_uuid={luks_uuid} krypt_luks_name=krypt-root"
        # krypt_timeout nicht gesetzt = Produktions-Default (unendlich warten)
        # Für QEMU-Tests: GRUB-Editor 'e', dann krypt_timeout=15 anhängen
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
    content += krypt_grub

    with open(path, "w") as f:
        f.write(content)


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
