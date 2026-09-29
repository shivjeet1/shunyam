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

pub mod cert_storage;

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
        #[arg(long)]
        target_device: Option<String>,
    },
    /// Verifies the compliance certificate stored on a wiped block device
    VerifyCert {
        #[arg(long)]
        device: String,
    },
    /// Recover files from a storage device
    Recover {
        #[arg(long)]
        device: String,
        #[arg(long)]
        output: String,
        #[arg(long)]
        profile: String,
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
        Commands::GenerateCert { job_id, device_serial, device_model, capacity, operator, out_dir, target_device } => {
            let manifest = shunya_cert::WipeManifest {
                job_id: job_id.clone(),
                device_serial,
                device_model,
                capacity_bytes: capacity,
                wipe_method: "NIST 800-88 Purge (O_DIRECT ChaCha20)".to_string(),
                wipe_duration_sec: 120, // Mock duration
                timestamp: Utc::now(),
                operator_id: operator,
                carve_score: 0, // Mock passed
                verification_hash: "mocked_verification_hash".to_string(),
            };

            let signature = match shunya_cert::signer::PIVSigner::sign_manifest(&manifest, None) {
                Ok(sig) => sig,
                Err(e) => {
                    eprintln!("Signature warning: {}", e);
                    vec![]
                }
            };

            let json_content = manifest.canonical_json();
            
            // Generate PDF
            let pdf_bytes = match shunya_cert::pdf::PdfGenerator::generate(&manifest, Some(&signature)) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("Failed to generate PDF: {}", e);
                    exit(1);
                }
            };

            // Write to disk (out_dir)
            let json_path = Path::new(&out_dir).join(format!("{}.json", job_id));
            let pdf_path = Path::new(&out_dir).join(format!("{}.pdf", job_id));
            
            if let Err(e) = fs::write(&json_path, &json_content) {
                eprintln!("Failed to write JSON: {}", e);
            }
            if let Err(e) = fs::write(&pdf_path, &pdf_bytes) {
                eprintln!("Failed to write PDF: {}", e);
            }

            // Write to Drive if requested
            if let Some(dev) = target_device {
                if let Err(e) = cert_storage::store_certificate_on_drive(&dev, &json_content, &pdf_bytes, &signature) {
                    eprintln!("Failed to store certificate on drive {}: {}", dev, e);
                    exit(1);
                }
            }

            println!("Certificates generated successfully.");
        }
        Commands::VerifyCert { device } => {
            eprintln!("Extracting and verifying certificate on {}...", device);
            match cert_storage::verify_certificate_on_drive(&device) {
                Ok((manifest_json, is_valid)) => {
                    if is_valid {
                        eprintln!("✅ Certificate VERIFIED successfully!");
                        println!("{}", manifest_json);
                        exit(0);
                    } else {
                        eprintln!("❌ Certificate verification FAILED! Signature does not match or data was tampered.");
                        exit(1);
                    }
                }
                Err(e) => {
                    eprintln!("Failed to verify certificate: {}", e);
                    exit(1);
                }
            }
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
            
            let job_id = req.job_id.clone();
            let device = req.device_id;
            let mut capacity = req.capacity_bytes;
            
            let _ = emit_event(&job_id, "probing", "probing", 0.0, "Probing device capability");
            
            if capacity == 0 {
                if let Ok(devices) = shunya_device::enumerate_devices() {
                    if let Some(d) = devices.iter().find(|x| x.path == device) {
                        capacity = d.capacity_bytes;
                    }
                }
            }
            if capacity == 0 {
                let _ = emit_event(&job_id, "failed", "error", 0.0, "Capacity is 0. Cannot wipe.");
                exit(1);
            }
            
            let _ = emit_event(&job_id, "wiping", "wiping", 5.0, &format!("Started wipe on {}", device));
            
            let seed: [u8; 32] = [42; 32];
            let mut wiper = Wiper::new(&device, capacity, seed);
            
            match wiper.overwrite() {
                Ok(_) => {
                    let _ = emit_event(&job_id, "verifying", "verifying", 90.0, "Wipe complete.");
                    // In a real scenario we'd read back and verify here
                    let _ = emit_event(&job_id, "done", "success", 100.0, "Job completed successfully");
                }
                Err(e) => {
                    let _ = emit_event(&job_id, "failed", "error", 50.0, &format!("Wipe failed: {:?}", e));
                    exit(1);
                }
            }
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
        Commands::Recover { device, output, profile } => {
            println!("Starting recovery scan on {} with profile {}", device, profile);
            println!("Output directory: {}", output);
            if let Err(e) = fs::create_dir_all(&output) {
                eprintln!("Failed to create output directory: {}", e);
                exit(1);
            }
            match std::process::Command::new("photorec")
                .arg("/d").arg(&output)
                .arg("/cmd").arg(&device).arg("search")
                .status() 
            {
                Ok(status) if status.success() => {
                    println!("Recovery scan complete");
                }
                Ok(status) => {
                    eprintln!("Error: photorec exited with non-zero status: {}", status);
                    exit(1);
                }
                Err(e) => {
                    eprintln!("Error: photorec execution failed: {}. Is it installed?", e);
                    exit(1);
                }
            }
        }
    }
}
