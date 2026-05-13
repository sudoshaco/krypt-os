#!/bin/bash
# build/build.sh — Krypt OS ISO Builder (lokal, Arch Linux)
#
# Voraussetzungen:
#   pacman -S archiso xorriso mtools dosfstools
#   rustup (https://rustup.rs) — für cargo build --release
#   Muss als root ausgeführt werden (mkarchiso erfordert root)
#
# Verwendung:
#   sudo ./build/build.sh [--clean] [--output /pfad/zu/dist/] [--skip-rust]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

PROFILE_DIR="${SCRIPT_DIR}/krypt-profile"
WORK_DIR="${TMPDIR:-/tmp}/krypt-iso-work"
OUT_DIR="${REPO_ROOT}/dist"
ARCHISO_RELENG="/usr/share/archiso/configs/releng"
KRYPT_VERSION="${KRYPT_VERSION:-$(git -C "${REPO_ROOT}" describe --tags --always 2>/dev/null || echo "dev")}"

# ─────────────────────────────────────────────────────────────────────────────
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
CYAN='\033[0;36m'; BOLD='\033[1m'; RESET='\033[0m'
log()  { echo -e "${CYAN}[krypt]${RESET} $*"; }
ok()   { echo -e "${GREEN}[krypt]${RESET} ✓ $*"; }
warn() { echo -e "${YELLOW}[krypt]${RESET} ! $*"; }
die()  { echo -e "${RED}[krypt]${RESET} ✗ $*" >&2; exit 1; }
# ─────────────────────────────────────────────────────────────────────────────

CLEAN=0
SKIP_RUST=0
while [[ $# -gt 0 ]]; do
    case "$1" in
        --clean)     CLEAN=1;         shift ;;
        --output)    OUT_DIR="$2";    shift 2 ;;
        --skip-rust) SKIP_RUST=1;     shift ;;
        *)           die "Unbekannter Parameter: $1" ;;
    esac
done

echo ""
echo -e "  ${BOLD}██╗  ██╗██████╗ ██╗   ██╗██████╗ ████████╗${RESET}"
echo -e "  ${BOLD}██║ ██╔╝██╔══██╗╚██╗ ██╔╝██╔══██╗╚══██╔══╝${RESET}"
echo -e "  ${BOLD}█████╔╝ ██████╔╝ ╚████╔╝ ██████╔╝   ██║   ${RESET}"
echo -e "  ${BOLD}██╔═██╗ ██╔══██╗  ╚██╔╝  ██╔═══╝    ██║   ${RESET}"
echo -e "  ${BOLD}██║  ██╗██║  ██║   ██║   ██║         ██║   ${RESET}"
echo -e "  ${BOLD}╚═╝  ╚═╝╚═╝  ╚═╝   ╚═╝   ╚═╝         ╚═╝  ${RESET}"
echo ""
echo -e "  ISO Builder  ${CYAN}${KRYPT_VERSION}${RESET}"
echo ""

# ── Voraussetzungen ───────────────────────────────────────────────────────────
[[ $EUID -eq 0 ]] || die "Root-Rechte erforderlich (sudo ./build/build.sh)"
command -v mkarchiso  >/dev/null || die "archiso nicht installiert (pacman -S archiso)"
command -v xorriso    >/dev/null || die "xorriso nicht installiert (pacman -S xorriso)"
command -v mksquashfs >/dev/null || die "squashfs-tools nicht installiert (pacman -S squashfs-tools)"

# ── Cleanup ───────────────────────────────────────────────────────────────────
if [[ $CLEAN -eq 1 ]]; then
    log "Clean: entferne ${PROFILE_DIR} und ${WORK_DIR}"
    rm -rf "${PROFILE_DIR}" "${WORK_DIR}"
fi

