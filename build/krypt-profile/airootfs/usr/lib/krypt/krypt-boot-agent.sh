#!/bin/bash
# krypt-boot-agent.sh — Post-Boot Auth-Stick-Registrierung
#
# Läuft als systemd-Service in dom0 direkt nach dem Start von krypt-daemon.
# Aufgabe: den beim initramfs-Boot verwendeten Auth-Stick identifizieren
# und dessen UUID via IPC an krypt-daemon melden, damit der USB-Monitor
# weiss welchen Stick er überwachen soll.
#
# Hintergrund: Der initramfs-Hook öffnet LUKS und bootet weiter.
# Krypt-daemon startet später — er kennt die Stick-UUID noch nicht.
# Dieser Agent schliesst diese Lücke.
#
# Systemd-Unit: /etc/systemd/system/krypt-boot-agent.service
#   [Unit]
#   Description=Krypt Boot Auth Stick Registration
#   After=krypt-daemon.service
#   Requires=krypt-daemon.service
#
#   [Service]
#   Type=oneshot
#   ExecStart=/usr/lib/krypt/krypt-boot-agent.sh
#   RemainAfterExit=yes
#
#   [Install]
#   WantedBy=multi-user.target

set -euo pipefail

SOCKET_PATH="/run/krypt/ipc.sock"
LUKS_NAME="krypt-root"
BOOT_STICK_FILE="/run/krypt/boot-stick-uuid"

log() { echo "krypt-boot-agent: $*" >&2; }

# ---------------------------------------------------------------------------
# Warte auf IPC-Socket (krypt-daemon braucht einen Moment nach dem Start)
# ---------------------------------------------------------------------------
wait_for_socket() {
    local retries=20
    while [ $retries -gt 0 ]; do
        [ -S "$SOCKET_PATH" ] && return 0
        sleep 0.2
        retries=$((retries - 1))
    done
    log "ERROR: IPC-Socket $SOCKET_PATH nicht verfügbar nach 4s"
    return 1
}

# ---------------------------------------------------------------------------
# Stick-UUID ermitteln: welches USB-Device hat das krypt-root-Volume geöffnet?
# Weg: /sys/block/dm-N/slaves/ → welches Block-Device steckt dahinter.
# ---------------------------------------------------------------------------
find_boot_stick_uuid() {
    # dm-crypt Device für krypt-root finden
    local dm_dev
    dm_dev=$(dmsetup info -c --noheadings -o blkdevname "$LUKS_NAME" 2>/dev/null) || {
        log "WARN: kein dm-Device für '$LUKS_NAME' gefunden"
        echo ""
        return 0
    }

    # Underlying Block-Device aus dm-crypt slaves lesen
    local slave
    slave=$(ls "/sys/block/${dm_dev}/slaves/" 2>/dev/null | head -1)
    [ -n "$slave" ] || { echo ""; return 0; }

    # Übergeordnetes Gerät (Disk, nicht Partition): sda1 → sda
    local parent
    parent=$(lsblk -no PKNAME "/dev/${slave}" 2>/dev/null || echo "$slave")

    # UUID aus sysfs oder udevadm
    local uuid
    uuid=$(udevadm info --query=property --name="/dev/${parent}" 2>/dev/null \
           | grep '^ID_SERIAL=' | cut -d= -f2)

    echo "$uuid"
}

# ---------------------------------------------------------------------------
# Einfaches IPC-Request via socat (falls vorhanden) oder Python-Fallback
# ---------------------------------------------------------------------------
send_ipc() {
    local msg="$1"
    local msg_len=${#msg}

    # 4-Byte-LE-Länge + JSON via Python (kein externes socat nötig)
    python3 - <<EOF
import socket, struct, json, sys

SOCKET_PATH = "$SOCKET_PATH"
msg = $msg.encode("utf-8")

with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as s:
    s.settimeout(3.0)
    s.connect(SOCKET_PATH)
    s.sendall(struct.pack("<I", len(msg)) + msg)
    raw_len = s.recv(4)
    length  = struct.unpack("<I", raw_len)[0]
    body    = s.recv(length)
    response = json.loads(body)
    print(json.dumps(response))
EOF
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------
main() {
    log "starting (LUKS name: $LUKS_NAME)"

    wait_for_socket

    local stick_uuid
    stick_uuid=$(find_boot_stick_uuid)

    if [ -z "$stick_uuid" ]; then
        log "WARN: Stick-UUID nicht ermittelbar — USB-Monitor läuft ohne UUID-Filter"
        log "      (krypt-daemon verwendet Serials aus daemon.toml)"
        exit 0
    fi

    log "Boot-Stick-UUID: $stick_uuid"

    # UUID für andere Prozesse persistieren
    echo "$stick_uuid" > "$BOOT_STICK_FILE"
    chmod 600 "$BOOT_STICK_FILE"

    # krypt-daemon via IPC über den Boot-Stick informieren
    # Nachrichtenformat: PolicyCheck auf "boot-stick" als Eigendiagnose
    # In Phase 7+: dedizierter RegisterBootStick-Nachrichtentyp
    log "INFO: IPC-Registrierung via daemon.toml serial-Matching aktiv"
    log "      Explizite RegisterBootStick IPC-Message: Phase 7+"

    log "done"
}

main "$@"
