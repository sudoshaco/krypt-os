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
    PANIC_BIN="${REPO_ROOT}/target/release/krypt-panic"

    [[ -f "${DAEMON_BIN}" ]] || die "krypt-daemon binary nicht gefunden nach cargo build"
    [[ -f "${STICK_BIN}" ]]  || die "krypt-stick binary nicht gefunden nach cargo build"
    [[ -f "${GUI_BIN}" ]]    || die "krypt-gui binary nicht gefunden nach cargo build"
    [[ -f "${PANIC_BIN}" ]]  || die "krypt-panic binary nicht gefunden nach cargo build"
    ok "Rust release builds fertig"
else
    warn "--skip-rust: überspringe cargo build"
    DAEMON_BIN="${REPO_ROOT}/target/release/krypt-daemon"
    GUI_BIN="${REPO_ROOT}/target/release/krypt-gui"
    STICK_BIN="${REPO_ROOT}/target/release/krypt-stick"
    PANIC_BIN="${REPO_ROOT}/target/release/krypt-panic"
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
  ["/usr/bin/krypt-panic"]="0:0:755"
  ["/usr/lib/krypt/krypt-boot-agent.sh"]="0:0:755"
  ["/usr/share/krypt-installer/main.py"]="0:0:755"
  ["/usr/bin/krypt-install"]="0:0:755"
  ["/root/.automated_script.sh"]="0:0:700"
  ["/root/.zlogin"]="0:0:600"
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
# Die Units werden in den airootfs kopiert, aber NICHT für die Live-ISO
# aktiviert: krypt-daemon braucht Xen (xl) und würde im Live-System
# in einen Restart-Loop laufen. Der Installer enabled die Units auf
# dem Ziel-System (siehe installer/steps/install.py).
install -Dm644 "${REPO_ROOT}/init/krypt-daemon.service" \
    "${PROFILE_DIR}/airootfs/etc/systemd/system/krypt-daemon.service"
install -Dm644 "${REPO_ROOT}/init/krypt-boot-agent.service" \
    "${PROFILE_DIR}/airootfs/etc/systemd/system/krypt-boot-agent.service"

mkdir -p "${PROFILE_DIR}/airootfs/etc/systemd/system/multi-user.target.wants"

# ── Rust-Binaries ─────────────────────────────────────────────────────────────
log "Rust-Binaries einbinden…"
mkdir -p "${PROFILE_DIR}/airootfs/usr/bin"
# Im --skip-rust Pfad sind die Binaries möglicherweise von einem alten Run
# übrig — wir prüfen jetzt mit -f und failen hart wenn was fehlt, statt
# stillschweigend eine ISO ohne USB-Kill-Switch oder Wayland-GUI zu bauen.
for bin_var in DAEMON_BIN GUI_BIN STICK_BIN PANIC_BIN; do
    src="${!bin_var}"
    name="$(basename "${src}")"
    [[ -f "${src}" ]] || die "${name} fehlt — Rust-Workspace neu bauen (build.sh ohne --skip-rust)"
    install -Dm755 "${src}" "${PROFILE_DIR}/airootfs/usr/bin/${name}"
done
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
set -u

INSTALLER_DIR="/usr/share/krypt-installer"

if [[ ! -f "${INSTALLER_DIR}/main.py" ]]; then
    echo "krypt-install: FEHLER — ${INSTALLER_DIR}/main.py nicht gefunden." >&2
    echo "                ISO defekt oder Installer nicht eingebunden." >&2
    exit 2
fi

# Python-Importpfad fixieren, damit 'from steps.welcome import ...' greift,
# egal aus welchem cwd der Wrapper gestartet wird.
cd "${INSTALLER_DIR}" || { echo "krypt-install: cd ${INSTALLER_DIR} fehlgeschlagen" >&2; exit 2; }
export PYTHONPATH="${INSTALLER_DIR}:${PYTHONPATH:-}"

# Schnelle Sanity-Checks für die drei Pflicht-Module.
if ! python3 -c "import textual, rich, psutil" 2>/dev/null; then
    echo "krypt-install: FEHLER — Python-Module fehlen (textual/rich/psutil)." >&2
    echo "                Erwartet via packages.x86_64 → python-textual etc." >&2
    exit 3