# ── Rust workspace kompilieren ────────────────────────────────────────────────
if [[ $SKIP_RUST -eq 0 ]]; then
    log "Rust workspace kompilieren (release)…"

    if [[ -n "${CARGO_HOME:-}" ]] && command -v cargo >/dev/null 2>&1; then
        CARGO_CMD="cargo"
    elif command -v cargo >/dev/null 2>&1; then
        CARGO_CMD="cargo"
    elif [[ -f "/usr/local/cargo/bin/cargo" ]]; then
        CARGO_CMD="/usr/local/cargo/bin/cargo"
    else
        die "cargo nicht gefunden. Rust installieren: https://rustup.rs"
    fi

    # Als normaler User bauen (root-cargo vermeiden), dann Binaries kopieren
    BUILD_USER="${SUDO_USER:-root}"
    if [[ "${BUILD_USER}" != "root" ]]; then
        sudo -u "${BUILD_USER}" \
            env HOME="$(getent passwd "${BUILD_USER}" | cut -d: -f6)" \
            bash -c "source ~/.cargo/env 2>/dev/null || true; \
                     cargo build --release --manifest-path '${REPO_ROOT}/Cargo.toml'" \
            2>&1 | grep -E "Compiling|Finished|error" || true
    else
        ${CARGO_CMD} build --release --manifest-path "${REPO_ROOT}/Cargo.toml" \
            2>&1 | grep -E "Compiling|Finished|error" || true
    fi

    DAEMON_BIN="${REPO_ROOT}/target/release/krypt-daemon"
    GUI_BIN="${REPO_ROOT}/target/release/krypt-gui"
    STICK_BIN="${REPO_ROOT}/target/release/krypt-stick"

    [[ -f "${DAEMON_BIN}" ]] || die "krypt-daemon binary nicht gefunden nach cargo build"
    [[ -f "${STICK_BIN}" ]] || die "krypt-stick binary nicht gefunden nach cargo build"
    ok "Rust release builds fertig"
else
    warn "--skip-rust: überspringe cargo build"
    DAEMON_BIN="${REPO_ROOT}/target/release/krypt-daemon"
    GUI_BIN="${REPO_ROOT}/target/release/krypt-gui"
    STICK_BIN="${REPO_ROOT}/target/release/krypt-stick"
fi

# ── Archiso-Profil aufbauen ───────────────────────────────────────────────────
log "ISO-Profil erstellen: ${PROFILE_DIR}"
rm -rf "${PROFILE_DIR}"
cp -r "${ARCHISO_RELENG}" "${PROFILE_DIR}"

# profiledef.sh — Krypt-spezifische ISO-Metadaten
cat > "${PROFILE_DIR}/profiledef.sh" <<EOF
#!/usr/bin/env bash
# Krypt OS ISO Profile
iso_name="krypt-os"
iso_label="KRYPT_OS_$(date +%Y%m)"
iso_publisher="Krypt OS Project"
iso_application="Krypt OS — Cryptography-first Linux"
iso_version="${KRYPT_VERSION}"
install_dir="arch"
buildmodes=('iso')
bootmodes=('bios.syslinux' 'uefi.systemd-boot')
arch="x86_64"
pacman_conf="pacman.conf"
airootfs_image_type="squashfs"
airootfs_image_tool_options=('-comp' 'xz' '-Xbcj' 'x86' '-b' '1M' '-Xdict-size' '1M')
file_permissions=(
  ["/etc/shadow"]="0:0:400"
  ["/etc/gshadow"]="0:0:400"
  ["/usr/bin/krypt-daemon"]="0:0:755"
  ["/usr/bin/krypt-stick"]="0:0:755"
  ["/usr/bin/krypt-gui"]="0:0:755"
  ["/usr/lib/krypt/krypt-boot-agent.sh"]="0:0:755"
  ["/usr/share/krypt-installer/main.py"]="0:0:755"
)
EOF
chmod +x "${PROFILE_DIR}/profiledef.sh"

# Paketliste
cat "${SCRIPT_DIR}/packages.x86_64" >> "${PROFILE_DIR}/packages.x86_64"
ok "Pakete: $(wc -l < "${SCRIPT_DIR}/packages.x86_64") Krypt-Pakete ergänzt"

# ── airootfs-Overlays ─────────────────────────────────────────────────────────
log "airootfs-Overlays einbinden…"

# Krypt-eigene airootfs-Basis
[[ -d "${SCRIPT_DIR}/airootfs" ]] && \
    cp -rT "${SCRIPT_DIR}/airootfs" "${PROFILE_DIR}/airootfs"

# ── initramfs-Hook ────────────────────────────────────────────────────────────
install -Dm644 "${REPO_ROOT}/initramfs/hooks/krypt" \
    "${PROFILE_DIR}/airootfs/etc/initcpio/hooks/krypt"
install -Dm644 "${REPO_ROOT}/initramfs/install/krypt" \
    "${PROFILE_DIR}/airootfs/etc/initcpio/install/krypt"
