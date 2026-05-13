#!/bin/bash
# krypt-launcher.sh — Rofi VM-Launcher
#
# Listet alle VMs (laufend + gestoppt) mit Trust-Level-Icons.
# Ausgewählte VM: starten wenn gestoppt, Workspace wechseln wenn läuft.
#
# Verwendung aus Waybar: on-click
# Verwendung aus Hyprland: bind = $mod, Space, exec, ~/.config/rofi/krypt-launcher.sh

SOCKET_PATH="/run/krypt/ipc.sock"
ROFI_THEME="$HOME/.config/rofi/krypt.rasi"

# Trust-Level → Icon + Farbe (Pango-Markup)
trust_icon() {
    case "$1" in
        black)  echo "󰌾" ;;  # Vault
        green)  echo "󰒊" ;;  # Trusted
        yellow) echo "󰒋" ;;  # Medium
        orange) echo "󰀦" ;;  # Low
        red)    echo "󰕈" ;;  # Untrusted
        blue)   echo "󰒍" ;;  # System
        *)      echo "󰏝" ;;  # Unknown
    esac
}

state_icon() {
    case "$1" in
        Running) echo "󰐊" ;;   # play
        Halted)  echo "󰏤" ;;   # stop
        Paused)  echo "󰏦" ;;   # pause
        *)       echo "󰒿" ;;
    esac
}

# ─────────────────────────────────────────────────────────────────────────────
# VM-Liste via IPC holen (Python für Socket-Kommunikation)
# ─────────────────────────────────────────────────────────────────────────────
get_vms() {
    python3 - <<'EOF'
import socket, struct, json, sys

SOCKET_PATH = "/run/krypt/ipc.sock"

def recv_exact(s, n):
    buf = bytearray()
    while len(buf) < n:
        chunk = s.recv(n - len(buf))
        if not chunk:
            raise ConnectionError("closed")
        buf.extend(chunk)
    return bytes(buf)

try:
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as s:
        s.settimeout(2.0)
        s.connect(SOCKET_PATH)
        msg = json.dumps({"type": "list_vms_query"}).encode()
        s.sendall(struct.pack("<I", len(msg)) + msg)
        raw_len = recv_exact(s, 4)
        length  = struct.unpack("<I", raw_len)[0]
        body    = recv_exact(s, length)
        resp    = json.loads(body)
        if resp.get("type") == "list_vms_response":
            for vm in resp.get("vms", []):
                print(f"{vm['name']}|{vm['state']}|{vm.get('trust_level','red')}")
except Exception:
    # Fallback: krypt-daemon nicht erreichbar
    sys.exit(0)
EOF
}

# ─────────────────────────────────────────────────────────────────────────────
# Rofi-Einträge bauen
# ─────────────────────────────────────────────────────────────────────────────
build_entries() {
    while IFS='|' read -r name state trust; do
        [[ -z "$name" ]] && continue
        icon=$(trust_icon "$trust")
        sicon=$(state_icon "$state")
        # Format: "ICON NAME    STATE ICON [trust]"
        printf "%s  %-20s %s  [%s]\0info\x1f%s|%s|%s\n" \
            "$icon" "$name" "$sicon" "$trust" \
            "$name" "$state" "$trust"
    done
}

# ─────────────────────────────────────────────────────────────────────────────
# Aktion ausführen
# ─────────────────────────────────────────────────────────────────────────────
handle_selection() {
    local info="$1"
    local name state trust
    IFS='|' read -r name state trust <<< "$info"

    case "$state" in
        Running)
            # Workspace wechseln je nach Trust-Level
            case "$trust" in
                red)    hyprctl dispatch workspace 1 ;;
                yellow) hyprctl dispatch workspace 2 ;;
                green)  hyprctl dispatch workspace 3 ;;
                orange) hyprctl dispatch workspace 4 ;;
                black)  hyprctl dispatch workspace 9 ;;
                *)      hyprctl dispatch workspace 1 ;;
            esac
            ;;
        Halted)
            # VM starten via IPC VmStartRequest (xl create dauert ~5–30s)
            notify-send "Krypt" "Starting $name…" --icon=dialog-information
            python3 - "$name" <<'EOF'
import socket, struct, json, sys

SOCKET_PATH = "/run/krypt/ipc.sock"
vm_name = sys.argv[1]

def recv_exact(s, n):
    buf = bytearray()
    while len(buf) < n:
        chunk = s.recv(n - len(buf))
        if not chunk:
            raise ConnectionError("closed")
        buf.extend(chunk)
    return bytes(buf)

try:
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as s:
        s.settimeout(30.0)   # xl create kann bei LUKS-Disk langsam sein
        s.connect(SOCKET_PATH)
        msg = json.dumps({"type": "vm_start_request", "vm_name": vm_name}).encode()
        s.sendall(struct.pack("<I", len(msg)) + msg)
        raw_len = recv_exact(s, 4)
        length  = struct.unpack("<I", raw_len)[0]
        body    = recv_exact(s, length)
        resp    = json.loads(body)
        if resp.get("type") == "error":
            print(f"krypt-launcher: start failed: {resp.get('message','?')}", file=sys.stderr)
            sys.exit(1)
        # VmStartResponse: vm_name + domain_id
        domid = resp.get("domain_id")
        print(f"krypt-launcher: {vm_name} started (domid={domid})")
except Exception as e:
    print(f"krypt-launcher: IPC error: {e}", file=sys.stderr)
    sys.exit(1)
EOF
            # Nach erfolgreichem Start zum richtigen Workspace wechseln
            case "$trust" in
                red)    hyprctl dispatch workspace 1 ;;
                yellow) hyprctl dispatch workspace 2 ;;
                green)  hyprctl dispatch workspace 3 ;;
                orange) hyprctl dispatch workspace 4 ;;
                black)  hyprctl dispatch workspace 9 ;;
                *)      hyprctl dispatch workspace 1 ;;
            esac
            ;;
        *)
            notify-send "Krypt" "VM $name is $state" --icon=dialog-warning
            ;;
    esac
}

# ─────────────────────────────────────────────────────────────────────────────
# Main
# ─────────────────────────────────────────────────────────────────────────────
VM_DATA=$(get_vms)

if [[ -z "$VM_DATA" ]]; then
    ENTRIES="  krypt-daemon nicht erreichbar\0info\x1fnoop"
else
    ENTRIES=$(echo "$VM_DATA" | build_entries)
fi

SELECTED=$(printf "%b" "$ENTRIES" | \
    rofi -dmenu \
        -p "󰒊 Krypt VMs" \
        -theme "$ROFI_THEME" \
        -format "i" \
        -display-columns 1 \
        -no-custom \
        2>/dev/null)

[[ -z "$SELECTED" ]] && exit 0

# info-Feld des gewählten Eintrags extrahieren
INFO=$(printf "%b" "$ENTRIES" | \
    rofi -dmenu \
        -p "󰒊 Krypt VMs" \
        -theme "$ROFI_THEME" \
        -format "info" \
        -display-columns 1 \
        -no-custom \
        2>/dev/null)

[[ -z "$INFO" || "$INFO" == "noop" ]] && exit 0

handle_selection "$INFO"
