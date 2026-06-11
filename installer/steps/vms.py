# installer/steps/vms.py — AppVM-Erstellung + Abschluss
from __future__ import annotations

import os
import subprocess
import threading

from textual.app import ComposeResult
from textual.screen import Screen
from textual.widgets import Button, Checkbox, Footer, Header, Log, Static
from textual.containers import Vertical


DEFAULT_VMS = [
    ("sys-gui",  "green",  2048, 2, "Hyprland Desktop (sys-gui) — Pflicht"),
    ("work",     "green",  2048, 2, "Arbeits-VM (work)"),
    ("browser",  "yellow", 2048, 2, "Browser-VM (browser)"),
    ("vault",    "black",  1024, 1, "Passwort-Tresor (vault)"),
    ("personal", "green",  1024, 1, "Persönliche VM (personal)"),
]

VM_IMAGES_DIR = "/mnt/var/lib/krypt/vms"
VM_KEYS_DIR   = "/mnt/etc/krypt/keys"


class VmsScreen(Screen):
    """AppVM-Auswahl + Konfiguration + verschlüsselte Disk-Images erstellen."""

    CSS = """
    VmsScreen { background: $background; }
    #title    { color: #9d4edd; text-style: bold; margin: 1 2; }
    #subtitle { color: #6c7086; margin: 0 2 1 2; }
    Checkbox  { margin: 0 2; color: #cdd6f4; }
    #vm-log   { margin: 1 2; height: 12; border: solid #313244; }
    #status   { color: #a6e3a1; margin: 0 2; }
    #error    { color: #f38ba8; margin: 0 2; }
    """

    def __init__(self) -> None:
        super().__init__()
        self._log: Log
        self._status: Static
        self._error: Static

    def compose(self) -> ComposeResult:
        yield Header(show_clock=False)
        with Vertical():
            yield Static("Schritt 5 / 5 — AppVMs konfigurieren", id="title")
            yield Static(
                "Wähle welche VMs beim ersten Start erstellt werden sollen.\n"
                "Für jede VM wird ein verschlüsseltes Disk-Image (LUKS2 + ext4) angelegt.",
                id="subtitle",
            )
            for vm_id, (name, trust, mem, cpus, desc) in enumerate(DEFAULT_VMS):
                yield Checkbox(
                    desc,
                    value=(name in ("sys-gui", "work", "browser")),
                    id=f"vm-{vm_id}",
                )
            yield Log(id="vm-log", auto_scroll=True)
            yield Static("", id="status")
            yield Static("", id="error")
            yield Button("Konfigurieren + Abschließen", id="btn-create", variant="primary")
        yield Footer()

    def on_mount(self) -> None:
        self._log    = self.query_one("#vm-log", Log)
        self._status = self.query_one("#status", Static)
        self._error  = self.query_one("#error", Static)

    def on_button_pressed(self, event: Button.Pressed) -> None:
        if event.button.id == "btn-create":
            event.button.disabled = True
            selected = [
                DEFAULT_VMS[int(cb.id.split("-")[1])]
                for cb in self.query(Checkbox)
                if cb.value and cb.id
            ]
            threading.Thread(target=self._create_vms, args=(selected,), daemon=True).start()

    def _create_vms(self, vms: list) -> None:
        log = lambda msg: self.app.call_from_thread(self._log.write_line, msg)

        try:
            subprocess.run(["mkdir", "-p", "/mnt/etc/xen/krypt"], check=True)
            subprocess.run(["mkdir", "-p", "/mnt/etc/krypt"],     check=True)

            # ── XL-Configs schreiben ──────────────────────────────────────────
            log("Erstelle XL-Konfigurationen…")
            for name, trust, mem, cpus, desc in vms:
                log(f"  ▶ {name} ({trust}, {mem}MB, {cpus} vCPU)")
                with open(f"/mnt/etc/xen/krypt/{name}.cfg", "w") as f:
                    f.write(_generate_xl_config(name, trust, mem, cpus))

            # ── daemon.toml schreiben ─────────────────────────────────────────
            log("Erstelle /etc/krypt/daemon.toml…")
            _write_daemon_toml(vms)

            # ── Verschlüsselte Disk-Images anlegen ────────────────────────────
            log("Erstelle verschlüsselte AppVM-Disk-Images…")
            log("(LUKS2 + ext4, dauert je nach Disk 1–3 Minuten)")
            _create_vm_disk_images(vms, log)

            # ── krypt-vm-open Wrapper schreiben ───────────────────────────────
            log("Erstelle krypt-vm-open Wrapper…")
            _write_krypt_vm_open()

            log("")
            log("✓ Krypt OS Installation abgeschlossen!")
            self.app.call_from_thread(
                self._status.update, "✓ Fertig — bitte neu starten."
            )
            self.app.call_from_thread(self.app.push_screen, FinishScreen())

        except Exception as exc:
            self.app.call_from_thread(self._error.update, f"✗ Fehler: {exc}")
            log(f"✗ {exc}")


