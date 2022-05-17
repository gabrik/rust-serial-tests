use clap::Parser;
use std::{time::Duration, process::exit};

mod zserial;

use zserial::ZSerial;

static DEFAULT_INTERVAL: &str = "1.0";
static DEFAULT_RATE: &str = "9600";

#[derive(Parser, Debug)]
struct Args {
    port: String,
    #[clap(short, long)]
    server: bool,
    #[clap(short, long, default_value = DEFAULT_RATE)]
    baud_rate: u32,
    #[clap(short, long, default_value = DEFAULT_INTERVAL)]
    interval: f64,
}

#[tokio::main]
async fn main() -> tokio_serial::Result<()> {
    let args = Args::parse();
    let mut buff = [0u8; 65535];

    println!("Arguments: {:?}", args);

    let mut port = ZSerial::new(args.port, args.baud_rate)?;

    if args.server {
        loop {
            let read = port.read_msg(&mut buff).await?;
            if read > 0 {
                println!(">> Read {read} bytes: {:02X?}", &buff[0..read]);

                port.write(&buff[..read]).await?;

                println!("<< Echoed back");
            }



            // port.dump().await?;
        }
    } else {
        let mut count = 1usize;
        let mut lost = 0usize;


        let timeout_duration = if args.interval > 0.5 {
            3.0*args.interval
        } else {
            2.0
        };

        loop {
            tokio::time::sleep(Duration::from_secs_f64(args.interval)).await;



            let data = count.to_ne_bytes();

            port.write(&data).await?;

            println!("<< Wrote {} bytes bytes: {:02X?}", data.len(), data);

            let timeout = async move {
                tokio::time::sleep(Duration::from_secs_f64(timeout_duration)).await;
            };

            let _out = tokio::select! {
                res = port.read_msg(&mut buff) => {
                    let read = res?;
                    if read > 0 {
                        println!(">> Read {read} bytes: {:02X?}", &buff[0..read]);
                        println!("Read: {}", usize::from_ne_bytes(buff[..read].try_into().expect("slice with incorrect length")));
                    }
                    count = count.wrapping_add(1);
                },
                _ = timeout => {
                    count = count.wrapping_add(1);
                    lost = lost.wrapping_add(1);
                },
                _ = tokio::signal::ctrl_c() => {
                    println!("Sent a total of {count} messages, lost {lost}");
                    exit(0);
                }
            };

            // let read = port.read_msg(&mut buff).await?;
            // if read > 0 {
            //     println!(">> Read {read} bytes: {:02X?}", &buff[0..read]);
            //     println!("Read: {}", usize::from_ne_bytes(buff[..read].try_into().expect("slice with incorrect length")));
            // }


        }
    }
}
