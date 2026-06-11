# Krypt OS — Post-Install Setup

Dieser Guide ergänzt die TUI-Installation um manuelle Schritte, die der
Installer (Stand v0.1.0-alpha) noch nicht vollständig automatisiert.

Wenn der Installer am Ende eine Warnung über fehlendes Xen geschrieben hat
(`⚠ Xen wurde nicht installiert`), gilt Abschnitt 1.

---

## 1. Xen Hypervisor installieren (wenn vom Installer übersprungen)

`xen` liegt nicht in den offiziellen Arch-Repos. Der TUI-Installer versucht
zuerst das [Krypt-Pacman-Repo](https://github.com/sudoshaco/krypt-pkgs)
zu nutzen. Schlägt das fehl, läuft die Installation OHNE Xen durch und
das System bootet als reguläres Arch Linux.

### Option A: Krypt-Repo manuell hinzufügen (empfohlen)

```bash
sudo tee -a /etc/pacman.conf <<'EOF'

[krypt]
SigLevel = Optional TrustAll
Server = https://github.com/sudoshaco/krypt-pkgs/releases/download/latest
EOF

sudo pacman -Sy xen
```

### Option B: AUR-Build (Fallback)

```bash
sudo pacman -S --needed base-devel git
git clone https://aur.archlinux.org/yay.git
cd yay && makepkg -si

yay -S xen
```

### Nach Xen-Install GRUB neu generieren

```bash
sudo grub-mkconfig -o /boot/grub/grub.cfg
```

Beim nächsten Reboot erscheint im GRUB-Menü ein "Xen 4.x" Eintrag.

---

## 2. Auth-Stick einrichten (falls im Installer übersprungen)

LUKS-Mapper muss offen sein:

```bash
sudo cryptsetup open /dev/sda2 krypt-root   # interaktive Passphrase
sudo krypt-stick --luks-dev /dev/mapper/krypt-root setup --stick-dev /dev/sdX
```

Die ausgegebene Serial + LUKS-Slot in `/etc/krypt/daemon.toml` ergänzen:

```toml
[[auth_sticks]]
serial    = "<vom krypt-stick gemeldete Seriennummer>"
luks_slot = <ausgegebener Slot, üblich: 1>
```

Daemon neu laden:

```bash
sudo systemctl restart krypt-daemon
```

---

## 3. Backup-Stick einrichten

Stell sicher dass der Primärstick angesteckt ist (mindestens ein Slot
muss aktiv sein bevor cryptsetup luksAddKey läuft).

```bash
sudo krypt-stick --luks-dev /dev/mapper/krypt-root add-backup --stick-dev /dev/sdY
```

Backup-Stick ebenfalls in `daemon.toml` eintragen.

---

## 4. AppVM-Templates bootstrappen

Der Installer legt nur leere LUKS2-Disk-Images in `/var/lib/krypt/vms/<name>.img`
an (per [vms.py](../installer/steps/vms.py)). Ein bootbares Gast-System ist
darin noch nicht. Schnellweg für Alpine:

```bash
sudo cryptsetup open --key-file /etc/krypt/keys/work.key \
    /var/lib/krypt/vms/work.img work-root

sudo mount /dev/mapper/work-root /mnt

# Alpine-Mini-Root als Basis
wget https://dl-cdn.alpinelinux.org/alpine/latest-stable/releases/x86_64/alpine-minirootfs-3.20.0-x86_64.tar.gz
sudo tar -xzf alpine-minirootfs-*.tar.gz -C /mnt

sudo umount /mnt
sudo cryptsetup close work-root
```

Pro AppVM wiederholen.

---

## 5. dom0 Netzwerk-Isolation prüfen

Krypt setzt voraus dass dom0 KEINEN direkten Netzwerkzugang hat —
sämtlicher Traffic geht durch die `sys-net` AppVM. Der Installer
deaktiviert NetworkManager und konfiguriert systemd-networkd entsprechend.

Prüfen nach Reboot:

```bash
ip -br addr show           # Keine IPv4-Adresse auf eth0/wlan0
systemctl status NetworkManager  # inactive (disabled)
systemctl status systemd-networkd  # active (running)
```

Wenn doch eine IP da ist, in dom0:

```bash
sudo systemctl disable --now NetworkManager
sudo systemctl restart systemd-networkd
```

---

## 6. USB Kill-Switch funktional verifizieren

Ohne Reboot testen:

```bash
sudo systemctl status krypt-daemon  # active
journalctl -u krypt-daemon -f       # Tailen
```

In einem zweiten Terminal Stick abziehen — der Daemon sollte `PANIC level=…`
ins Journal schreiben und je nach `panic_level` in daemon.toml die VMs
einfrieren + Suspend/Lock/Poweroff auslösen.

---

## Weitere Referenzen

- `docs/install.md` — komplette Installations-Anleitung
- `docs/usb-kill-switch.md` — Stick-Spezifikation
- `docs/known-issues.md` — bekannte Lücken vor v0.1.0-alpha
