# installer/steps/usb.py — USB Auth-Stick einrichten
from __future__ import annotations

import json
import os
import subprocess
import threading

from textual.app import ComposeResult
from textual.screen import Screen
from textual.widgets import Button, Footer, Header, Label, ListItem, ListView, Log, Static
from textual.containers import Vertical


def _list_removable_devices() -> list[str]:
    """Gibt alle entfernbaren Block-Devices zurück (lsblk JSON)."""
    try:
        out = subprocess.run(
            ["lsblk", "--json", "--output", "NAME,RM,TYPE", "--bytes"],
            capture_output=True, text=True, timeout=5,
        )
        data = json.loads(out.stdout)
        return [
            f"/dev/{dev['name']}"
            for dev in data.get("blockdevices", [])
            if dev.get("type") == "disk" and dev.get("rm")
        ]
    except Exception:
        return []


def _find_krypt_stick_binary() -> str | None:
    """Sucht die krypt-stick Binary in bekannten Pfaden."""
    candidates = [
        "/mnt/usr/local/bin/krypt-stick",
        "/usr/bin/krypt-stick",
        "/usr/local/bin/krypt-stick",
    ]
    return next((c for c in candidates if os.path.isfile(c)), None)


class UsbScreen(Screen):
    """USB Auth-Stick auf LUKS-Keyslot einrichten via krypt-stick setup."""

    CSS = """
    UsbScreen  { background: $background; }
    #title     { color: #9d4edd; text-style: bold; margin: 1 2; }
    #info      { color: #cdd6f4; margin: 0 2 1 2; }
    #stick-list { border: solid #45475a; margin: 0 2; height: 5; }
    #status    { color: #a6e3a1; margin: 0 2; }
    #stick-log { margin: 0 2; height: 8; border: solid #313244; }
    #error     { color: #f38ba8; margin: 0 2; }
    """

    def __init__(self) -> None:
        super().__init__()
        self._selected_stick: str | None = None
        self._log: Log
        self._status: Static
        self._error: Static
        self._btn_setup: Button
        self._btn_next: Button

    def compose(self) -> ComposeResult:
        yield Header(show_clock=False)
        with Vertical():
            yield Static("Schritt 4 / 5 — USB Auth-Stick", id="title")
            yield Static(
                "Lege deinen USB-Stick ein. Der Installer schreibt einen 64-Byte-Zufallsschlüssel\n"
                "auf Sektor 1 und registriert ihn als LUKS2-Keyslot.\n"
                "Der Stick wird als physischer Schlüssel zum Entsperren des Systems benötigt.",
                id="info",
            )
            devices = _list_removable_devices()
            items = [ListItem(Label(d), id=f"stick-{i}") for i, d in enumerate(devices)]
            if not items:
                items = [ListItem(Label("Kein entfernbares Laufwerk gefunden — Stick einstecken"))]
            yield ListView(*items, id="stick-list")
            yield Static("Kein Stick ausgewählt.", id="status")
            yield Log(id="stick-log", auto_scroll=True)
            yield Static("", id="error")
            yield Button("Stick einrichten", id="btn-setup", variant="primary", disabled=True)
            yield Button("Überspringen (unsicher — System danach manuell absichern!)", id="btn-skip")
            yield Button("Weiter →", id="btn-next", variant="primary", disabled=True)
        yield Footer()

    def on_mount(self) -> None:
        self._log      = self.query_one("#stick-log", Log)
        self._status   = self.query_one("#status", Static)
        self._error    = self.query_one("#error", Static)
        self._btn_setup = self.query_one("#btn-setup", Button)
        self._btn_next = self.query_one("#btn-next", Button)

    def on_list_view_selected(self, event: ListView.Selected) -> None:
        idx_str = (event.item.id or "").replace("stick-", "")
        try:
            idx = int(idx_str)
            devices = _list_removable_devices()
            self._selected_stick = devices[idx]
            self._status.update(f"Ausgewählt: {self._selected_stick}")
            self._btn_setup.disabled = False
        except (ValueError, IndexError):
            pass

    def on_button_pressed(self, event: Button.Pressed) -> None:
        if event.button.id == "btn-setup" and self._selected_stick:
            self._btn_setup.disabled = True
            threading.Thread(
                target=self._run_setup,
                args=(self._selected_stick,),
                daemon=True,
            ).start()
        elif event.button.id in ("btn-skip", "btn-next"):
            from .vms import VmsScreen
            self.app.push_screen(VmsScreen())

    def _run_setup(self, stick_dev: str) -> None:
        self.app.call_from_thread(self._log.write_line, f"Richte Stick ein: {stick_dev}")
        self.app.call_from_thread(self._status.update, "Richtet Stick ein…")

        binary = _find_krypt_stick_binary()
        if binary is None:
            self.app.call_from_thread(
                self._log.write_line,
                "krypt-stick nicht gefunden — USB-Setup nach Installation manuell:\n"
                "  sudo krypt-stick --luks-dev /dev/sda2 setup --stick-dev /dev/sdb",
            )
            self.app.call_from_thread(
                self._error.update,
                "krypt-stick Binary nicht verfügbar. Manuell einrichten.",
            )
            self.app.call_from_thread(self._btn_next.set, disabled=False)
            return

        # Korrekte krypt-stick CLI: krypt-stick --luks-dev <dev> setup --stick-dev <stick> --force
        cmd = [
            binary,
            "--luks-dev", "/dev/mapper/krypt-root",
            "setup",
            "--stick-dev", stick_dev,
            "--force",   # kein interaktiver Prompt im Installer
        ]
        self.app.call_from_thread(self._log.write_line, "  $ " + " ".join(cmd))

        try:
            result = subprocess.run(
                cmd,
                capture_output=True, text=True, timeout=120,
            )
            for line in (result.stdout + result.stderr).strip().splitlines():
                self.app.call_from_thread(self._log.write_line, f"  {line}")

            if result.returncode == 0:
                self.app.call_from_thread(
                    self._status.update, "✓ Auth-Stick erfolgreich eingerichtet"
                )
                self.app.call_from_thread(self._btn_next.set, disabled=False)
            else:
                raise RuntimeError(
                    f"krypt-stick fehlgeschlagen (exit {result.returncode}) — "
                    "Passphrase korrekt? LUKS-Device /dev/mapper/krypt-root offen?"
                )

        except subprocess.TimeoutExpired:
            self.app.call_from_thread(
                self._error.update, "Timeout — cryptsetup brauchte > 120s"
            )
            self.app.call_from_thread(self._btn_next.set, disabled=False)

        except Exception as exc:
            self.app.call_from_thread(self._error.update, f"✗ Fehler: {exc}")
            self.app.call_from_thread(self._log.write_line, f"✗ {exc}")
            self.app.call_from_thread(self._btn_next.set, disabled=False)
