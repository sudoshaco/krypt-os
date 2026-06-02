#!/usr/bin/env bash
# Krypt OS — Live-ISO Auto-Start
# Wird aus ~/.zlogin nach dem agetty-Autologin (root@tty1) aufgerufen.
#
# Verhalten:
#   * tty1, kein script= in cmdline    → krypt-install starten
#   * tty1, mit script=URL in cmdline  → upstream archiso-Verhalten (Script laden)
#   * andere tty                       → no-op (normale Shell)
#
# Crasht der Installer, fällt das Script auf eine Hinweis-Shell zurück,
# anstatt den User vor einem leeren Prompt sitzen zu lassen.

set -u

script_cmdline() {
    local param
    for param in $(</proc/cmdline); do
        case "${param}" in
            script=*) echo "${param#*=}"; return 0 ;;
        esac
    done
}

automated_remote_script() {
    local script rt
    script="$(script_cmdline)"
    [[ -z "${script}" ]] && return 1
    [[ -x /tmp/startup_script ]] && return 0

    if [[ "${script}" =~ ^((http|https|ftp|tftp)://) ]]; then
        printf '%s: downloading %s\n' "$0" "${script}"
        systemd-run --pty --quiet -p Wants=network-online.target -p After=network-online.target \
            curl "${script}" --location --retry-connrefused --retry 10 --fail -s -o /tmp/startup_script
        rt=$?
    else
        cp "${script}" /tmp/startup_script
        rt=$?
    fi
    if [[ ${rt} -eq 0 ]]; then
        chmod +x /tmp/startup_script
        printf '%s: executing automated script\n' "$0"
        /tmp/startup_script
        return 0
    fi
    return 1
}

krypt_disabled_in_cmdline() {
    local param
    for param in $(</proc/cmdline); do
        [[ "${param}" == "krypt.installer=off" ]] && return 0
        [[ "${param}" == "nokrypt" ]] && return 0
    done
    return 1
}

launch_installer() {
    # System darf nicht bereits installiert sein.
    [[ -e /etc/krypt/.installed ]] && return 0

    # Manueller Opt-out via Kernel-Cmdline.
    krypt_disabled_in_cmdline && {
        echo ""
        echo "  krypt: Installer per Cmdline deaktiviert (krypt.installer=off)."
        echo "  Manuell starten:  krypt-install"
        echo ""
        return 0
    }

    # Auf Filesystem-Sichtbarkeit warten — kurz nach Boot kann
    # /usr/bin/krypt-install noch nicht fertig gelinkt sein.
    local retries=20
    while (( retries > 0 )); do
        [[ -x /usr/bin/krypt-install ]] && break
        sleep 0.25
        (( retries-- ))
    done

    if [[ ! -x /usr/bin/krypt-install ]]; then
        echo ""
        echo "  krypt: FEHLER — /usr/bin/krypt-install fehlt."
        echo "  ISO defekt? Bitte erneut bauen oder Issue melden."
        echo ""
        return 1
    fi

    # Installer im Loop starten: bei Crash kurze Pause, dann Notfall-Shell.
    while true; do
        clear
        /usr/bin/krypt-install
        local rc=$?
        # Sauberes Quit (q / Strg+C) → Drop in Shell, nicht endlos relaunchen.
        if (( rc == 0 )); then
            echo ""
            echo "  krypt: Installer beendet. Shell folgt — 'krypt-install' für Neustart."
            echo ""
            return 0
        fi
        echo ""
        echo "  krypt: Installer abgestürzt (exit ${rc})."
        echo "  Neustart in 5s — Strg+C für Shell."
        echo ""
        sleep 5 || return 1
    done
}

# ── Main ────────────────────────────────────────────────────────────────────
[[ "$(tty)" == "/dev/tty1" ]] || exit 0

# Erst archiso-script=, dann Krypt-Installer.
if automated_remote_script; then
    exit 0
fi

launch_installer
