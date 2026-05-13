#!/usr/bin/env python3
# krypt-vms.py — Waybar custom module
# Zeigt laufende VMs mit Trust-Level-Farbe in der Statusbar.
#
# Kommuniziert mit krypt-daemon via Unix-Socket /run/krypt/ipc.sock.
# Protokoll: 4-Byte-LE-Länge + JSON-Body (identisch zu ipc.rs).
# Fallback: Leere Liste wenn Daemon nicht erreichbar.
#
# Waybar config:
#   "custom/krypt-vms": {
#       "exec": "~/.config/waybar/krypt-vms.py",
#       "interval": 5,
#       "return-type": "json",
#       "format": "{}"
#   }

import json
import socket
import struct
import sys

SOCKET_PATH = "/run/krypt/ipc.sock"
TIMEOUT_SEC = 2.0
MAX_FRAME    = 64 * 1024

# Catppuccin Mocha — eine Farbe pro Trust-Level
COLORS: dict[str, str] = {
    "red":    "#f38ba8",  # Untrusted
    "orange": "#fab387",  # Low-trust
    "yellow": "#f9e2af",  # Medium
    "green":  "#a6e3a1",  # Trusted
    "black":  "#cba6f7",  # Vault (lila für Sichtbarkeit)
    "blue":   "#89b4fa",  # System VMs
    "gray":   "#6c7086",  # unbekannt
}


def _recv_exact(sock: socket.socket, n: int) -> bytes:
    buf = bytearray()
    while len(buf) < n:
        chunk = sock.recv(n - len(buf))
        if not chunk:
            raise ConnectionError("IPC connection closed prematurely")
        buf.extend(chunk)
    return bytes(buf)


def _ipc_request(msg: dict) -> dict:
    """Sendet eine IPC-Nachricht und gibt die Antwort zurück."""
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as sock:
        sock.settimeout(TIMEOUT_SEC)
        sock.connect(SOCKET_PATH)

        body = json.dumps(msg).encode("utf-8")
        sock.sendall(struct.pack("<I", len(body)) + body)

        raw_len = _recv_exact(sock, 4)
        length = struct.unpack("<I", raw_len)[0]
        if length > MAX_FRAME:
            raise ValueError(f"IPC frame too large: {length} bytes")

        return json.loads(_recv_exact(sock, length))


def get_running_vms() -> list[dict[str, str]]:
    """
    Fragt krypt-daemon nach allen VMs via ListVmsQuery.
    Gibt eine Liste von {"name", "trust_level"} zurück (nur Running-VMs).
    Bei Fehler: leere Liste (kein Crash, kein Daemon heißt kein Output).
    """
    try:
        response = _ipc_request({"type": "list_vms_query"})
    except (OSError, ConnectionError, ValueError, json.JSONDecodeError):
        return []

    if response.get("type") != "list_vms_response":
        return []

    return [
        {"name": vm["name"], "trust_level": vm.get("trust_level", "gray")}
        for vm in response.get("vms", [])
        if vm.get("state") == "Running"
    ]


def build_output(vms: list[dict[str, str]]) -> str:
    if not vms:
        return json.dumps({
            "text":    "no vms",
            "tooltip": "Keine VMs aktiv",
            "class":   "idle",
        })

    parts: list[str] = []
    tooltip_lines = [f"{len(vms)} VM(s) aktiv:"]

    for vm in sorted(vms, key=lambda v: v["name"]):
        trust = vm["trust_level"]
        color = COLORS.get(trust, COLORS["gray"])
        parts.append(f'<span color="{color}">⬤</span> {vm["name"]}')
        tooltip_lines.append(f"  {vm['name']} [{trust}]")

    return json.dumps({
        "text":    "  ".join(parts),
        "tooltip": "\n".join(tooltip_lines),
        "class":   "active",
    })


if __name__ == "__main__":
    vms = get_running_vms()
    print(build_output(vms))
    sys.exit(0)