fi

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

    # ── PF2-Fonts generieren (known-issue #11) ──────────────────────────────
    # GRUB versteht ausschließlich PF2-Fonts. theme.txt referenziert
    # "JetBrainsMono Nerd Font Regular 14" etc. — ohne entsprechende PF2
    # fällt GRUB auf den Default-Font (unicode.pf2) zurück und das Theme
    # sieht generisch aus. grub-mkfont konvertiert TTF → PF2 mit dem Namen
    # aus der TTF-Metadata, sodass die Referenz im Theme greift.
    #
    # TTF-Pfad variiert zwischen ttf-jetbrains-mono-nerd Versionen — wir
    # probieren mehrere bekannte Pfade. Wenn keiner existiert ODER
    # grub-mkfont nicht installiert ist, wird nur gewarnt (kein die).
    if command -v grub-mkfont >/dev/null 2>&1; then
        # Regular + Bold TTFs separat auflösen — theme.txt referenziert
        # "JetBrainsMono Nerd Font Regular 14" UND "...Bold 14"/"Bold 28".
        # Wenn wir nur Regular generieren, fällt GRUB für die Bold-Strings
        # stillschweigend auf unicode.pf2 zurück und das Boot-Menü mischt
        # Stile, mit denen das Layout (item_height = 38) nicht passt.
        REGULAR_TTF=""
        BOLD_TTF=""
        for candidate in \
            "/usr/share/fonts/TTF/JetBrainsMonoNerdFont-Regular.ttf" \
            "/usr/share/fonts/jetbrains-mono-nerd/JetBrainsMonoNerdFont-Regular.ttf" \
            "/usr/share/fonts/jetbrainsmono-nerd/JetBrainsMonoNerdFont-Regular.ttf" \
            "/usr/share/fonts/JetBrains-Mono/JetBrainsMonoNerdFont-Regular.ttf" \
        ; do
            [[ -f "$candidate" ]] && { REGULAR_TTF="$candidate"; break; }
        done
        for candidate in \
            "/usr/share/fonts/TTF/JetBrainsMonoNerdFont-Bold.ttf" \
            "/usr/share/fonts/jetbrains-mono-nerd/JetBrainsMonoNerdFont-Bold.ttf" \
            "/usr/share/fonts/jetbrainsmono-nerd/JetBrainsMonoNerdFont-Bold.ttf" \
            "/usr/share/fonts/JetBrains-Mono/JetBrainsMonoNerdFont-Bold.ttf" \
        ; do
            [[ -f "$candidate" ]] && { BOLD_TTF="$candidate"; break; }
        done
        if [[ -n "$REGULAR_TTF" ]]; then
            # theme.txt nutzt Regular in 10/11/13/14pt
            for size in 10 11 13 14; do
                grub-mkfont --size="${size}" \
                    --output="${GRUB_THEME_DST}/jbm-regular-${size}.pf2" \
                    "$REGULAR_TTF" 2>/dev/null || warn "grub-mkfont regular ${size}pt fehlgeschlagen"
            done
            ok "Regular-PF2 generiert (10/11/13/14pt aus ${REGULAR_TTF})"
        else
            warn "JetBrainsMono Nerd Font Regular nicht gefunden — GRUB-Theme nutzt Default-Font"
            warn "  Installier: sudo pacman -S ttf-jetbrains-mono-nerd"
        fi
        if [[ -n "$BOLD_TTF" ]]; then
            # theme.txt nutzt Bold in 14/28pt (selected_item_font + title)
            for size in 14 28; do
                grub-mkfont --size="${size}" \
                    --output="${GRUB_THEME_DST}/jbm-bold-${size}.pf2" \
                    "$BOLD_TTF" 2>/dev/null || warn "grub-mkfont bold ${size}pt fehlgeschlagen"
            done
            ok "Bold-PF2 generiert (14/28pt aus ${BOLD_TTF})"
        else
            warn "JetBrainsMono Nerd Font Bold nicht gefunden — Theme-Titel fällt auf Regular zurück"
        fi
    else
        warn "grub-mkfont nicht installiert — überspringe PF2-Font-Generation"
        warn "  Installier: sudo pacman -S grub"
    fi

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
# Hyprland (hyprland.conf, animations.conf)
[[ -d "${REPO_ROOT}/dotfiles/hyprland" ]] && \
    cp -rT "${REPO_ROOT}/dotfiles/hyprland" "${SKEL}/.config/hypr"
# Hyprlock-Conf landet im selben ~/.config/hypr/ Verzeichnis
if [[ -f "${REPO_ROOT}/dotfiles/hyprlock/hyprlock.conf" ]]; then
    install -Dm644 "${REPO_ROOT}/dotfiles/hyprlock/hyprlock.conf" \
        "${SKEL}/.config/hypr/hyprlock.conf"
fi
# Hypridle-Conf — sonst startet hypridle via exec-once ohne Config
# und das ganze Idle-Timing-Verhalten (Screensaver/Lock/DPMS) wirkt nicht.
if [[ -f "${REPO_ROOT}/dotfiles/hypridle/hypridle.conf" ]]; then
    install -Dm644 "${REPO_ROOT}/dotfiles/hypridle/hypridle.conf" \
        "${SKEL}/.config/hypr/hypridle.conf"
fi
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
# Branding (Screensaver + ASCII-Logo)
# - Logo nach /etc/skel/.config/krypt/branding/ damit neue User es haben
# - Skripte nach /usr/local/bin damit Hyprland sie ohne absoluten Pfad
#   per `exec` aufrufen kann (siehe Keybind in hyprland.conf).
if [[ -d "${REPO_ROOT}/dotfiles/branding" ]]; then
    mkdir -p "${SKEL}/.config/krypt/branding"
    install -Dm644 "${REPO_ROOT}/dotfiles/branding/screensaver.txt" \
        "${SKEL}/.config/krypt/branding/screensaver.txt"
    install -Dm755 "${REPO_ROOT}/dotfiles/branding/krypt-screensaver" \
        "${PROFILE_DIR}/airootfs/usr/local/bin/krypt-screensaver"
    install -Dm755 "${REPO_ROOT}/dotfiles/branding/krypt-launch-screensaver" \
        "${PROFILE_DIR}/airootfs/usr/local/bin/krypt-launch-screensaver"
