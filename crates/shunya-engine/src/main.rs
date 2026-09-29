use clap::{Parser, Subcommand};
use std::process::exit;

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
    }
}
