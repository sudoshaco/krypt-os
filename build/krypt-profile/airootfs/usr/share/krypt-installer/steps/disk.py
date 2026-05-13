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

    def label(self) -> str:
        rm = " [removable]" if self.removable else ""
        return f"{self.path}  {self.model}  ({self.size_gb:.1f} GB){rm}"


def list_block_devices() -> list[BlockDevice]:
    """Gibt alle nicht-Loop Block-Devices zurück (lsblk JSON)."""
    try:
        out = subprocess.run(
            ["lsblk", "--json", "--output", "NAME,MODEL,SIZE,RM,TYPE", "--bytes"],
            capture_output=True, text=True, timeout=10,
        )
        import json
        data = json.loads(out.stdout)
        devices = []
        for dev in data.get("blockdevices", []):
            if dev.get("type") != "disk":
                continue
            size_bytes = int(dev.get("size", 0) or 0)
            devices.append(BlockDevice(
                path=f"/dev/{dev['name']}",
                model=(dev.get("model") or "Unknown").strip(),
                size_gb=size_bytes / 1e9,
                removable=bool(dev.get("rm")),
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
            self.query_one("#selection-info", Static).update(
                f"Ausgewählt: {self._selected.label()}"
            )
            self.query_one("#btn-next", Button).disabled = False
        except (ValueError, IndexError):
            pass

    def on_button_pressed(self, event: Button.Pressed) -> None:
        if event.button.id == "btn-back":
            self.app.pop_screen()
        elif event.button.id == "btn-next" and self._selected:
            from .luks import LuksScreen
            self.app.push_screen(LuksScreen(disk=self._selected))
