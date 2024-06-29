//! HID transport for local USB devices
use std::collections::HashMap;

use anyhow::Result;

use futures::{SinkExt, TryStreamExt};
//pub use hidapi::HidError;
//use hidapi::{HidApi, HidDevice, HidResult};
use rusb::{Device as RusbDevice, DeviceList, Error, GlobalContext};
use stream::HidStream;
use url2::Url2;

mod discover;
mod stream;
//mod wrapper;
pub use discover::*;

use super::IntoTransport;

pub const VID_MINIDSP: u16 = 0x2752;
pub const OLD_MINIDSP_PID: (u16, u16) = (0x04d8, 0x003f);
/*
static HIDAPI_INSTANCE: AtomicRefCell<Option<Arc<Mutex<HidApi>>>> = AtomicRefCell::new(None);

/// Initializes a global instance of HidApi
pub fn initialize_api() -> HidResult<Arc<Mutex<HidApi>>> {
    if let Some(x) = HIDAPI_INSTANCE.borrow().deref() {
        return Ok(x.clone());
    }

    let api = Arc::new(Mutex::new(HidApi::new()?));
    HIDAPI_INSTANCE.borrow_mut().replace(api.clone());
    Ok(api)
}*/

pub struct HidTransport {
    stream: HidStream,
}

impl HidTransport {
    pub fn new(device: RusbDevice<GlobalContext>) -> HidTransport {
        HidTransport {
            stream: HidStream::new(device),
        }
    }

    pub fn with_url(url: &Url2) -> Result<HidTransport, Error> {
        // If we have a path, decode it.
        /*let path = url.path();
        if !path.is_empty() {
            let path = urlencoding::decode(path).map_err(|_| Error::InvalidParam)?; // HidError::HidApiErrorEmpty)?;
            return HidTransport::with_path(path.to_string());
        }*/

        // If it's empty, try to get the vid and pid from the query string
        let query: HashMap<_, _> = url.query_pairs().collect();
        let vid = query.get("vid");
        let pid = query.get("pid");

        if let (Some(vid), Some(pid)) = (vid, pid) {
            let vid = u16::from_str_radix(vid, 16).map_err(|_| Error::InvalidParam)?;
            let pid = u16::from_str_radix(pid, 16).map_err(|_| Error::InvalidParam)?;
            return HidTransport::with_product_id(vid, pid);
        }

        Err(Error::InvalidParam)
    }

    /*pub fn with_path(path: String) -> Result<HidTransport, Error> {
        let path = std::ffi::CString::new(path.into_bytes()).unwrap();

        Ok(HidTransport::new(hid.open_path(&path)?))


        let hid_device = DeviceList::new()?
            .iter()
            //.flat_map(|d| Ok(d.device_descriptor()?))
            .find(|d| {
                d.address()==path.into_bytes()
            }); //hid.open(vid, pid)?;
        match hid_device {
            Some(device) => Ok(HidTransport::new(device)),
            None => Err(Error::NotFound),
        }
    }*/

    pub fn with_product_id(vid: u16, pid: u16) -> Result<HidTransport, Error> {
        let hid_device = DeviceList::new()?
            .iter()
            //.flat_map(|d| Ok(d.device_descriptor()?))
            .find(|d| {
                let descriptor = d.device_descriptor().unwrap(); //yuck
                descriptor.vendor_id() == vid && descriptor.product_id() == pid
            }); //hid.open(vid, pid)?;
        match hid_device {
            Some(device) => Ok(HidTransport::new(device)),
            None => Err(Error::NotFound),
        }
    }

    pub fn into_inner(self) -> HidStream {
        self.stream
    }
}

impl IntoTransport for HidTransport {
    fn into_transport(self) -> super::Transport {
        Box::pin(self.into_inner().sink_err_into().err_into())
    }
}