def _create_vm_disk_images(vms: list, log_fn) -> None:
    """Erstellt verschlüsselte AppVM-Disk-Images (LUKS2 + ext4) in /mnt/var/lib/krypt/vms/."""
    # os.makedirs(mode=…, exist_ok=True) wendet den mode NUR beim
    # Anlegen an — wenn der Pfad aus einem vorigen (abgebrochenen)
    # Install bereits mit umask-Default 0o755 existiert, bleibt er
    # world-listable und alle Key-Dateinamen darin durch ls einsehbar.
    # Wir setzen den mode darum nach makedirs explizit noch einmal.
    os.makedirs(VM_IMAGES_DIR, mode=0o700, exist_ok=True)
    os.chmod(VM_IMAGES_DIR, 0o700)
    os.makedirs(VM_KEYS_DIR, mode=0o700, exist_ok=True)
    os.chmod(VM_KEYS_DIR, 0o700)

    for name, trust, mem, cpus, _desc in vms:
        img_path = f"{VM_IMAGES_DIR}/{name}.img"
        key_path = f"{VM_KEYS_DIR}/{name}.key"
        mapper   = f"{name}-root"

        if os.path.exists(img_path):
            log_fn(f"  {name}.img existiert bereits, überspringe")
            continue

        size = "10G" if mem >= 2048 else "5G"
        log_fn(f"  {name}: fallocate {size}…")

        # 1. Sparse-Datei anlegen
        subprocess.run(
            ["fallocate", "-l", size, img_path],
            check=True, capture_output=True,
        )

        # 2. 64-Byte-Zufallsschlüssel (kein Passwort — nur Key-File)
        # os.open() statt open() + chmod: ohne die Mode-im-Create-Call läge
        # der Key zwischen write() und chmod() mit umask-Default (0644 unter
        # root) auf dem Filesystem — ein paralleler Prozess könnte ihn lesen.
        # O_EXCL verhindert zudem ein vorhandenes Stale-File als Key-Sink.
        fd = os.open(key_path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o400)
        with os.fdopen(fd, "wb") as kf:
            kf.write(os.urandom(64))

        # 3. LUKS2 formatieren
        log_fn(f"  {name}: cryptsetup luksFormat…")
        subprocess.run(
            [
                "cryptsetup", "luksFormat",
                "--type", "luks2",
                "--cipher", "aes-xts-plain64",
                "--key-size", "512",
                "--hash", "sha512",
                "--pbkdf", "argon2id",
                "--batch-mode",
                "--key-file", key_path,
                img_path,
            ],
            check=True, capture_output=True,
        )

        # 4. Öffnen für ext4-Format
        log_fn(f"  {name}: mkfs.ext4…")
        subprocess.run(
            ["cryptsetup", "open", "--key-file", key_path, img_path, mapper],
            check=True, capture_output=True,
        )
        try:
            subprocess.run(
                ["mkfs.ext4", "-q", "-L", name, f"/dev/mapper/{mapper}"],
                check=True, capture_output=True,
            )
        finally:
            subprocess.run(
                ["cryptsetup", "close", mapper],
                check=False, capture_output=True,
            )

        log_fn(f"  ✓ {name}.img ({size}) — Key: /etc/krypt/keys/{name}.key")


