use clap::Parser;
use shunya_io::wiper::Wiper;
use std::process::exit;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Device path to wipe
    #[arg(short, long)]
    device: String,

    /// Capacity of the device in bytes
    #[arg(short, long)]
    capacity: u64,
}

fn main() {
    let args = Args::parse();

    println!("Starting wipe engine for device: {}", args.device);
    println!("Capacity to overwrite: {} bytes", args.capacity);

    // Provide a deterministic or random seed depending on the use case.
    // For now, deterministic just for the skeleton.
    let seed: [u8; 32] = [42; 32];
    let mut wiper = Wiper::new(&args.device, args.capacity, seed);

    match wiper.overwrite() {
        Ok(_) => {
            println!("Successfully overwrote {}", args.device);
        }
        Err(e) => {
            eprintln!("Failed to wipe device: {:?}", e);
            exit(1);
        }
    }
}
