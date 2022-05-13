use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_serial::{SerialPortBuilderExt, SerialStream};

const MAX_FRAME_SIZE: usize = 1510;
const CRC32_LEN: u16 = 4;

const FRAME_HEADER_TRAILER_LEN: u16 = 10;

const PREAMBLE: [u8; 4] = [0xF0, 0x0F, 0x0F, 0xF0];

//FIXME: just for testing
const CRC32: [u8; 4] = [0xCC, 0x32, 0xCC, 0x32];

const MAX_MTU: usize = 1500;

/// ZSerial Frame Format
///
///
/// +--------+----+------------+--------+
/// |F00F0FF0|XXXX|ZZZZ....ZZZZ|CCCCCCCC|
/// +--------+----+------------+--------+
/// |Preamble| Len|   Data     |  CRC32 |
/// +----4---+-2--+----N-------+---4----+
///
/// Max Frame Size: 65535
/// Max MTU: 1500

pub struct ZSerial {
    port: String,
    baud_rate: u32,
    serial: SerialStream,
    buff: [u8; MAX_FRAME_SIZE],
}

impl ZSerial {
    pub fn new(port: String, baud_rate: u32) -> tokio_serial::Result<Self> {
        let mut serial = tokio_serial::new(port.clone(), baud_rate).open_native_async()?;

        #[cfg(unix)]
        serial.set_exclusive(false)?;

        Ok(Self {
            port,
            baud_rate,
            serial,
            buff: [0u8; MAX_FRAME_SIZE],
        })
    }

    pub async fn dump(&mut self) -> tokio_serial::Result<()> {
        self.serial
            .read_exact(std::slice::from_mut(&mut self.buff[0]))
            .await?;
        println!("Read {:02X?}", self.buff[0]);
        Ok(())
    }

    pub async fn read_msg(&mut self, buff: &mut [u8]) -> tokio_serial::Result<usize> {
        let mut start_count = 0;

        if buff.len() < MAX_MTU {
            return Err(tokio_serial::Error::new(
                tokio_serial::ErrorKind::InvalidInput,
                format!("Recv buffer is too small, required minimum {MAX_MTU}"),
            ));
        }

        loop {
            // Wait for sync preamble: 0xF0 0x0F 0x0F 0xF0

            // Read one byte

            self.serial
                .read_exact(std::slice::from_mut(&mut self.buff[start_count]))
                .await?;

            // println!("Read {:02X?}, count {start_count}", self.buff[start_count]);

            if start_count == 0 {
                if self.buff[start_count] == PREAMBLE[0] {
                    // First sync byte found
                    start_count = 1;
                }
            } else if start_count == 1 {
                if self.buff[start_count] == PREAMBLE[1] {
                    // Second sync byte found
                    start_count = 2;
                }
            } else if start_count == 2 {
                if self.buff[start_count] == PREAMBLE[2] {
                    // Third sync byte found
                    start_count = 3;
                }
            } else if start_count == 3 {
                if self.buff[start_count] == PREAMBLE[3] {
                    // fourth and last sync byte found
                    start_count = 4;

                    // lets read the len now
                    self.serial
                        .read_exact(&mut self.buff[start_count..start_count + 2])
                        .await?;

                    // println!("Read size {:02X?} {:02X?}", self.buff[start_count], self.buff[start_count + 1]);

                    let size: u16 =
                        (self.buff[start_count + 1] as u16) << 8 | self.buff[start_count] as u16;

                    // println!("Wire size {size}");

                    let data_size = (size - FRAME_HEADER_TRAILER_LEN) as usize;

                    //println!("Data size {data_size}");

                    // read the data
                    self.serial.read_exact(&mut buff[0..data_size]).await?;

                    start_count = start_count + 2;

                    //read the CRC32
                    self.serial
                        .read_exact(&mut self.buff[start_count..start_count + 3])
                        .await?;

                    // reading CRC32
                    let _crc: u32 = (self.buff[start_count] as u32) << 24
                        | (self.buff[start_count + 1] as u32) << 16
                        | (self.buff[start_count + 2] as u32) << 8
                        | (self.buff[start_count + 3] as u32);

                    //println!("CRC32 {:02X?} {:02X?} {:02X?} {:02X?} ", self.buff[start_count], self.buff[start_count + 1], self.buff[start_count + 2], self.buff[start_count + 3]);

                    return Ok(data_size);
                }
            } else {
                // We start again looking for a preamble
                start_count = 0;
                println!("No sync!");
                return Ok(0);
            }
        }
    }

    pub async fn read(serial: &mut SerialStream, buff: &mut [u8]) -> tokio_serial::Result<usize> {
        Ok(serial.read(buff).await?)
    }

    pub async fn read_all(serial: &mut SerialStream, buff: &mut [u8]) -> tokio_serial::Result<()> {
        let mut read: usize = 0;
        while read < buff.len() {
            let n = Self::read(serial, &mut buff[read..]).await?;
            read += n;
        }
        Ok(())
    }

    pub async fn write(&mut self, buff: &[u8]) -> tokio_serial::Result<()> {
        if buff.len() > MAX_MTU {
            return Err(tokio_serial::Error::new(
                tokio_serial::ErrorKind::InvalidInput,
                "Payload is too big",
            ));
        }

        // Write the preamble
        self.serial.write_all(&PREAMBLE).await?;

        let wire_size: u16 = buff.len() as u16 + FRAME_HEADER_TRAILER_LEN;

        // println!("Data size {} wire size {wire_size}",buff.len());

        let size_bytes = wire_size.to_ne_bytes();

        // println!("Size on the wire {size_bytes:02X?}");

        // Write the len
        self.serial.write_all(&size_bytes).await?;

        // Write the data
        self.serial.write_all(&buff).await?;

        //Write the CRC32
        self.serial.write_all(&CRC32).await?;

        // self.serial.flush().await?;

        Ok(())
    }

    /// Gets the configured baud rate
    pub fn baud_rate(&self) -> u32 {
        self.baud_rate
    }

    /// Gets the configured serial port
    pub fn port(&self) -> String {
        self.port.clone()
    }
}