def _write_krypt_vm_open() -> None:
    """Schreibt /mnt/usr/local/bin/krypt-vm-open — öffnet LUKS + startet VM via xl."""
    os.makedirs("/mnt/usr/local/bin", exist_ok=True)
    script = """\
#!/bin/bash
# krypt-vm-open — LUKS-Mapping öffnen + Xen-VM starten
# Usage: krypt-vm-open <vm-name>
set -euo pipefail

VM="${1:?Verwendung: krypt-vm-open <vm-name>}"
KEY_FILE="/etc/krypt/keys/${VM}.key"
IMG_FILE="/var/lib/krypt/vms/${VM}.img"
CFG_FILE="/etc/xen/krypt/${VM}.cfg"

[[ -f "${KEY_FILE}" ]] || { echo "Key-Datei fehlt: ${KEY_FILE}" >&2; exit 1; }
[[ -f "${IMG_FILE}" ]] || { echo "Image fehlt: ${IMG_FILE}" >&2; exit 1; }
[[ -f "${CFG_FILE}" ]] || { echo "XL-Config fehlt: ${CFG_FILE}" >&2; exit 1; }

# LUKS öffnen falls noch nicht offen
if ! cryptsetup status "${VM}-root" >/dev/null 2>&1; then
    echo "Öffne ${VM}.img…"
    cryptsetup open --key-file "${KEY_FILE}" "${IMG_FILE}" "${VM}-root"
fi

echo "Starte VM: ${VM}"
xl create "${CFG_FILE}"
"""
    path = "/mnt/usr/local/bin/krypt-vm-open"
    with open(path, "w") as f:
        f.write(script)
    os.chmod(path, 0o755)


def _generate_xl_config(name: str, trust: str, mem_mb: int, vcpus: int) -> str:
    # AppVM-Disk via /dev/mapper/<name>-root (von krypt-vm-open geöffnet)
    return (
        f"# Krypt OS AppVM — {name} (trust: {trust})\n"
        f"# Starten: krypt-vm-open {name}\n"
        f"# Image:   /var/lib/krypt/vms/{name}.img\n"
        f"# Key:     /etc/krypt/keys/{name}.key\n"
        f"name   = \"{name}\"\n"
        f"memory = {mem_mb}\n"
        f"vcpus  = {vcpus}\n"
        f"kernel = \"/boot/vmlinuz-linux-lts\"\n"
        f"ramdisk = \"/boot/initramfs-linux-lts.img\"\n"
        f"extra  = \"root=/dev/xvda ro quiet\"\n"
        f"disk   = [ \"phy:/dev/mapper/{name}-root,xvda,rw\" ]\n"
        f"vif    = [ \"bridge=xenbr0,mac=00:16:3e:{_mac(name)}\" ]\n"
        f"on_poweroff = \"destroy\"\n"
        f"on_reboot   = \"restart\"\n"
        f"on_crash    = \"destroy\"\n"
    )


def _mac(name: str) -> str:
    """Deterministischer 3-Byte MAC-Suffix aus dem VM-Namen.

    Python's eingebautes hash() ist seit 3.3 per-process randomisiert
    (PYTHONHASHSEED) — Re-Runs des Installers würden sonst jedes Mal neue
    MACs erzeugen. Das bricht DHCP-Reservierungen, ARP-Caches und Xen-
    Bridge-Filter. SHA-256 liefert stabile Bytes über alle Runs hinweg.
    """
    import hashlib
    digest = hashlib.sha256(name.encode("utf-8")).digest()
    return f"{digest[0]:02x}:{digest[1]:02x}:{digest[2]:02x}"


