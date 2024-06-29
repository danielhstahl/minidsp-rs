use core::panic;
use std::{
    pin::Pin,
    task::{Context, Poll},
    thread, time,
};

use bytes::{BufMut, Bytes, BytesMut};
use futures::{
    channel::{mpsc, mpsc::TrySendError},
    Future, FutureExt, Sink, Stream,
};
//use hidapi::{HidDevice, HidError};
//use super::wrapper::HidDeviceWrapper;
use pin_project::pin_project;
use rusb::{Device, Error, GlobalContext};
use std::time::Duration;

// 65 byte wide: 1 byte report id + 64 bytes data
const HID_PACKET_SIZE: usize = 65;

type SendFuture = Box<dyn Future<Output = Result<(), Error>> + Send>;

///*Device<T>, //*/
/// A stream of HID reports
#[pin_project]
pub struct HidStream {
    device: Device<GlobalContext>,

    #[pin]
    rx: mpsc::UnboundedReceiver<Result<Bytes, Error>>,

    current_tx: Option<Pin<SendFuture>>,
}

impl HidStream {
    pub fn new(device: Device<GlobalContext>) -> Self {
        //let device = Arc::new(HidDeviceWrapper::new(device));
        let (tx, rx) = mpsc::unbounded();
        Self::start_recv_loop(device.clone(), tx);

        Self {
            rx,
            current_tx: None,
            device,
        }
    }

    fn send(&self, frame: Bytes) -> impl Future<Output = Result<(), Error>> {
        let mut buf = BytesMut::with_capacity(HID_PACKET_SIZE);

        // HID report id
        buf.put_u8(0);

        // Frame data
        buf.extend_from_slice(&frame);

        // Pad remaining packet data with 0xFF
        buf.resize(HID_PACKET_SIZE, 0xFF);

        let buf = buf.freeze();

        let device = self.device.clone();
        async move {
            let device_handle = device.open().unwrap();
            tokio::task::block_in_place(|| {
                let mut remaining_tries = 10;
                let mut result = device_handle.write_bulk(
                    device.address(),
                    &buf,
                    time::Duration::from_millis(500),
                );
                while let Err(e) = result {
                    log::warn!("retrying usb write: {:?}", e);
                    thread::sleep(time::Duration::from_millis(250));
                    result = device_handle.write_bulk(
                        device.address(),
                        &buf,
                        time::Duration::from_millis(500),
                    );
                    remaining_tries -= 1;
                    if remaining_tries == 0 {
                        return result;
                    }
                }
                result
            })?;
            Ok(())
        }
    }

    fn start_recv_loop(
        device: Device<GlobalContext>,
        tx: mpsc::UnboundedSender<Result<Bytes, Error>>,
    ) {
        thread::spawn(move || {
            let device_handle = device.open().unwrap();
            loop {
                if tx.is_closed() {
                    return Ok::<(), TrySendError<_>>(());
                }

                let mut read_buf = BytesMut::with_capacity(HID_PACKET_SIZE);
                read_buf.resize(HID_PACKET_SIZE, 0);

                // Use a short timeout because we want to be able to bail out if the receiver gets
                // dropped.
                let size =
                    device_handle.read_interrupt(1, &mut read_buf, Duration::from_millis(500));
                match size {
                    // read_timeout returns Ok(0) if a timeout has occurred
                    Ok(0) => continue,
                    Ok(size) => {
                        // successful read
                        read_buf.truncate(size);
                        log::trace!("read: {:02x?}", read_buf.as_ref());
                        tx.unbounded_send(Ok(read_buf.freeze()))?;
                    }
                    Err(e) => {
                        // device error
                        log::error!("error in hid receive loop: {:?}", e);
                        tx.unbounded_send(Err(e))?;
                    }
                }
            }
        });
    }
}

impl Stream for HidStream {
    type Item = Result<Bytes, Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.project().rx.poll_next(cx)
    }
}

impl Sink<Bytes> for HidStream {
    type Error = Error;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // If we have a pending send future, poll it here
        if let Some(ref mut future) = self.current_tx {
            let result = future.as_mut().poll(cx);
            if result.is_ready() {
                self.current_tx.take();
            }
            return result;
        }

        // If not, we're ready to send
        Poll::Ready(Ok(()))
    }

    fn start_send(mut self: Pin<&mut Self>, item: Bytes) -> Result<(), Self::Error> {
        if self.current_tx.is_some() {
            panic!("start_send called without being ready")
        }

        // Start sending future
        self.current_tx = Some(Box::pin(self.send(item).fuse()));

        Ok(())
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.poll_ready(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // Poll for readiness to ensure that no sends are pending
        self.poll_ready(cx)
    }
}

#[cfg(test)]
mod test {
    use futures::{SinkExt, StreamExt};

    use super::*;
    //use crate::transport::hid::initialize_api;
    use rusb::DeviceList;

    #[tokio::test]
    #[ignore]
    async fn test_hid() {
        let mut device = {
            //let api = initialize_api().unwrap();
            //let api = api.lock().unwrap();
            let device = DeviceList::new()
                .unwrap()
                .iter()
                //.flat_map(|d| Ok(d.device_descriptor()?))
                .find(|d| {
                    let descriptor = d.device_descriptor().unwrap(); //yuck
                    descriptor.vendor_id() == 0x2752 && descriptor.product_id() == 0x0011
                })
                .unwrap(); //hid.open(vid, pid)?;
                           //let device = api.open(0x2752, 0x0011).unwrap();
            Box::pin(HidStream::new(device))
        };

        device
            .send(Bytes::from_static(&[0x02, 0x31, 0x33]))
            .await
            .unwrap();
        let resp = device.next().await.unwrap();
        println!("{:02x?}", resp.as_ref());
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore]
    async fn test_multi() {
        let device = {
            //let api = initialize_api().unwrap();
            //let api = api.lock().unwrap();
            let device = DeviceList::new()
                .unwrap()
                .iter()
                //.flat_map(|d| Ok(d.device_descriptor()?))
                .find(|d| {
                    let descriptor = d.device_descriptor().unwrap(); //yuck
                    descriptor.vendor_id() == 0x2752 && descriptor.product_id() == 0x0011
                })
                .unwrap();
            //let device = api.open(0x2752, 0x0011).unwrap();
            Box::new(HidStream::new(device))
        };

        let (mut sink, mut stream) = device.split();
        tokio::spawn(async move {
            loop {
                sink.send(Bytes::from_static(&[0x02, 0x31, 0x33]))
                    .await
                    .unwrap();
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
        });

        tokio::spawn(async move {
            loop {
                let x = stream.next().await;
                println!("{x:?}");
            }
        });
    }
}
