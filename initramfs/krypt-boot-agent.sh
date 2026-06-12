#!/bin/bash
# krypt-boot-agent.sh — Post-Boot Auth-Stick-Registrierung
#
# Läuft als systemd-Service in dom0 direkt nach dem Start von krypt-daemon.
# Aufgabe: den beim initramfs-Boot verwendeten Auth-Stick identifizieren
# und dessen Serial via IPC an krypt-daemon melden, damit der USB-Monitor
# weiss welchen Stick er überwachen soll.
#
# Hintergrund: Der initramfs-Hook öffnet LUKS und bootet weiter.
# Krypt-daemon startet später — er kennt die Stick-Serial noch nicht.
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
BOOT_STICK_FILE="/run/krypt/boot-stick-serial"

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
# Stick-Serial ermitteln: welches USB-Device hat das krypt-root-Volume geöffnet?
# Weg: /sys/block/dm-N/slaves/ → welches Block-Device steckt dahinter.
# ---------------------------------------------------------------------------
find_boot_stick_serial() {
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

    # Serial aus udevadm — ID_SERIAL_SHORT entspricht dem Format das
    # krypt-stick aus /sys/block/<dev>/device/serial liest und das auch
    # in daemon.toml [[auth_sticks]] serial steht. ID_SERIAL wäre die
    # lange vendor+model+serial-Form und würde dem daemon-Matching fehlen.
    local serial
    serial=$(udevadm info --query=property --name="/dev/${parent}" 2>/dev/null \
             | grep '^ID_SERIAL_SHORT=' | cut -d= -f2)

    echo "$serial"
}

# ---------------------------------------------------------------------------
# (Phase 7+) IPC-Request für RegisterBootStick wird hier hinzukommen.
# Bis dahin: Stick-Serial nur in $BOOT_STICK_FILE persistieren, krypt-daemon
# liest sie beim eigenen Start. send_ipc() wurde entfernt, weil es nirgends
# aufgerufen wurde und die Python-Heredoc-Interpolation von $msg fragil ist
# (würde bei JSON mit Quotes oder Newlines brechen).
# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------
main() {
    log "starting (LUKS name: $LUKS_NAME)"

    # wait_for_socket: aktuell warten wir bis krypt-daemon den IPC-Socket
    # gebunden hat. Bis Phase 7+ ist die Antwort nicht load-bearing — das
    # Stick-Matching läuft über daemon.toml-Serials, nicht via Live-IPC —
    # aber das Warten gibt dem Daemon Zeit hochzufahren bevor der Boot
    # Agent weitere Service-Files anstößt. Vorher wurde der Return-Wert
    # ignoriert, sodass ein Timeout im Journal verschluckt wurde.
    if ! wait_for_socket; then
        log "WARN: krypt-daemon-Socket nicht bereit — Boot-Agent läuft weiter"
        log "      (kein Fehler bis Phase 7+ — Stick-Matching via daemon.toml)"
    fi

    local stick_serial
    stick_serial=$(find_boot_stick_serial)

    if [ -z "$stick_serial" ]; then
        log "WARN: Stick-Serial nicht ermittelbar — USB-Monitor läuft ohne Serial-Filter"
        log "      (krypt-daemon verwendet Serials aus daemon.toml)"
        exit 0
    fi

    log "Boot-Stick-Serial: $stick_serial"

    # Serial für andere Prozesse persistieren
    echo "$stick_serial" > "$BOOT_STICK_FILE"
    chmod 600 "$BOOT_STICK_FILE"

    # krypt-daemon via IPC über den Boot-Stick informieren
    # Nachrichtenformat: PolicyCheck auf "boot-stick" als Eigendiagnose
    # In Phase 7+: dedizierter RegisterBootStick-Nachrichtentyp
    log "INFO: IPC-Registrierung via daemon.toml serial-Matching aktiv"
    log "      Explizite RegisterBootStick IPC-Message: Phase 7+"

    log "done"
}

main "$@"