def _write_daemon_toml(vms: list) -> None:
    """Schreibt /mnt/etc/krypt/daemon.toml — nur Felder die config.rs kennt."""
    lines = [
        "# /etc/krypt/daemon.toml — generiert vom Krypt-Installer\n",
        "# Trust-Level: black | green | yellow | orange | red (lowercase!)\n",
        "# Panic-Level: lock | suspend | nuke\n",
        "\n",
        "[daemon]\n",
        'log_level   = "info"\n',
        'panic_level = "suspend"\n',
        "\n",
        "# Auth-Stick nach USB-Setup ergänzen:\n",
        "# [[auth_sticks]]\n",
        '# serial    = "STICK_SERIENNUMMER"\n',
        "# luks_slot = 1\n",
        "\n",
    ]
    for name, trust, mem, cpus, _ in vms:
        lines += [
            "[[vms]]\n",
            f'name        = "{name}"\n',
            f"memory_mb   = {mem}\n",
            f"vcpus       = {cpus}\n",
            f'trust_level = "{trust}"\n',
            f'kernel      = "/boot/vmlinuz-linux-lts"\n',
            "\n",
        ]
    # Standard Inter-VM-Policy
    policy_rules = [
        ("browser", "vault", "deny"),
        ("browser", "work",  "deny"),
        ("work",    "vault", "ask"),
    ]
    for src, tgt, act in policy_rules:
        src_vms = [v[0] for v in vms]
        if src in src_vms and tgt in src_vms:
            lines += [
                "[[policy]]\n",
                f'source = "{src}"\n',
                f'target = "{tgt}"\n',
                f'action = "{act}"\n',
                "\n",
            ]
    with open("/mnt/etc/krypt/daemon.toml", "w") as f:
        f.writelines(lines)


class FinishScreen(Screen):
    """Abschluss-Screen nach erfolgreicher Installation."""

    CSS = """
    FinishScreen { background: $background; align: center middle; }
    #logo    { color: #9d4edd; text-align: center; margin-bottom: 1; }
    #msg     { color: #a6e3a1; text-align: center; margin-bottom: 1; }
    #details { color: #6c7086; text-align: center; margin-bottom: 2; }
    """

    def compose(self) -> ComposeResult:
        from textual.containers import Center, Middle
        with Middle():
            with Center():
                yield Static(
                    "  ██╗  ██╗██████╗ ██╗   ██╗██████╗ ████████╗\n"
                    "  ██║ ██╔╝██╔══██╗╚██╗ ██╔╝██╔══██╗╚══██╔══╝\n"
                    "  █████╔╝ ██████╔╝ ╚████╔╝ ██████╔╝   ██║   \n"
                    "  ██╔═██╗ ██╔══██╗  ╚██╔╝  ██╔═══╝    ██║   \n"
                    "  ██║  ██╗██║  ██║   ██║   ██║         ██║   \n"
                    "  ╚═╝  ╚═╝╚═╝  ╚═╝   ╚═╝   ╚═╝         ╚═╝  \n",
                    id="logo",
                )
                yield Static(
                    "Installation abgeschlossen!\n"
                    "Entferne das Installationsmedium und starte neu.",
                    id="msg",
                )
                yield Static(
                    "Erster Boot:\n"
                    "  UEFI → GRUB → Xen → dom0 (Linux LTS)\n"
                    "  LUKS-Passphrase eingeben — oder USB-Stick einlegen\n\n"
                    "Erste VMs starten:\n"
                    "  sudo krypt-vm-open sys-gui\n"
                    "  sudo krypt-vm-open work",
                    id="details",
                )
                yield Button("Neu starten", id="btn-reboot", variant="success")
                yield Button("Beenden",     id="btn-quit")
        yield Footer()

    def on_button_pressed(self, event: Button.Pressed) -> None:
        if event.button.id == "btn-reboot":
            subprocess.run(["reboot"], check=False)
        elif event.button.id == "btn-quit":
            self.app.exit()
