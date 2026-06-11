#!/bin/bash
# dotfiles/install.sh — Krypt OS Dotfiles Installer
#
# Erstellt Symlinks von diesem Repo nach ~/.config/ (und andere Ziele).
# Ist idempotent — mehrfaches Ausführen macht nichts kaputt.
#
# Verwendung:
#   cd ~/krypt-os && ./dotfiles/install.sh
#   cd ~/krypt-os && ./dotfiles/install.sh --dry-run   # zeigt was passieren würde
#   cd ~/krypt-os && ./dotfiles/install.sh --force      # überschreibt existierende Dateien

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

DRY_RUN=0
FORCE=0

while [[ $# -gt 0 ]]; do
    case "$1" in
        --dry-run) DRY_RUN=1; shift ;;
        --force)   FORCE=1;   shift ;;
        *) echo "Unbekannt: $1"; exit 1 ;;
    esac
done

# ─────────────────────────────────────────────────────────────────────────────
# Helpers
# ─────────────────────────────────────────────────────────────────────────────
GREEN='\033[0;32m'; YELLOW='\033[1;33m'; RED='\033[0;31m'
CYAN='\033[0;36m'; DIM='\033[2m'; RESET='\033[0m'

ok()   { echo -e "${GREEN}  ✓${RESET} $*"; }
skip() { echo -e "${DIM}  -${RESET} $*"; }
warn() { echo -e "${YELLOW}  !${RESET} $*"; }
info() { echo -e "${CYAN}  →${RESET} $*"; }
err()  { echo -e "${RED}  ✗${RESET} $*" >&2; }

do_link() {
    local src="$1"
    local dst="$2"

    if [[ $DRY_RUN -eq 1 ]]; then
        info "[dry-run] $dst → $src"
        return
    fi

    # Ziel-Verzeichnis anlegen
    mkdir -p "$(dirname "$dst")"

    if [[ -L "$dst" ]]; then
        # Bereits ein Symlink — prüfen ob er auf das Richtige zeigt
        if [[ "$(readlink -f "$dst")" == "$(readlink -f "$src")" ]]; then
            skip "$dst (bereits verlinkt)"
            return
        else
            warn "$dst zeigt auf falsches Ziel — ersetze"
        fi
    elif [[ -e "$dst" ]]; then
        if [[ $FORCE -eq 1 ]]; then
            warn "$dst existiert — --force: wird ersetzt"
            local backup="${dst}.krypt-backup-$(date +%Y%m%d%H%M%S)"
            mv "$dst" "$backup"
            warn "  Backup: $backup"
        else
            err "$dst existiert bereits (kein Symlink). Nutze --force um zu ersetzen."
            return 1
        fi
    fi

    ln -sf "$src" "$dst"
    ok "$dst"
}

# ─────────────────────────────────────────────────────────────────────────────
# SYMLINK-TABELLE
# Format: do_link "<Quelle im Repo>" "<Ziel im Filesystem>"
# ─────────────────────────────────────────────────────────────────────────────

echo ""
echo "  ██╗  ██╗██████╗ ██╗   ██╗██████╗ ████████╗"
echo "  ██║ ██╔╝██╔══██╗╚██╗ ██╔╝██╔══██╗╚══██╔══╝"
echo "  █████╔╝ ██████╔╝ ╚████╔╝ ██████╔╝   ██║   "
echo "  ██╔═██╗ ██╔══██╗  ╚██╔╝  ██╔═══╝    ██║   "
echo "  ██║  ██╗██║  ██║   ██║   ██║         ██║   "
echo "  ╚═╝  ╚═╝╚═╝  ╚═╝   ╚═╝   ╚═╝         ╚═╝  "
echo ""
echo "  Dotfiles Installer${DRY_RUN:+ (DRY-RUN)}"
echo ""