install -Dm755 "${REPO_ROOT}/initramfs/krypt-boot-agent.sh" \
    "${PROFILE_DIR}/airootfs/usr/lib/krypt/krypt-boot-agent.sh"

# ── systemd-Units ─────────────────────────────────────────────────────────────
install -Dm644 "${REPO_ROOT}/init/krypt-daemon.service" \
    "${PROFILE_DIR}/airootfs/etc/systemd/system/krypt-daemon.service"
install -Dm644 "${REPO_ROOT}/init/krypt-boot-agent.service" \
    "${PROFILE_DIR}/airootfs/etc/systemd/system/krypt-boot-agent.service"

# Units aktivieren
mkdir -p "${PROFILE_DIR}/airootfs/etc/systemd/system/multi-user.target.wants"
ln -sf "/etc/systemd/system/krypt-daemon.service" \
    "${PROFILE_DIR}/airootfs/etc/systemd/system/multi-user.target.wants/krypt-daemon.service"

# ── Rust-Binaries ─────────────────────────────────────────────────────────────
log "Rust-Binaries einbinden…"
mkdir -p "${PROFILE_DIR}/airootfs/usr/bin"
install -Dm755 "${DAEMON_BIN}"  "${PROFILE_DIR}/airootfs/usr/bin/krypt-daemon"
[[ -f "${GUI_BIN}" ]]   && install -Dm755 "${GUI_BIN}"   "${PROFILE_DIR}/airootfs/usr/bin/krypt-gui"
[[ -f "${STICK_BIN}" ]] && install -Dm755 "${STICK_BIN}" "${PROFILE_DIR}/airootfs/usr/bin/krypt-stick"
ok "Binaries eingebunden"

# ── Installer ─────────────────────────────────────────────────────────────────
log "TUI-Installer einbinden…"
INST_DST="${PROFILE_DIR}/airootfs/usr/share/krypt-installer"
mkdir -p "${INST_DST}/steps"
install -Dm755 "${REPO_ROOT}/installer/main.py"               "${INST_DST}/main.py"
cp            "${REPO_ROOT}/installer/steps/"*.py             "${INST_DST}/steps/"
install -Dm644 "${REPO_ROOT}/installer/requirements.txt"      "${INST_DST}/requirements.txt"
# __init__.py sicherstellen
touch "${INST_DST}/steps/__init__.py"

# Wrapper-Script
cat > "${PROFILE_DIR}/airootfs/usr/bin/krypt-install" <<'WRAPPER'
#!/bin/bash
# Krypt OS TUI-Installer
cd /usr/share/krypt-installer
exec python3 main.py "$@"
WRAPPER
chmod +x "${PROFILE_DIR}/airootfs/usr/bin/krypt-install"
ok "Installer eingebunden"

# ── GRUB-Theme ────────────────────────────────────────────────────────────────
log "GRUB-Theme einbinden…"
GRUB_THEME_SRC="${REPO_ROOT}/dotfiles/grub/krypt-grub"
if [[ -d "${GRUB_THEME_SRC}" ]]; then
    GRUB_THEME_DST="${PROFILE_DIR}/airootfs/boot/grub/themes/krypt-grub"
    mkdir -p "${GRUB_THEME_DST}"
    cp -r "${GRUB_THEME_SRC}/." "${GRUB_THEME_DST}/"
    # GRUB-Config mit Theme
    mkdir -p "${PROFILE_DIR}/airootfs/etc/default"
    cat >> "${PROFILE_DIR}/airootfs/etc/default/grub" <<'GRUBCONF'

# Krypt OS GRUB-Theme
GRUB_THEME="/boot/grub/themes/krypt-grub/theme.txt"
GRUB_TIMEOUT=5
GRUB_TIMEOUT_STYLE=menu
GRUB_DISTRIBUTOR="Krypt OS"
GRUBCONF
    ok "GRUB-Theme eingebunden"
else
    warn "GRUB-Theme nicht gefunden unter ${GRUB_THEME_SRC} — überspringe"
fi

