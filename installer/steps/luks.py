# installer/steps/luks.py — LUKS2-Verschlüsselung einrichten
from __future__ import annotations

import subprocess
from typing import TYPE_CHECKING

from textual.app import ComposeResult
from textual.screen import Screen
from textual.widgets import Button, Footer, Header, Input, Label, Static
from textual.containers import Horizontal, Vertical

if TYPE_CHECKING:
    from .disk import BlockDevice

MIN_PASSPHRASE_LEN = 20


class LuksScreen(Screen):
    """LUKS2-Passphrase eingeben + Verschlüsselung konfigurieren."""

    CSS = """
    LuksScreen {
        background: $background;
    }
    #title {
        color: #9d4edd;
        text-style: bold;
        margin: 1 2;
    }
    #disk-info {
        color: #89b4fa;
        margin: 0 2 1 2;
    }
    .section-label {
        color: #a6adc8;
        margin: 1 2 0 2;
    }
    Input {
        margin: 0 2;
    }
    #strength-bar {
        margin: 0 2 1 2;
        color: #6c7086;
    }
    #error {
        color: #f38ba8;
        margin: 0 2;
        height: 1;
    }
    #info {
        color: #6c7086;
        margin: 1 2;
    }
    Horizontal {
        margin: 1 2;
        height: 3;
    }
    """

    def __init__(self, disk: "BlockDevice") -> None:
        super().__init__()
        self.disk = disk

    def compose(self) -> ComposeResult:
        yield Header(show_clock=False)
        with Vertical():
            yield Static("Schritt 2 / 5 — LUKS2-Verschlüsselung", id="title")
            yield Static(f"Ziel: {self.disk.label()}", id="disk-info")

            yield Label("Passphrase (mind. 20 Zeichen):", classes="section-label")
            yield Input(placeholder="Passphrase eingeben…", password=True, id="passphrase")
            yield Label("Passphrase bestätigen:", classes="section-label")
            yield Input(placeholder="Passphrase wiederholen…", password=True, id="confirm")
            yield Static("", id="strength-bar")
            yield Static("", id="error")

            yield Static(
                "LUKS2 mit Argon2id-KDF (PBKDF2 disabled).\n"
                "cipher=aes-xts-plain64  key-size=512  hash=sha512\n"
                "USB Auth-Stick kann im nächsten Schritt eingerichtet werden.",
                id="info",
            )
            with Horizontal():
                yield Button("← Zurück", id="btn-back")
                yield Button("Weiter →", id="btn-next", variant="primary", disabled=True)
        yield Footer()

    def on_input_changed(self, event: Input.Changed) -> None:
        self._validate()

    def _validate(self) -> None:
        pw  = self.query_one("#passphrase", Input).value
        pw2 = self.query_one("#confirm", Input).value
        err = self.query_one("#error", Static)
        bar = self.query_one("#strength-bar", Static)
        btn = self.query_one("#btn-next", Button)

        # Stärke-Anzeige
        strength = _passphrase_strength(pw)
        bar.update(f"Stärke: {'█' * strength}{'░' * (5 - strength)}  ({_strength_label(strength)})")

        if not pw:
            err.update("")
            btn.disabled = True
            return
        if len(pw) < MIN_PASSPHRASE_LEN:
            err.update(f"Passphrase zu kurz (min. {MIN_PASSPHRASE_LEN} Zeichen)")
            btn.disabled = True
            return
        if pw2 and pw != pw2:
            err.update("Passphrases stimmen nicht überein")
            btn.disabled = True
            return
        if pw == pw2 and len(pw) >= MIN_PASSPHRASE_LEN:
            err.update("")
            btn.disabled = False
        else:
            btn.disabled = True

    def on_button_pressed(self, event: Button.Pressed) -> None:
        if event.button.id == "btn-back":
            self.app.pop_screen()
        elif event.button.id == "btn-next":
            pw = self.query_one("#passphrase", Input).value
            from .install import InstallScreen
            self.app.push_screen(InstallScreen(disk=self.disk, passphrase=pw))


def _passphrase_strength(pw: str) -> int:
    """Gibt 0–5 zurück (grob: Länge + Zeichenklassen)."""
    score = 0
    if len(pw) >= 12: score += 1
    if len(pw) >= 20: score += 1
    if any(c.isupper() for c in pw): score += 1
    if any(c.isdigit() for c in pw): score += 1
    if any(not c.isalnum() for c in pw): score += 1
    return score


def _strength_label(score: int) -> str:
    return ["Sehr schwach", "Schwach", "Mittel", "Gut", "Stark", "Sehr stark"][score]
