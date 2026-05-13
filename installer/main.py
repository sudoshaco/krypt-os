#!/usr/bin/env python3
# installer/main.py — Krypt OS TUI-Installer
#
# Verwendung:
#   sudo python3 installer/main.py
#
# Anforderungen:
#   pip install textual rich psutil
#
# Der Installer führt durch:
#   1. Disk-Auswahl
#   2. LUKS2-Verschlüsselung
#   3. Arch + Xen Installation (pacstrap)
#   4. USB Auth-Stick einrichten
#   5. AppVM-Erstellung

import os
import sys

from textual.app import App
from textual.binding import Binding


def _check_root() -> None:
    if os.geteuid() != 0:
        print("Fehler: Krypt-Installer muss als root ausgeführt werden.", file=sys.stderr)
        print("  sudo python3 installer/main.py", file=sys.stderr)
        sys.exit(1)


class KryptInstaller(App):
    """Krypt OS Installations-TUI (textual)."""

    TITLE = "Krypt OS Installer"
    BINDINGS = [
        Binding("q", "quit", "Beenden", show=True),
        Binding("escape", "pop_screen_safe", "Zurück"),
    ]

    CSS = """
    App {
        background: #1e1e2e;
    }
    Header {
        background: #181825;
        color: #9d4edd;
    }
    Footer {
        background: #181825;
        color: #6c7086;
    }
    Button.-primary {
        background: #9d4edd;
        color: #1e1e2e;
        border: none;
    }
    Button.-primary:hover {
        background: #c77dff;
    }
    Button.-success {
        background: #a6e3a1;
        color: #1e1e2e;
    }
    Button {
        background: #313244;
        color: #cdd6f4;
        border: none;
        padding: 0 3;
    }
    Button:disabled {
        background: #1e1e2e;
        color: #45475a;
    }
    Input {
        background: #181825;
        border: solid #45475a;
        color: #cdd6f4;
    }
    Input:focus {
        border: solid #9d4edd;
    }
    Log {
        background: #181825;
        color: #a6adc8;
        scrollbar-color: #313244;
    }
    ProgressBar {
        width: 1fr;
    }
    ProgressBar > .bar--complete {
        color: #9d4edd;
    }
    ProgressBar > .bar--bar {
        color: #9d4edd;
    }
    ListView {
        background: #181825;
        border: solid #45475a;
    }
    ListView:focus {
        border: solid #9d4edd;
    }
    ListItem {
        padding: 0 1;
    }
    Checkbox {
        color: #cdd6f4;
    }
    Checkbox:focus {
        color: #9d4edd;
    }
    """

    def on_mount(self) -> None:
        from steps.welcome import WelcomeScreen
        self.push_screen(WelcomeScreen())

    def action_pop_screen_safe(self) -> None:
        if len(self.screen_stack) > 1:
            self.pop_screen()


if __name__ == "__main__":
    _check_root()
    app = KryptInstaller()
    app.run()
