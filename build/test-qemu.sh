#!/bin/bash
# build/test-qemu.sh — Krypt OS QEMU Boot-Test
#
# Bootet das Krypt OS ISO (oder ein installiertes Disk-Image) in QEMU/OVMF.
# Dokumentiert den Boot-Prozess für docs/qemu-boot-log.md.
#
# Verwendung:
#   # 1. Erst ISO bauen (braucht archiso + root):
#   sudo pacman -S archiso
#   sudo ./build/build.sh --clean
#
#   # 2a. Live-ISO testen (Installer auf tty1):
#   ./build/test-qemu.sh --live
#
#   # 2b. Live-ISO + virtuelle Disk (Installer durchlaufen):
#   ./build/test-qemu.sh --install
#
#   # 2c. Installiertes System booten (nach --install):
#   ./build/test-qemu.sh --boot-installed
#
#   # 2d. Mit simuliertem USB-Stick (Kill-Switch Test):
#   ./build/test-qemu.sh --boot-installed --with-stick /pfad/zu/stick.img
#
# Flags:
#   --live             Nur ISO booten, keine virtuelle Disk
#   --install          ISO + virtuelle Disk (40GB), Installer läuft durch
#   --boot-installed   Installiertes System in DISK_IMG booten
#   --with-stick IMG   USB-Stick-Image als QEMU USB-Gerät einbinden
#   --no-kvm           KVM deaktivieren (langsamer, für Nested-Virt)
#   --log FILE         Serial-Output in FILE schreiben (default: docs/qemu-boot-log.md)
#   --headless         Kein Display-Fenster (nur serial)
#   --snapshot         Disk-Änderungen nicht persistieren (für schnelle Tests)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

# ─── Konfiguration ────────────────────────────────────────────────────────────
OVMF_CODE="/usr/share/edk2/x64/OVMF_CODE.4m.fd"
OVMF_VARS_TEMPLATE="/usr/share/edk2/x64/OVMF_VARS.4m.fd"
OVMF_VARS_COPY="/tmp/krypt-ovmf-vars.fd"

DISK_IMG="${REPO_ROOT}/build/krypt-test-disk.qcow2"
DISK_SIZE="40G"
RAM_MB=4096
CPUS=2

LOG_FILE="${REPO_ROOT}/docs/qemu-boot-log.md"

# ─── Farben ───────────────────────────────────────────────────────────────────
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
CYAN='\033[0;36m'; BOLD='\033[1m'; RESET='\033[0m'
log()  { echo -e "${CYAN}[test-qemu]${RESET} $*"; }
ok()   { echo -e "${GREEN}[test-qemu]${RESET} ✓ $*"; }
warn() { echo -e "${YELLOW}[test-qemu]${RESET} ! $*"; }
die()  { echo -e "${RED}[test-qemu]${RESET} ✗ $*" >&2; exit 1; }

# ─── Argument-Parsing ─────────────────────────────────────────────────────────
MODE="live"
STICK_IMG=""
USE_KVM=1
HEADLESS=0
SNAPSHOT=0

while [[ $# -gt 0 ]]; do
    case "$1" in
        --live)           MODE="live";          shift ;;
        --install)        MODE="install";       shift ;;
        --boot-installed) MODE="boot";          shift ;;
        --with-stick)     STICK_IMG="$2";       shift 2 ;;
        --no-kvm)         USE_KVM=0;            shift ;;
        --log)            LOG_FILE="$2";        shift 2 ;;
        --headless)       HEADLESS=1;           shift ;;
        --snapshot)       SNAPSHOT=1;           shift ;;
        *) die "Unbekannter Parameter: $1" ;;
    esac
done

# ─── Voraussetzungen prüfen ────────────────────────────────────────────────────
command -v qemu-system-x86_64 >/dev/null || die "qemu-system-x86_64 nicht gefunden"
[[ -f "$OVMF_CODE" ]]         || die "OVMF nicht gefunden: $OVMF_CODE\n  pacman -S edk2-ovmf"

# ISO finden
ISO_FILE=$(ls "${REPO_ROOT}/dist"/krypt-os-*.iso 2>/dev/null | sort -V | tail -1 || true)
if [[ -z "$ISO_FILE" && "$MODE" != "boot" ]]; then
    die "Kein ISO in dist/ — erst bauen:\n  sudo ./build/build.sh --clean"
fi

# ─── OVMF VARS kopieren (UEFI-State pro Test-Run) ─────────────────────────────
cp -f "${OVMF_VARS_TEMPLATE}" "${OVMF_VARS_COPY}"
ok "OVMF VARS: ${OVMF_VARS_COPY}"

# ─── UEFI-Firmware-Argumente ──────────────────────────────────────────────────
OVMF_ARGS=(
    -drive "if=pflash,format=raw,readonly=on,file=${OVMF_CODE}"
    -drive "if=pflash,format=raw,file=${OVMF_VARS_COPY}"
)

# ─── KVM ──────────────────────────────────────────────────────────────────────
KVM_ARGS=()
if [[ $USE_KVM -eq 1 ]] && [[ -c /dev/kvm ]]; then
    KVM_ARGS=(-enable-kvm -cpu host)
    ok "KVM aktiviert"
else
    KVM_ARGS=(-cpu qemu64)
    warn "KVM nicht verfügbar — Test wird langsamer"