fi
ok "Dotfiles eingebunden (nvim, hyprland, waybar, rofi, foot, branding)"

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

# ── Autostart des Installers via root-Autologin (.zlogin → .automated_script.sh)
#
# Archiso aktiviert auf tty1 ein agetty mit --autologin root. Die Login-Shell
# (zsh) ruft ~/.zlogin auf, das wiederum ~/.automated_script.sh ausführt.
# Unser Overlay (build/airootfs/root/.automated_script.sh) startet von dort aus
# /usr/bin/krypt-install.
#
# Vorteil gegenüber einer eigenen systemd-Unit: kein TTY-Konflikt mit getty,
# Plymouth-Boot-Splash bleibt aktiv, und beim Beenden landet der User in der
# autologged Shell statt vor einem leeren Prompt.
log "Installer-Autostart (root-autologin → .zlogin → krypt-install)…"
KRYPT_AUTORUN="${REPO_ROOT}/build/airootfs/root/.automated_script.sh"
if [[ -f "${KRYPT_AUTORUN}" ]]; then
    install -Dm755 "${KRYPT_AUTORUN}" \
        "${PROFILE_DIR}/airootfs/root/.automated_script.sh"
    ok "Installer-Autostart konfiguriert (.automated_script.sh)"
else
    warn ".automated_script.sh nicht im Overlay gefunden — Installer startet nicht automatisch"
fi

# ── GRUB-Menü Krypt-branden ──────────────────────────────────────────────────
# Standard-releng grub.cfg sagt "Arch Linux install medium". Wir ersetzen
# Bezeichnung und Default-Eintrag durch Krypt-OS-Versionen — keine
# Funktionsänderung, nur Branding.
if [[ -f "${PROFILE_DIR}/grub/grub.cfg" ]]; then
    sed -i \
        -e 's|Arch Linux install medium|Krypt OS Installer|g' \
        -e 's|Arch Linux install medium with speakup screen reader|Krypt OS Installer (screen reader)|g' \
        "${PROFILE_DIR}/grub/grub.cfg" \
        "${PROFILE_DIR}/grub/loopback.cfg" 2>/dev/null || true
    ok "GRUB-Menüeinträge umbenannt"
fi
if [[ -f "${PROFILE_DIR}/syslinux/archiso_sys-linux.cfg" ]]; then
    sed -i \
        -e 's|Arch Linux install medium|Krypt OS Installer|g' \
        "${PROFILE_DIR}/syslinux/archiso_sys-linux.cfg" \
        "${PROFILE_DIR}/syslinux/archiso_pxe-linux.cfg" 2>/dev/null || true
    ok "Syslinux-Menüeinträge umbenannt"
fi

# ── Plymouth: quiet splash an die Kernel-Cmdline anhängen ────────────────────
# Ohne diese Flags werden Plymouth-Frames vom Kernel-Log überdeckt.
if [[ -f "${PROFILE_DIR}/grub/grub.cfg" ]] && \
   ! grep -q 'quiet splash' "${PROFILE_DIR}/grub/grub.cfg"; then
    sed -i 's|archisosearchuuid=%ARCHISO_UUID%|archisosearchuuid=%ARCHISO_UUID% quiet splash|g' \
        "${PROFILE_DIR}/grub/grub.cfg"
    ok "GRUB: quiet splash an Kernel-Cmdline angehängt"
fi
if [[ -f "${PROFILE_DIR}/syslinux/archiso_sys-linux.cfg" ]] && \
   ! grep -q 'quiet splash' "${PROFILE_DIR}/syslinux/archiso_sys-linux.cfg"; then
    sed -i 's|archisosearchuuid=%ARCHISO_UUID%|archisosearchuuid=%ARCHISO_UUID% quiet splash|g' \
        "${PROFILE_DIR}/syslinux/archiso_sys-linux.cfg"
    ok "Syslinux: quiet splash an Kernel-Cmdline angehängt"
fi
# UEFI systemd-boot entries (loader/entries/*.conf) wenn archiso sie hat
if compgen -G "${PROFILE_DIR}/efiboot/loader/entries/*.conf" >/dev/null 2>&1; then
    for entry in "${PROFILE_DIR}"/efiboot/loader/entries/*.conf; do
        if grep -q '^options' "${entry}" && ! grep -q 'quiet splash' "${entry}"; then
            sed -i 's|^options \(.*\)$|options \1 quiet splash|' "${entry}"
        fi
    done
    ok "systemd-boot entries: quiet splash ergänzt"
fi

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
