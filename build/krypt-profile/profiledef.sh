#!/usr/bin/env bash
# Krypt OS ISO Profile
iso_name="krypt-os"
iso_label="KRYPT_OS_202605"
iso_publisher="Krypt OS Project"
iso_application="Krypt OS — Cryptography-first Linux"
iso_version="b7e9aee"
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