fi

# ─── Display ──────────────────────────────────────────────────────────────────
DISPLAY_ARGS=()
SERIAL_ARGS=(-serial "file:${LOG_FILE}.serial" -serial stdio)
if [[ $HEADLESS -eq 1 ]]; then
    DISPLAY_ARGS=(-display none)
    SERIAL_ARGS=(-serial stdio -serial "file:${LOG_FILE}.serial")
else
    DISPLAY_ARGS=(-display gtk,zoom-to-fit=on)
fi

# ─── Netzwerk ─────────────────────────────────────────────────────────────────
# User-Mode Networking (kein root nötig, langsamerer als Tap)
NET_ARGS=(
    -netdev user,id=net0,hostfwd=tcp::2222-:22
    -device virtio-net-pci,netdev=net0
)

# ─── USB-Controller + optionaler Stick ────────────────────────────────────────
USB_ARGS=(-device nec-usb-xhci,id=xhci0)
if [[ -n "$STICK_IMG" ]]; then
    [[ -f "$STICK_IMG" ]] || die "USB-Stick-Image nicht gefunden: $STICK_IMG"
    USB_ARGS+=(
        -drive "file=${STICK_IMG},format=raw,if=none,id=usbstick0"
        -device "usb-storage,drive=usbstick0,bus=xhci0.0"
    )
    ok "USB-Stick: ${STICK_IMG}"
fi

# ─── Virtuelle Disk ──────────────────────────────────────────────────────────
DISK_ARGS=()
SNAPSHOT_FLAG=()
if [[ "$MODE" != "live" ]]; then
    if [[ "$MODE" == "install" || ! -f "$DISK_IMG" ]]; then
        log "Erstelle virtuelle Disk: ${DISK_IMG} (${DISK_SIZE})"
        qemu-img create -f qcow2 "${DISK_IMG}" "${DISK_SIZE}"
        ok "Disk erstellt"
    fi
    [[ $SNAPSHOT -eq 1 ]] && SNAPSHOT_FLAG=(-snapshot)
    DISK_ARGS=(
        "${SNAPSHOT_FLAG[@]}"
        -drive "file=${DISK_IMG},format=qcow2,if=none,id=disk0"
        -device "virtio-blk-pci,drive=disk0"
    )
fi

# ─── Boot-Modus ───────────────────────────────────────────────────────────────
case "$MODE" in
    live|install)
        BOOT_ARGS=(-cdrom "${ISO_FILE}" -boot order=dc)
        log "Modus: ${MODE} — ISO: $(basename "${ISO_FILE}")"
        ;;
    boot)
        [[ -f "$DISK_IMG" ]] || die "Disk-Image nicht gefunden: $DISK_IMG\n  Erst: ./build/test-qemu.sh --install"
        BOOT_ARGS=(-boot order=c)
        log "Modus: boot — Disk: $(basename "${DISK_IMG}")"
        ;;
esac

# ─── Zusammenfassung ──────────────────────────────────────────────────────────
echo ""
echo -e "  ${BOLD}Krypt OS — QEMU Boot-Test${RESET}"
echo ""
printf "  %-12s %s\n" "Modus:"   "${MODE}"
printf "  %-12s %s\n" "RAM:"     "${RAM_MB}MB"
printf "  %-12s %s\n" "CPUs:"    "${CPUS}"
[[ -n "$ISO_FILE" ]] && printf "  %-12s %s\n" "ISO:" "$(basename "${ISO_FILE}")"
[[ ${#DISK_ARGS[@]} -gt 0 ]] && printf "  %-12s %s\n" "Disk:" "${DISK_IMG}"
[[ -n "$STICK_IMG" ]] && printf "  %-12s %s\n" "USB-Stick:" "${STICK_IMG}"
printf "  %-12s %s\n" "Serial-Log:" "${LOG_FILE}.serial"
echo ""

# Serial-Log-Header schreiben
{
    echo "# Krypt OS QEMU Serial Log"
    echo "# Datum: $(date -Iseconds)"
    echo "# Modus: ${MODE}"
    echo "# Befehl: $0 $*"
    echo "---"
    echo ""
} > "${LOG_FILE}.serial"

# ─── QEMU starten ────────────────────────────────────────────────────────────
log "Starte QEMU…"
echo ""

qemu-system-x86_64 \
    -name "Krypt OS" \
    -m "${RAM_MB}M" \
    -smp "${CPUS}" \
    "${KVM_ARGS[@]}" \
    "${OVMF_ARGS[@]}" \
    "${DISPLAY_ARGS[@]}" \
    "${SERIAL_ARGS[@]}" \
    "${NET_ARGS[@]}" \
    "${USB_ARGS[@]}" \
    "${DISK_ARGS[@]}" \
    "${BOOT_ARGS[@]}" \
    -machine q35 \
    -device virtio-vga \
    -audiodev none,id=noaudio \
    -monitor none \
    2>&1

QEMU_EXIT=$?
echo ""
if [[ $QEMU_EXIT -eq 0 ]]; then
    ok "QEMU beendet (normal)"
else
    warn "QEMU beendet mit Exit-Code: ${QEMU_EXIT}"
fi

echo ""
log "Serial-Log gespeichert: ${LOG_FILE}.serial"
echo ""
