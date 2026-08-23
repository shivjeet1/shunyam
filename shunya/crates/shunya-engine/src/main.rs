use clap::{Parser, Subcommand};
use shunya_io::wiper::Wiper;
use std::process::exit;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Overwrite a block device
    Wipe {
        /// Device path to wipe
        #[arg(short, long)]
        device: String,

        /// Capacity of the device in bytes
        #[arg(short, long)]
        capacity: u64,
    },
    /// Enumerate storage devices on the system
    ListDevices,
}

fn main() {
    let args = Args::parse();

    match args.command {
        Commands::Wipe { device, capacity } => {
            println!("Starting wipe engine for device: {}", device);
            println!("Capacity to overwrite: {} bytes", capacity);

            let seed: [u8; 32] = [42; 32];
            let mut wiper = Wiper::new(&device, capacity, seed);

            match wiper.overwrite() {
                Ok(_) => {
                    println!("Successfully overwrote {}", device);
                }
                Err(e) => {
                    eprintln!("Failed to wipe device: {:?}", e);
                    exit(1);
                }
            }
        }
        Commands::ListDevices => {
            match shunya_device::enumerate_devices() {
                Ok(devices) => {
                    for dev in devices {
                        println!("Device: {} (Model: {}, NVMe: {})", dev.path, dev.model, dev.is_nvme);
                    }
                }
                Err(e) => {
                    eprintln!("Failed to enumerate devices: {:?}", e);
                    exit(1);
                }
            }
        }
    }
}
