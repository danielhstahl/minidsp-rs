use crate::{
    commands::{self, Commands, FloatView, MemoryView, Value},
    transport::{MiniDSPError, SharedService},
    utils::ErrInto,
    DeviceInfo,
};

pub struct Client {
    transport: SharedService,
}

impl Client {
    pub fn new(transport: SharedService) -> Self {
        Self { transport }
    }

    pub async fn roundtrip(
        &self,
        cmd: commands::Commands,
    ) -> Result<commands::Responses, MiniDSPError> {
        self.transport.lock().await.call(cmd).await
    }

    /// Gets the hardware id and dsp firmware version
    pub async fn get_device_info(&self) -> Result<DeviceInfo, MiniDSPError> {
        let hw_id = self
            .roundtrip(Commands::ReadHardwareId)
            .await?
            .into_hardware_id()?;

        let dsp_version_view = self.read_memory(0xffa1, 1).await?;
        let serial_view = self.read_memory(0xfffc, 2).await?;
        let info = DeviceInfo {
            hw_id,
            dsp_version: dsp_version_view.read_u8(0xffa1),
            serial: 900000 + (serial_view.read_u16(0xfffc) as u32),
        };
        Ok(info)
    }

    /// Reads eeprom memory from the device
    pub async fn read_memory(&self, addr: u16, size: u8) -> Result<MemoryView, MiniDSPError> {
        self.roundtrip(Commands::ReadMemory { addr, size })
            .await?
            .into_memory_view()
            .err_into()
    }

    /// Reads a series of contiguous floats
    pub async fn read_floats(&self, addr: u16, len: u8) -> Result<FloatView, MiniDSPError> {
        self.roundtrip(Commands::ReadFloats { addr, len })
            .await?
            .into_float_view()
            .err_into()
    }

    /// Writes data to the dsp memory area
    pub async fn write_dsp<T: Into<Value>>(&self, addr: u16, value: T) -> Result<(), MiniDSPError> {
        self.roundtrip(Commands::Write {
            addr,
            value: value.into(),
        })
        .await?
        .into_ack()
        .err_into()
    }

    /// Reads floats (using `read_floats`) using the least amount of commands possible
    pub async fn read_floats_multi<T: IntoIterator<Item = u16>>(
        &self,
        addrs: T,
    ) -> Result<Vec<f32>, MiniDSPError> {
        let mut addrs: Vec<_> = addrs.into_iter().collect();
        addrs.sort_unstable();

        let mut addrs = addrs.into_iter();
        let mut output = Vec::with_capacity(addrs.len());

        // Break the reads into chunks that fit into the the max packet size
        loop {
            let mut begin: Option<u16> = None;
            let chunk: Vec<_> = {
                (&mut addrs).take_while(|&i| match begin {
                    None => {
                        begin = Some(i);
                        true
                    }
                    Some(val) => (i - val) < commands::READ_FLOATS_MAX as u16,
                })
            }
            .collect();

            if chunk.is_empty() {
                break;
            }

            let min_addr = *chunk.first().unwrap();
            let max_addr = *chunk.last().unwrap();
            let range = max_addr - min_addr + 1;
            let data = self.read_floats(min_addr, range as u8).await?;
            for addr in chunk {
                output.push(data.get(addr));
            }
        }

        Ok(output)
    }
}
