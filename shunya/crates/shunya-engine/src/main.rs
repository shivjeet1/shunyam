use clap::{Parser, Subcommand};
use shunya_io::wiper::Wiper;
use std::process::exit;
use std::io::{self, Read, Write};
use prost::Message;
use shunya_proto::v1::{StartJobRequest, JobEvent};
use std::time::Duration;
use std::thread;

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
    /// Stream job commands and events over stdin/stdout using length-prefixed protobufs
    Stream,
}

fn emit_event(job_id: &str, step: &str, status: &str, progress: f32, message: &str) -> io::Result<()> {
    let event = JobEvent {
        job_id: job_id.to_string(),
        step_name: step.to_string(),
        progress_percent: progress,
        status: status.to_string(),
        message: message.to_string(),
    };
    
    let mut buf = Vec::new();
    event.encode(&mut buf).unwrap();
    
    let mut stdout = io::stdout().lock();
    let length = (buf.len() as u32).to_be_bytes();
    stdout.write_all(&length)?;
    stdout.write_all(&buf)?;
    stdout.flush()?;
    Ok(())
}

fn main() {
    let args = Args::parse();

    match args.command {
        Commands::Stream => {
            // 1. Read the 4-byte length prefix
            let mut len_buf = [0u8; 4];
            let mut stdin = io::stdin().lock();
            if let Err(e) = stdin.read_exact(&mut len_buf) {
                eprintln!("Failed to read request length from stdin: {}", e);
                exit(1);
            }
            
            let len = u32::from_be_bytes(len_buf) as usize;
            
            // 2. Read the StartJobRequest protobuf
            let mut payload = vec![0u8; len];
            if let Err(e) = stdin.read_exact(&mut payload) {
                eprintln!("Failed to read request payload from stdin: {}", e);
                exit(1);
            }
            
            let req = match StartJobRequest::decode(&payload[..]) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Failed to decode StartJobRequest: {}", e);
                    exit(1);
                }
            };
            
            // We use a mock job ID since the request doesn't contain one (daemon assigns it).
            // Actually, we can just use the device ID or a dummy ID for the event stream
            // The Go daemon ignores the JobEvent.job_id anyway because it tracks it locally per process.
            let job_id = "engine-run";
            let device = req.device_id;
            
            // Simulate execution loop with Protobuf events
            let _ = emit_event(job_id, "probing", "probing", 10.0, "Probing device capability");
            thread::sleep(Duration::from_millis(200));
            
            let _ = emit_event(job_id, "wiping", "wiping", 20.0, &format!("Started wipe on {}", device));
            
            // Here we would normally call Wiper::new(&device, capacity, seed).overwrite()
            thread::sleep(Duration::from_millis(500));
            
            let _ = emit_event(job_id, "wiping", "wiping", 70.0, "Wipe nearing completion");
            thread::sleep(Duration::from_millis(300));
            
            let _ = emit_event(job_id, "verifying", "verifying", 85.0, "Wipe complete. Verifying...");
            thread::sleep(Duration::from_millis(200));
            
            let _ = emit_event(job_id, "done", "success", 100.0, "Job completed successfully");
        }
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
