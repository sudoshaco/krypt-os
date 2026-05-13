# installer/steps/welcome.py — Willkommens-Screen
from textual.app import ComposeResult
from textual.screen import Screen
from textual.widgets import Button, Static, Footer
from textual.containers import Center, Middle


LOGO = """\
  ██╗  ██╗██████╗ ██╗   ██╗██████╗ ████████╗
  ██║ ██╔╝██╔══██╗╚██╗ ██╔╝██╔══██╗╚══██╔══╝
  █████╔╝ ██████╔╝ ╚████╔╝ ██████╔╝   ██║
  ██╔═██╗ ██╔══██╗  ╚██╔╝  ██╔═══╝    ██║
  ██║  ██╗██║  ██║   ██║   ██║         ██║
  ╚═╝  ╚═╝╚═╝  ╚═╝   ╚═╝   ╚═╝         ╚═╝  """


class WelcomeScreen(Screen):
    """Erster Installer-Screen: Logo, Warnung, Weiter-Button."""

    CSS = """
    WelcomeScreen {
        background: $background;
        align: center middle;
    }
    #logo {
        color: #9d4edd;
        text-align: center;
        margin-bottom: 1;
    }
    #tagline {
        color: #cdd6f4;
        text-align: center;
        margin-bottom: 2;
    }
    #warning {
        color: #f38ba8;
        border: solid #f38ba8;
        padding: 1 3;
        margin-bottom: 2;
        max-width: 70;
        text-align: center;
    }
    #btn-start {
        background: #9d4edd;
        color: #1e1e2e;
        border: none;
        padding: 0 4;
    }
    #btn-start:hover {
        background: #c77dff;
    }
    #btn-quit {
        background: $surface;
        margin-left: 2;
    }
    """

    def compose(self) -> ComposeResult:
        with Middle():
            with Center():
                yield Static(LOGO, id="logo")
                yield Static("Cryptography · Isolation · Lightweight", id="tagline")
                yield Static(
                    "⚠  WARNUNG: Der Installer löscht alle Daten auf dem"
                    " Ziel-Laufwerk.\nNur auf dedizierter Hardware ausführen.",
                    id="warning",
                )
                with Center():
                    yield Button("Installieren →", id="btn-start", variant="primary")
                    yield Button("Abbrechen", id="btn-quit")
        yield Footer()

    def on_button_pressed(self, event: Button.Pressed) -> None:
        if event.button.id == "btn-start":
            from .disk import DiskScreen
            self.app.push_screen(DiskScreen())
        elif event.button.id == "btn-quit":
            self.app.exit()
