#!/bin/bash
# build/make-test-stick.sh — USB-Stick-Image für QEMU Kill-Switch-Test
#
# Erstellt ein 1MB Raw-Image das einen Krypt-Key an Offset 512 enthält.
# Dieses Image wird via test-qemu.sh --with-stick eingebunden.
#
# Verwendung (auf dem installierten System):
#   # Option A: Stick mit existierendem LUKS-Device verknüpfen (krypt-stick)
#   sudo ./build/make-test-stick.sh --luks-dev /dev/vda2
#
#   # Option B: Nur leeres Image (für erste ISO-Tests ohne LUKS)
#   ./build/make-test-stick.sh --empty
#
# Output: build/krypt-test-stick.img
#
# QEMU-Einbindung:
#   ./build/test-qemu.sh --boot-installed --with-stick build/krypt-test-stick.img

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT="$SCRIPT_DIR/krypt-test-stick.img"

CYAN='\033[0;36m'; GREEN='\033[0;32m'; RED='\033[0;31m'; RESET='\033[0m'
log() { echo -e "${CYAN}[make-stick]${RESET} $*"; }
ok()  { echo -e "${GREEN}[make-stick]${RESET} ✓ $*"; }
die() { echo -e "${RED}[make-stick]${RESET} ✗ $*" >&2; exit 1; }

MODE="empty"
LUKS_DEV=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --luks-dev) LUKS_DEV="$2"; MODE="luks"; shift 2 ;;
        --empty)    MODE="empty"; shift ;;
        *) die "Unbekannter Parameter: $1" ;;
    esac
done

# Leeres 1MB Image
log "Erstelle Stick-Image: ${OUT} (1MB)"
dd if=/dev/zero bs=1M count=1 of="$OUT" 2>/dev/null
ok "1MB-Image erstellt"

if [[ "$MODE" == "luks" ]]; then
    [[ $EUID -eq 0 ]] || die "Root-Rechte erforderlich für --luks-dev (krypt-stick setup)"
    [[ -b "$LUKS_DEV" ]] || die "LUKS-Device nicht gefunden: $LUKS_DEV"

    log "Richte Auth-Key auf Stick + LUKS-Device ein…"
    log "  LUKS-Device: ${LUKS_DEV}"
    log "  Stick-Image: ${OUT}"

    # krypt-stick setup läuft — schreibt Key an Offset 512 + luksAddKey
    krypt-stick \
        --luks-dev "${LUKS_DEV}" \
        setup \
        --stick-dev "${OUT}" \
        --force

    ok "Auth-Stick-Image erstellt und in LUKS registriert"
    echo ""
    echo "  Starte QEMU mit Stick:"
    echo "  ./build/test-qemu.sh --boot-installed --with-stick ${OUT}"
else
    # Nur leeres Image — kein echter Key (für erste Tests ohne funktionierende Installation)
    log "Leeres Image (kein LUKS-Key). Nur für Smoke-Tests des USB-Device-Erkennungs-Codes."
    ok "Leeres Stick-Image: ${OUT}"
    echo ""
    echo "  Für echten Kill-Switch-Test:"
    echo "  sudo ./build/make-test-stick.sh --luks-dev /dev/vda2"
fi
