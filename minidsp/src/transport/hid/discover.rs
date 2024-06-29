//! Discovery of local devices
use std::{fmt, fmt::Formatter, str::FromStr};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
//use hidapi::{HidApi, HidError};
use super::{HidTransport, OLD_MINIDSP_PID, VID_MINIDSP};
use crate::transport::{IntoTransport, MiniDSPError, Openable, Transport};
use rusb::{DeviceDescriptor, DeviceList, Error};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    pub id: Option<(u16, u16)>,
    pub path: Option<String>,
}

impl Device {
    pub fn to_url(&self) -> String {
        self.to_string()
    }
}

impl FromStr for Device {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(path) = s.strip_prefix("path=") {
            Ok(Device {
                id: None,
                path: Some(path.to_owned()),
            })
        } else {
            let parts: Vec<_> = s.split(':').collect();
            if parts.len() != 2 {
                return Err("expected: vid:pid or path=...");
            }

            let vendor_id =
                u16::from_str_radix(parts[0], 16).map_err(|_| "couldn't parse vendor id")?;
            let product_id =
                u16::from_str_radix(parts[1], 16).map_err(|_| "couldn't parse product id")?;
            Ok(Device {
                id: Some((vendor_id, product_id)),
                path: None,
            })
        }
    }
}
impl fmt::Display for Device {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let query = match self.id {
            Some((vid, pid)) => format!("?vid={vid:04x}&pid={pid:04x}"),
            None => "".into(),
        };

        let path = match &self.path {
            Some(path) => urlencoding::encode(path.as_str()),
            None => "".into(),
        };

        write!(f, "usb:{path}{query}")
    }
}

#[async_trait]
impl Openable for Device {
    async fn open(&self) -> Result<Transport, MiniDSPError> {
        //let hid = initialize_api()?;
        //let hid = hid.lock().unwrap();

        if let Some(_path) = &self.path {
            //Ok(HidTransport::with_path(path.to_string())?.into_transport())
            Err(MiniDSPError::InternalError(anyhow!("path not implemented")))
        } else if let Some((vid, pid)) = &self.id {
            Ok(HidTransport::with_product_id(*vid, *pid)?.into_transport())
        } else {
            Err(MiniDSPError::InternalError(anyhow!(
                "invalid device, no path or id"
            )))
        }
    }

    fn to_url(&self) -> String {
        ToString::to_string(self)
    }
}

pub fn discover() -> Result<Vec<Device>, Error> {
    //hid.refresh_devices()?;

    Ok(DeviceList::new()?
        .iter()
        .map(|d| -> Result<(DeviceDescriptor, u8), Error> {
            let descriptor = d.device_descriptor()?;
            let address = d.address();
            Ok((descriptor, address))
        })
        .flatten()
        .filter(|(descriptor, _address)| {
            //let (descriptor, address) = device?;
            let vendor_id = descriptor.vendor_id();
            let product_id = descriptor.product_id();
            vendor_id == VID_MINIDSP || (vendor_id, product_id) == OLD_MINIDSP_PID
        })
        .map(|(descriptor, address)| Device {
            id: Some((descriptor.vendor_id(), descriptor.product_id())),
            path: Some(address.to_string()),
        })
        .collect())
}

pub fn discover_with<F: Fn(&DeviceDescriptor) -> bool>(func: F) -> Result<Vec<Device>, Error> {
    Ok(DeviceList::new()?
        .iter()
        .map(|d| -> Result<(DeviceDescriptor, u8), Error> {
            let descriptor = d.device_descriptor()?;
            let address = d.address();
            Ok((descriptor, address))
        })
        .flatten()
        .filter(|(descriptor, _address)| {
            //let (descriptor, address) = device?;
            func(descriptor)
        })
        .map(|(descriptor, address)| Device {
            id: Some((descriptor.vendor_id(), descriptor.product_id())),
            path: Some(address.to_string()),
        })
        .collect())
}