# ── Hyprland ────────────────────────────────────────────────────────────────
info "Hyprland"
do_link "${SCRIPT_DIR}/hyprland/hyprland.conf"  "${HOME}/.config/hypr/hyprland.conf"
do_link "${SCRIPT_DIR}/hyprland/animations.conf" "${HOME}/.config/hypr/animations.conf"

# ── Hyprlock ────────────────────────────────────────────────────────────────
info "Hyprlock"
do_link "${SCRIPT_DIR}/hyprlock/hyprlock.conf"  "${HOME}/.config/hypr/hyprlock.conf"

# ── Waybar ──────────────────────────────────────────────────────────────────
info "Waybar"
do_link "${SCRIPT_DIR}/waybar/config.jsonc"     "${HOME}/.config/waybar/config.jsonc"
do_link "${SCRIPT_DIR}/waybar/style.css"        "${HOME}/.config/waybar/style.css"
do_link "${SCRIPT_DIR}/waybar/krypt-vms.py"     "${HOME}/.config/waybar/krypt-vms.py"

# ── Rofi ────────────────────────────────────────────────────────────────────
# Exec-Bit der Quellen wird aus dem git-Index getragen — kein chmod nötig.
info "Rofi"
do_link "${SCRIPT_DIR}/rofi/krypt.rasi"         "${HOME}/.config/rofi/krypt.rasi"
do_link "${SCRIPT_DIR}/rofi/krypt-launcher.sh"  "${HOME}/.config/rofi/krypt-launcher.sh"

# ── Foot (Terminal) ─────────────────────────────────────────────────────────
info "Foot"
do_link "${SCRIPT_DIR}/foot/foot.ini"           "${HOME}/.config/foot/foot.ini"

# ── Krypt Theme ─────────────────────────────────────────────────────────────
info "Theme"
do_link "${SCRIPT_DIR}/theme/colors.conf"       "${HOME}/.config/krypt/colors.conf"

# ── Neovim ──────────────────────────────────────────────────────────────────
info "Neovim"
do_link "${SCRIPT_DIR}/neovim"                  "${HOME}/.config/nvim"

# ── GRUB Theme ──────────────────────────────────────────────────────────────
# GRUB-Dateien gehen nach /boot — Root nötig
info "GRUB"
if [[ $EUID -eq 0 ]]; then
    mkdir -p /boot/grub/themes
    do_link "${SCRIPT_DIR}/grub/krypt-grub"  "/boot/grub/themes/krypt-grub"
    echo ""
    warn "GRUB-Config aktualisieren:"
    warn "  echo 'GRUB_THEME=/boot/grub/themes/krypt-grub/theme.txt' >> /etc/default/grub"
    warn "  grub-mkconfig -o /boot/grub/grub.cfg"
else
    skip "GRUB-Theme: Root erforderlich (sudo ./dotfiles/install.sh für GRUB)"
fi

# ── Plymouth ────────────────────────────────────────────────────────────────
info "Plymouth"
if [[ $EUID -eq 0 ]]; then
    mkdir -p /usr/share/plymouth/themes
    do_link "${SCRIPT_DIR}/plymouth/krypt"  "/usr/share/plymouth/themes/krypt"
    echo ""
    warn "Plymouth aktivieren:"
    warn "  plymouth-set-default-theme krypt"
    warn "  mkinitcpio -P"
else
    skip "Plymouth-Theme: Root erforderlich"
fi

# ─────────────────────────────────────────────────────────────────────────────
echo ""
if [[ $DRY_RUN -eq 1 ]]; then
    echo -e "${YELLOW}  DRY-RUN: keine Änderungen vorgenommen.${RESET}"
    echo "  Ohne --dry-run ausführen um zu installieren."
else
    echo -e "${GREEN}  Dotfiles installiert.${RESET}"
    echo ""
    echo "  Waybar neu starten:   killall waybar && waybar &"
    echo "  Hyprland neu laden:   hyprctl reload"
fi
echo ""
