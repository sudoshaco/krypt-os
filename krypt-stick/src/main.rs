// krypt-stick — USB Authentication Stick Manager
//
// Verwaltet LUKS2-Key-Slots für Krypt OS Auth-Sticks.
// Alle Operationen benötigen Root-Rechte (LUKS2-Header-Zugriff).
//
// Usage:
//   krypt-stick --luks-dev /dev/sda2 setup --stick-dev /dev/sdb
//   krypt-stick --luks-dev /dev/sda2 add-backup --stick-dev /dev/sdc
//   krypt-stick --luks-dev /dev/sda2 revoke 1
//   krypt-stick --luks-dev /dev/sda2 list
//   krypt-stick --luks-dev /dev/sda2 promote 0

mod backup;
mod create;
mod luks;
mod revoke;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "krypt-stick")]
#[command(about = "Krypt OS — USB Authentication Stick Manager")]
struct Cli {
    /// LUKS2-Partition des Root-Devices (z.B. /dev/sda2)
    #[arg(long, env = "KRYPT_LUKS_DEV", default_value = "/dev/sda2")]
    luks_dev: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Primär-Stick beim Setup erstellen (schreibt Key auf Stick + luksAddKey)
    Setup {
        /// USB-Stick Block-Device (z.B. /dev/sdb)
        #[arg(long)]
        stick_dev: String,
        /// Bestätigungsprompt überspringen (für Installer/Scripting)
        #[arg(long)]
        force: bool,
    },
    /// Backup-Stick hinzufügen (neuer LUKS-Slot)
    AddBackup {
        /// USB-Stick Block-Device (z.B. /dev/sdb)
        #[arg(long)]
        stick_dev: String,
    },
    /// Stick-Slot widerrufen (luksKillSlot — irreversibel)
    Revoke {
        /// LUKS2-Slot-Nummer (0–31)
        slot: u32,
    },
    /// Alle aktiven Key-Slots des LUKS2-Devices anzeigen
    List,
    /// Backup-Slot als primären Slot markieren
    Promote {
        /// Slot-Nummer des Backup-Sticks
        slot: u32,
    },
}

fn main() {
    let cli = Cli::parse();

    if !nix::unistd::Uid::effective().is_root() {
        eprintln!("krypt-stick: root required (LUKS2 operations need CAP_SYS_ADMIN)");
        std::process::exit(1);
    }

    if let Err(e) = run(cli) {
        eprintln!("krypt-stick: error: {e}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> luks::Result<()> {
    match cli.command {
        Commands::Setup { stick_dev, force } => create::run_setup(&cli.luks_dev, &stick_dev, force),
        Commands::AddBackup { stick_dev } => backup::add(&cli.luks_dev, &stick_dev),
        Commands::Revoke { slot }         => revoke::slot(&cli.luks_dev, slot),
        Commands::List                    => luks::list_slots(&cli.luks_dev),
        Commands::Promote { slot }        => backup::promote(&cli.luks_dev, slot),
    }
}
