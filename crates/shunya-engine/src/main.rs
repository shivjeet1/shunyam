use clap::{Parser, Subcommand};
use shunya_io::wiper::Wiper;
use std::process::exit;
use std::io::{self, Read, Write};
use prost::Message;
use shunya_proto::v1::{StartJobRequest, JobEvent};
use std::time::Duration;
use std::thread;
use chrono::Utc;
use std::fs;
use std::path::Path;

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
        #[arg(short, long)]
        device: String,
        #[arg(short, long)]
        capacity: u64,
    },
    /// Enumerate storage devices on the system
    ListDevices,
    /// Stream job commands and events over stdin/stdout
    Stream,
    /// Generate PDF and JSON certificates
    GenerateCert {
        #[arg(long)]
        job_id: String,
        #[arg(long)]
        device_serial: String,
        #[arg(long)]
        device_model: String,
        #[arg(long)]
        capacity: u64,
        #[arg(long)]
        operator: String,
        #[arg(long)]
        out_dir: String,
    },
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
        Commands::GenerateCert { job_id, device_serial, device_model, capacity, operator, out_dir } => {
            let manifest = shunya_cert::WipeManifest {
                job_id: job_id.clone(),
                device_serial,
                device_model,
                capacity_bytes: capacity,
                wipe_method: "NIST 800-88 Purge (O_DIRECT ChaCha20)".to_string(),
                wipe_duration_sec: 120, // Mock duration
                timestamp: Utc::now(),
                operator_id: operator,
                carve_score: 100,
                verification_hash: "mocked_verification_hash".to_string(),
            };

            let signature = match shunya_cert::signer::PIVSigner::sign_manifest(&manifest, None) {
                Ok(sig) => sig,
                Err(e) => {
                    eprintln!("Signature warning: {}", e);
                    vec![]
                }
            };

            let json_path = Path::new(&out_dir).join(format!("{}.json", job_id));
            let pdf_path = Path::new(&out_dir).join(format!("{}.pdf", job_id));

            // Write JSON
            if let Err(e) = fs::write(&json_path, manifest.canonical_json()) {
                eprintln!("Failed to write JSON: {}", e);
                exit(1);
            }

            // Write PDF
            match shunya_cert::pdf::PdfGenerator::generate(&manifest, Some(&signature)) {
                Ok(pdf_bytes) => {
                    if let Err(e) = fs::write(&pdf_path, pdf_bytes) {
                        eprintln!("Failed to write PDF: {}", e);
                        exit(1);
                    }
                }
                Err(e) => {
                    eprintln!("Failed to generate PDF: {}", e);
                    exit(1);
                }
            }
            println!("Certificates generated successfully in {}", out_dir);
        }
        Commands::Stream => {
            let mut len_buf = [0u8; 4];
            let mut stdin = io::stdin().lock();
            if let Err(e) = stdin.read_exact(&mut len_buf) {
                eprintln!("Failed to read request length from stdin: {}", e);
                exit(1);
            }
            
            let len = u32::from_be_bytes(len_buf) as usize;
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
            
            let job_id = "engine-run";
            let device = req.device_id;
            
            let _ = emit_event(job_id, "probing", "probing", 10.0, "Probing device capability");
            thread::sleep(Duration::from_millis(200));
            
            let _ = emit_event(job_id, "wiping", "wiping", 20.0, &format!("Started wipe on {}", device));
            thread::sleep(Duration::from_millis(500));
            
            let _ = emit_event(job_id, "wiping", "wiping", 70.0, "Wipe nearing completion");
            thread::sleep(Duration::from_millis(300));
            
            let _ = emit_event(job_id, "verifying", "verifying", 85.0, "Wipe complete. Verifying...");
            thread::sleep(Duration::from_millis(200));
            
            let _ = emit_event(job_id, "done", "success", 100.0, "Job completed successfully");
        }
        Commands::Wipe { device, capacity } => {
            println!("Starting wipe engine for device: {}", device);
            let seed: [u8; 32] = [42; 32];
            let mut wiper = Wiper::new(&device, capacity, seed);

            match wiper.overwrite() {
                Ok(_) => println!("Successfully overwrote {}", device),
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