# ── Plymouth-Theme ────────────────────────────────────────────────────────────
log "Plymouth-Theme einbinden…"
PLYMOUTH_SRC="${REPO_ROOT}/dotfiles/plymouth/krypt"
if [[ -d "${PLYMOUTH_SRC}" ]]; then
    PLYMOUTH_DST="${PROFILE_DIR}/airootfs/usr/share/plymouth/themes/krypt"
    mkdir -p "${PLYMOUTH_DST}"
    cp -r "${PLYMOUTH_SRC}/." "${PLYMOUTH_DST}/"
    # Plymouth default setzen
    mkdir -p "${PROFILE_DIR}/airootfs/etc/plymouth"
    cat > "${PROFILE_DIR}/airootfs/etc/plymouth/plymouthd.conf" <<'PLYMOUTHCONF'
[Daemon]
Theme=krypt
ShowDelay=0
PLYMOUTHCONF
    ok "Plymouth-Theme eingebunden"
else
    warn "Plymouth-Theme nicht gefunden unter ${PLYMOUTH_SRC} — überspringe"
fi

# ── Dotfiles für sys-gui (in /etc/skel) ───────────────────────────────────────
log "Dotfiles für sys-gui einbinden…"
SKEL="${PROFILE_DIR}/airootfs/etc/skel"
mkdir -p "${SKEL}/.config"
# Neovim
[[ -d "${REPO_ROOT}/dotfiles/neovim" ]] && \
    cp -rT "${REPO_ROOT}/dotfiles/neovim" "${SKEL}/.config/nvim"
# Hyprland
[[ -d "${REPO_ROOT}/dotfiles/hyprland" ]] && \
    cp -rT "${REPO_ROOT}/dotfiles/hyprland" "${SKEL}/.config/hypr"
# Waybar
if [[ -d "${REPO_ROOT}/dotfiles/waybar" ]]; then
    cp -rT "${REPO_ROOT}/dotfiles/waybar" "${SKEL}/.config/waybar"
    chmod +x "${SKEL}/.config/waybar/krypt-vms.py" 2>/dev/null || true
fi
# Rofi
if [[ -d "${REPO_ROOT}/dotfiles/rofi" ]]; then
    cp -rT "${REPO_ROOT}/dotfiles/rofi" "${SKEL}/.config/rofi"
    chmod +x "${SKEL}/.config/rofi/krypt-launcher.sh" 2>/dev/null || true
fi
# Foot
[[ -d "${REPO_ROOT}/dotfiles/foot" ]] && \
    cp -rT "${REPO_ROOT}/dotfiles/foot" "${SKEL}/.config/foot"
# Krypt-Theme
[[ -d "${REPO_ROOT}/dotfiles/theme" ]] && {
    mkdir -p "${SKEL}/.config/krypt"
    cp -rT "${REPO_ROOT}/dotfiles/theme" "${SKEL}/.config/krypt"
}
ok "Dotfiles eingebunden (nvim, hyprland, waybar, rofi, foot)"

# ── Daemon-Config ─────────────────────────────────────────────────────────────
# daemon.toml ist bereits über airootfs-Copy oben eingebunden.
# daemon.toml.example als Referenz-Kopie separat installieren.
if [[ -f "${REPO_ROOT}/vm-daemon/daemon.toml.example" ]]; then
    install -Dm644 "${REPO_ROOT}/vm-daemon/daemon.toml.example" \
        "${PROFILE_DIR}/airootfs/etc/krypt/daemon.toml.example"
fi

# ── Verzeichnisse für AppVM-Images und Schlüssel vorbereiten ─────────────────
# Diese Verzeichnisse werden auf dem installierten System befüllt.
# Im Live-ISO sind sie leer — der Installer legt Inhalte in /mnt/ an.
mkdir -p "${PROFILE_DIR}/airootfs/var/lib/krypt/vms"
mkdir -p "${PROFILE_DIR}/airootfs/etc/krypt/keys"
chmod 700 "${PROFILE_DIR}/airootfs/var/lib/krypt/vms"
chmod 700 "${PROFILE_DIR}/airootfs/etc/krypt/keys"

# ── krypt-vm-open Wrapper ─────────────────────────────────────────────────────
log "krypt-vm-open Wrapper installieren…"
cat > "${PROFILE_DIR}/airootfs/usr/local/bin/krypt-vm-open" <<'VMOPEN'
#!/bin/bash
# krypt-vm-open — LUKS-Mapping öffnen + Xen-VM starten
# Usage: krypt-vm-open <vm-name>
set -euo pipefail

