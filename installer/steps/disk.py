# installer/steps/disk.py — Disk-Auswahl + Partitionierung
from __future__ import annotations

import subprocess
from dataclasses import dataclass
from typing import Optional

from textual.app import ComposeResult
from textual.screen import Screen
from textual.widgets import Button, Footer, Header, Label, ListItem, ListView, Static
from textual.containers import Horizontal, Vertical


@dataclass
class BlockDevice:
    path: str    # /dev/sda
    model: str
    size_gb: float
    removable: bool
    is_boot: bool = False     # True wenn das Gerät die gerade gebootete Live-ISO trägt

    def label(self) -> str:
        rm   = " [removable]" if self.removable else ""
        boot = "  ⚠ BOOT MEDIUM — Auswahl gesperrt" if self.is_boot else ""
        return f"{self.path}  {self.model}  ({self.size_gb:.1f} GB){rm}{boot}"


def _boot_disk_kernel_name() -> str | None:
    """Ermittelt den Kernel-Namen (z.B. "sdb" oder "nvme0n1") des Geräts, das die
    aktuell gebootete Live-ISO enthält. Damit kann der Installer diese Disk aus
    der Auswahl sperren — versehentliches Überschreiben der eigenen Boot-Quelle
    ist ein häufiger Foot-Gun in TUI-Installern und für Krypt OS besonders
    unschön, weil danach kein Recovery-Pfad mehr existiert (keine Live-ISO mehr).
    """
    try:
        # archiso mountet die ISO unter /run/archiso/bootmnt
        res = subprocess.run(
            ["findmnt", "-n", "-o", "SOURCE", "/run/archiso/bootmnt"],
            capture_output=True, text=True, timeout=5,
        )
        source = res.stdout.strip()
        if not source:
            return None
        # /dev/sdb1 → sdb; /dev/nvme0n1p1 → nvme0n1
        res2 = subprocess.run(
            ["lsblk", "-n", "-o", "PKNAME", source],
            capture_output=True, text=True, timeout=5,
        )
        parent = res2.stdout.strip().splitlines()
        return parent[0] if parent and parent[0] else None
    except Exception:
        return None


def list_block_devices() -> list[BlockDevice]:
    """Gibt alle nicht-Loop Block-Devices zurück (lsblk JSON).

    Das Boot-Medium wird in der Rückgabe markiert (is_boot=True) statt
    weggefiltert — der User soll sehen WARUM eine bestimmte Disk fehlt,
    sonst wundert er sich über den vermissten USB-Stick und steckt einen
    anderen ein. DiskScreen sperrt die Auswahl auf is_boot-Disks.
    """
    try:
        out = subprocess.run(
            ["lsblk", "--json", "--output", "NAME,MODEL,SIZE,RM,TYPE", "--bytes"],
            capture_output=True, text=True, timeout=10,
        )
        import json
        data = json.loads(out.stdout)
        boot_name = _boot_disk_kernel_name()
        devices = []
        for dev in data.get("blockdevices", []):
            if dev.get("type") != "disk":
                continue
            size_bytes = int(dev.get("size", 0) or 0)
            name = dev["name"]
            devices.append(BlockDevice(
                path=f"/dev/{name}",
                model=(dev.get("model") or "Unknown").strip(),
                size_gb=size_bytes / 1e9,
                removable=bool(dev.get("rm")),
                is_boot=(boot_name is not None and name == boot_name),
            ))
        return devices
    except Exception:
        return []


class DiskScreen(Screen):
    """Disk-Auswahl: zeigt alle Block-Devices, Nutzer wählt Ziel-Disk."""

    CSS = """
    DiskScreen {
        background: $background;
    }
    #title {
        color: #9d4edd;
        text-style: bold;
        margin: 1 2;
    }
    #subtitle {
        color: #6c7086;
        margin: 0 2 1 2;
    }
    #disk-list {
        border: solid #313244;
        margin: 0 2;
        height: 12;
    }
    #disk-list > ListItem.--highlight {
        background: #313244;
        color: #9d4edd;
    }
    #selection-info {
        color: #cdd6f4;
        margin: 1 2;
    }
    #warning {
        color: #f38ba8;
        margin: 0 2 1 2;
    }
    Horizontal {
        margin: 1 2;
        height: 3;
    }
    """

    def __init__(self) -> None:
        super().__init__()
        self._selected: Optional[BlockDevice] = None
        self._devices: list[BlockDevice] = []

    def compose(self) -> ComposeResult:
        yield Header(show_clock=False)
        with Vertical():
            yield Static("Schritt 1 / 5 — Ziel-Disk wählen", id="title")
            yield Static(
                "Wähle das Laufwerk auf dem Krypt OS installiert werden soll.",
                id="subtitle",
            )
            self._devices = list_block_devices()
            items = [
                ListItem(Label(d.label()), id=f"disk-{i}")
                for i, d in enumerate(self._devices)
            ]
            if not items:
                items = [ListItem(Label("Keine Laufwerke gefunden"))]
            yield ListView(*items, id="disk-list")
            yield Static("Kein Laufwerk ausgewählt.", id="selection-info")
            yield Static(
                "⚠  Alle Daten auf dem gewählten Laufwerk werden gelöscht!",
                id="warning",
            )
            with Horizontal():
                yield Button("← Zurück", id="btn-back")
                yield Button("Weiter →", id="btn-next", variant="primary", disabled=True)
        yield Footer()

    def on_list_view_selected(self, event: ListView.Selected) -> None:
        idx_str = (event.item.id or "").replace("disk-", "")
        try:
            idx = int(idx_str)
            self._selected = self._devices[idx]
        except (ValueError, IndexError):
            return

        # Boot-Medium kann nicht überschrieben werden — würde den laufenden
        # Installer und den Recovery-Weg gleichzeitig zerstören.
        if self._selected.is_boot:
            self.query_one("#selection-info", Static).update(
                f"{self._selected.path} ist das Boot-Medium — nicht installierbar."
            )
            self.query_one("#btn-next", Button).disabled = True
            self._selected = None
            return

        self.query_one("#selection-info", Static).update(
            f"Ausgewählt: {self._selected.label()}"
        )
        self.query_one("#btn-next", Button).disabled = False

    def on_button_pressed(self, event: Button.Pressed) -> None:
        if event.button.id == "btn-back":
            self.app.pop_screen()
        elif event.button.id == "btn-next" and self._selected:
            from .luks import LuksScreen
            self.app.push_screen(LuksScreen(disk=self._selected))
