// usb.rs — USB-Stick Monitor
//
// Überwacht USB-Events via tokio-udev (NETLINK_KOBJECT_UEVENT).
// Subsystem "usb", Devtype "usb_device" → nur physische Geräteereignisse.
// Auth-Stick abgezogen → UsbEvent::AuthStickRemoved auf den mpsc-Kanal.
#![allow(dead_code)]

use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::PathBuf;
use futures::StreamExt;
use thiserror::Error;
use tokio::sync::mpsc;
use tokio_udev::{AsyncMonitorSocket, EventType, MonitorBuilder};

#[derive(Debug, Error)]
pub enum UsbError {
    #[error("udev monitor error: {0}")]
    Udev(String),
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone)]
pub struct UsbDevice {
    pub vendor_id: u16,
    pub product_id: u16,
    /// Seriennummer — primäres Identifikationsmerkmal für Auth-Sticks
    pub serial: Option<String>,
    pub mount_point: Option<PathBuf>,
}

#[derive(Debug)]
pub enum UsbEvent {
    /// Registrierter Auth-Stick eingesteckt
    AuthStickConnected(UsbDevice),
    /// Auth-Stick abgezogen — Kill-Switch-Logik anstoßen
    AuthStickRemoved(UsbDevice),
    /// Unbekanntes Gerät
    Unknown(UsbDevice),
}

pub struct UsbMonitor {
    /// serial → LUKS-Key-Slot
    known_sticks: HashMap<String, u32>,
}

impl UsbMonitor {
    pub fn new() -> Self {
        Self {
            known_sticks: HashMap::new(),
        }
    }

    pub fn register_stick(&mut self, serial: String, slot: u32) {
        self.known_sticks.insert(serial, slot);
    }

    pub fn slot_for(&self, serial: &str) -> Option<u32> {
        self.known_sticks.get(serial).copied()
    }

    fn classify(&self, device: UsbDevice, added: bool) -> UsbEvent {
        let known = device
            .serial
            .as_deref()
            .map(|s| self.known_sticks.contains_key(s))
            .unwrap_or(false);

        match (known, added) {
            (true, true)  => UsbEvent::AuthStickConnected(device),
            (true, false) => UsbEvent::AuthStickRemoved(device),
            _             => UsbEvent::Unknown(device),
        }
    }

    /// Startet den udev-Event-Loop und sendet klassifizierte Events auf `tx`.
    /// Kehrt zurück wenn der Sender gedroppt wird oder ein fataler Fehler auftritt.
    pub async fn run(self, tx: mpsc::Sender<UsbEvent>) -> Result<(), UsbError> {
        let socket = MonitorBuilder::new()?
            .match_subsystem_devtype("usb", "usb_device")?
            .listen()?;

        let monitor = AsyncMonitorSocket::new(socket)?;
        tokio::pin!(monitor);

        while let Some(result) = monitor.next().await {
            let event = match result {
                Ok(e) => e,
                Err(e) => {
                    tracing::warn!("udev event read error: {e}");
                    continue;
                }
            };

            let is_add    = matches!(event.event_type(), EventType::Add);
            let is_remove = matches!(event.event_type(), EventType::Remove);
            if !is_add && !is_remove {
                continue;
            }

            // Device wraps *mut udev_device (!Send) — im Block droppen, bevor .await kommt
            let usb_dev = {
                let dev = event.device();
                UsbDevice {
                    vendor_id:   parse_hex_attr(dev.attribute_value("idVendor")),
                    product_id:  parse_hex_attr(dev.attribute_value("idProduct")),
                    serial:      dev.attribute_value("serial")
                                   .and_then(OsStr::to_str)
                                   .map(str::to_owned),
                    mount_point: None,
                }
            }; // dev hier gedroppt — !Send nicht mehr im Scope

            let usb_event = self.classify(usb_dev, is_add);

            if tx.send(usb_event).await.is_err() {
                break; // Receiver dropped — daemon fährt herunter
            }
        }

        Ok(())
    }
}

impl Default for UsbMonitor {
    fn default() -> Self {
        Self::new()
    }
}

fn parse_hex_attr(attr: Option<&OsStr>) -> u16 {
    attr.and_then(OsStr::to_str)
        .and_then(|s| u16::from_str_radix(s.trim(), 16).ok())
        .unwrap_or(0)
}