VM="${1:?Verwendung: krypt-vm-open <vm-name>}"
KEY_FILE="/etc/krypt/keys/${VM}.key"
IMG_FILE="/var/lib/krypt/vms/${VM}.img"
CFG_FILE="/etc/xen/krypt/${VM}.cfg"

[[ -f "${KEY_FILE}" ]] || { echo "Key-Datei fehlt: ${KEY_FILE}" >&2; exit 1; }
[[ -f "${IMG_FILE}" ]] || { echo "Image fehlt: ${IMG_FILE}" >&2; exit 1; }
[[ -f "${CFG_FILE}" ]] || { echo "XL-Config fehlt: ${CFG_FILE}" >&2; exit 1; }

if ! cryptsetup status "${VM}-root" >/dev/null 2>&1; then
    echo "Öffne ${VM}.img…"
    cryptsetup open --key-file "${KEY_FILE}" "${IMG_FILE}" "${VM}-root"
fi

echo "Starte VM: ${VM}"
xl create "${CFG_FILE}"
VMOPEN
chmod +x "${PROFILE_DIR}/airootfs/usr/local/bin/krypt-vm-open"
ok "krypt-vm-open installiert"

# ── Autostart des Installers auf tty1 ────────────────────────────────────────
mkdir -p "${PROFILE_DIR}/airootfs/etc/systemd/system"
cat > "${PROFILE_DIR}/airootfs/etc/systemd/system/krypt-installer-tty1.service" <<'SVCEOF'
[Unit]
Description=Krypt OS TUI-Installer
After=multi-user.target
# Nicht starten wenn das System bereits installiert wurde
ConditionPathExists=!/etc/krypt/.installed

[Service]
Type=simple
ExecStart=/usr/bin/krypt-install
StandardInput=tty
StandardOutput=tty
StandardError=journal
TTYPath=/dev/tty1
TTYReset=yes
TTYVHangup=yes
Restart=on-failure
RestartSec=3s

[Install]
WantedBy=multi-user.target
SVCEOF

mkdir -p "${PROFILE_DIR}/airootfs/etc/systemd/system/multi-user.target.wants"
ln -sf "/etc/systemd/system/krypt-installer-tty1.service" \
    "${PROFILE_DIR}/airootfs/etc/systemd/system/multi-user.target.wants/krypt-installer-tty1.service"
ok "Installer-Autostart konfiguriert"

ok "Profil fertig: ${PROFILE_DIR}"
echo ""

# ── ISO bauen ─────────────────────────────────────────────────────────────────
log "Baue ISO (Work-Dir: ${WORK_DIR})…"
mkdir -p "${WORK_DIR}" "${OUT_DIR}"

mkarchiso -v \
    -w "${WORK_DIR}" \
    -o "${OUT_DIR}" \
    "${PROFILE_DIR}"

# ── Ergebnis ─────────────────────────────────────────────────────────────────
ISO_FILE=$(ls "${OUT_DIR}"/krypt-os-*.iso 2>/dev/null | sort -V | tail -1)
[[ -z "${ISO_FILE}" ]] && ISO_FILE=$(ls "${OUT_DIR}"/*.iso 2>/dev/null | sort -V | tail -1)
[[ -n "${ISO_FILE}" ]] || die "ISO nicht gefunden in ${OUT_DIR}"

SHA=$(sha256sum "${ISO_FILE}" | cut -d' ' -f1)

echo ""
echo -e "  ${GREEN}${BOLD}ISO fertig!${RESET}"
echo ""
printf "  %-10s %s\n" "Datei:"  "$(basename "${ISO_FILE}")"
printf "  %-10s %s\n" "Größe:"  "$(du -sh "${ISO_FILE}" | cut -f1)"
printf "  %-10s %s\n" "SHA256:" "${SHA}"
echo ""
echo "  SHA256-Datei schreiben:"
echo "  echo '${SHA}  $(basename "${ISO_FILE}")' > ${OUT_DIR}/krypt-os-${KRYPT_VERSION}.sha256"
echo ""
echo "  Auf USB schreiben:"
echo "  dd if=${ISO_FILE} of=/dev/sdX bs=4M status=progress oflag=sync"
echo ""

# SHA256-Datei anlegen
echo "${SHA}  $(basename "${ISO_FILE}")" > "${OUT_DIR}/krypt-os-${KRYPT_VERSION}.sha256"
ok "SHA256 gespeichert: ${OUT_DIR}/krypt-os-${KRYPT_VERSION}.sha256"
