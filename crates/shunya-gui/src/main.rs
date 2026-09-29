slint::include_modules!();

use shunya_proto::v1::device_service_client::DeviceServiceClient;
use shunya_proto::v1::job_service_client::JobServiceClient;
use shunya_proto::v1::certificate_service_client::CertificateServiceClient;
use shunya_proto::v1::{
    ListDevicesRequest, StartJobRequest, GetChallengeRequest,
    StreamEventsRequest, GetCertificateRequest, VerifyCertificateRequest,
};
use tokio::runtime::Runtime;
use std::sync::Arc;
use std::rc::Rc;
use slint::{VecModel, Model};
use std::time::Duration;
use std::process::{Command, Stdio};
use std::fs::File;

fn main() -> Result<(), slint::PlatformError> {
    let ui = MainWindow::new()?;
    let rt = Arc::new(Runtime::new().unwrap());

    // ── 1. Daemon health-check poller ──────────────────────────────────────
    // NOTE: tonic connect() is lazy — it ALWAYS returns Ok even if nothing
    // is listening. A real RPC call MUST be made to prove the server is alive.
    let rt_poll = rt.clone();
    let ui_poll = ui.as_weak();
    rt_poll.spawn(async move {
        let mut was_connected = false;
        loop {
            let probe = tokio::time::timeout(Duration::from_millis(800), async {
                let mut c = DeviceServiceClient::connect("http://127.0.0.1:9090")
                    .await
                    .map_err(|e| tonic::Status::unavailable(e.to_string()))?;
                c.list_devices(ListDevicesRequest {}).await
            })
            .await;

            let (ok, devices_opt) = match probe {
                Ok(Ok(resp)) => (true, Some(resp.into_inner().devices)),
                _ => (false, None),
            };

            // Update daemon status indicator
            let ui_c = ui_poll.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_c.upgrade() {
                    ui.set_daemon_connected(ok);
                    if ok {
                        ui.set_daemon_starting(false);
                        ui.set_daemon_status_msg("".into());
                    }
                }
            });

            // On first successful probe: populate devices and switch to "ready"
            if ok && !was_connected {
                if let Some(raw_devs) = devices_opt {
                    let has_nvme = raw_devs.iter().any(|d| d.class == "nvme-ssd");
                    let devs: Vec<Device> = raw_devs
                        .into_iter()
                        .map(|d| Device {
                            id: d.id.into(),
                            model: d.model.into(),
                            capacity: format!(
                                "{:.1} GB",
                                d.capacity_bytes as f64 / 1_000_000_000.0
                            )
                            .into(),
                            status: "Ready".into(),
                            progress: 0.0,
                            phase: "idle".into(),
                            job_id: "".into(),
                            dev_class: d.class.into(),
                            is_system: d.is_system_disk,
                        })
                        .collect();

                    let ui_c2 = ui_poll.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_c2.upgrade() {
                            if has_nvme {
                                ui.set_selected_method("NIST 800-88 Cryptographic Erase (Crypto Erase)".into());
                            }
                            ui.set_devices(Rc::new(VecModel::from(devs)).into());
                            ui.set_ui_state("ready".into());
                        }
                    });
                }
            }

            // If daemon dropped while in app: revert to setup
            if !ok && was_connected {
                let ui_c3 = ui_poll.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_c3.upgrade() {
                        ui.set_devices(Rc::new(VecModel::from(Vec::<Device>::new())).into());
                        ui.set_ui_state("setup".into());
                    }
                });
            }

            was_connected = ok;
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });

    // ── 2. Start Daemon ────────────────────────────────────────────────────
    let ui_daemon = ui.as_weak();
    ui.on_start_daemon(move || {
        let home = std::env::var("HOME").unwrap_or_default();
        let candidates = vec![
            "shunyad".to_string(),
            format!("{}/.local/bin/shunyad", home),
            "/usr/local/bin/shunyad".to_string(),
            "./build/shunyad".to_string(),
            "../build/shunyad".to_string(),
        ];

        let mut launched = false;
        let mut used_path = String::new();
        for path in candidates {
            let log_file = File::create("/tmp/shunyad-gui.log").ok();
            let ok = if let Some(f) = log_file {
                Command::new("pkexec")
                    .arg(&path)
                    .stdout(Stdio::from(f.try_clone().unwrap()))
                    .stderr(Stdio::inherit())
                    .spawn()
                    .is_ok()
            } else {
                Command::new("pkexec").arg(&path).spawn().is_ok()
            };
            if ok {
                launched = true;
                used_path = path;
                break;
            }
        }

        let ui_c = ui_daemon.clone();
        if !launched {
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_c.upgrade() {
                    ui.set_daemon_starting(false);
                    ui.set_daemon_status_msg(
                        "❌ pkexec failed or shunyad binary not found.".into(),
                    );
                }
            });
        } else {
            let msg = format!("Elevating privileges for {}", used_path);
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_c.upgrade() {
                    ui.set_daemon_status_msg(msg.into());
                }
            });
        }
    });

    // ── 2.5. Refresh Drives ────────────────────────────────────────────────
    let rt_refresh = rt.clone();
    let ui_refresh = ui.as_weak();
    ui.on_refresh_drives(move || {
        let ui_c = ui_refresh.clone();
        rt_refresh.spawn(async move {
            if let Ok(mut client) = DeviceServiceClient::connect("http://127.0.0.1:9090").await {
                if let Ok(res) = client.list_devices(ListDevicesRequest {}).await {
                    let raw_devs = res.into_inner().devices;
                    let has_nvme = raw_devs.iter().any(|d| d.class == "nvme-ssd");
                    let devs: Vec<Device> = raw_devs
                        .into_iter()
                        .map(|d| Device {
                            id: d.id.into(),
                            model: d.model.into(),
                            capacity: format!(
                                "{:.1} GB",
                                d.capacity_bytes as f64 / 1_000_000_000.0
                            )
                            .into(),
                            status: "Ready".into(),
                            progress: 0.0,
                            phase: "idle".into(),
                            job_id: "".into(),
                            dev_class: d.class.into(),
                            is_system: d.is_system_disk,
                        })
                        .collect();

                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_c.upgrade() {
                            ui.set_selected_device_index(-1);
                            ui.set_active_challenge("".into());
                            ui.set_cert_status_msg("".into());
                            if has_nvme {
                                ui.set_selected_method("NIST 800-88 Cryptographic Erase (Crypto Erase)".into());
                            }
                            ui.set_devices(Rc::new(VecModel::from(devs)).into());
                        }
                    });
                }
            }
        });
    });

    // ── 3. Request challenge ───────────────────────────────────────────────
    let rt_ch = rt.clone();
    let ui_ch = ui.as_weak();
    ui.on_request_challenge(move |device_id| {
        let dev_id = device_id.to_string();
        let ui_c = ui_ch.clone();
        rt_ch.spawn(async move {
            if let Ok(mut client) =
                JobServiceClient::connect("http://127.0.0.1:9090").await
            {
                if let Ok(res) =
                    client.get_challenge(GetChallengeRequest { device_id: dev_id }).await
                {
                    let challenge = res.into_inner().challenge_string;
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_c.upgrade() {
                            ui.set_active_challenge(challenge.into());
                        }
                    });
                }
            }
        });
    });

    // ── 4. Start wipe + stream events ─────────────────────────────────────
    let rt_wipe = rt.clone();
    let ui_wipe = ui.as_weak();
    ui.on_start_wipe(move |device_id, challenge_response, requested_method| {
        let dev_id = device_id.to_string();
        let response = challenge_response.to_string();
        let method = requested_method.to_string();
        let ui_c = ui_wipe.clone();
        let ui_clear = ui_c.clone();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = ui_clear.upgrade() {
                ui.set_wipe_error_msg("".into());
            }
        });


        rt_wipe.spawn(async move {
            let mut client =
                match JobServiceClient::connect("http://127.0.0.1:9090").await {
                    Ok(c) => c,
                    Err(_) => return,
                };

            let req = StartJobRequest {
                device_id: dev_id.clone(),
                challenge_response: response,
                requested_method: method, job_id: "".to_string(), capacity_bytes: 0,
            };

            let job_id = match client.start_job(req).await {
                Ok(res) => res.into_inner().job_id,
                Err(e) => {
                    let err_str = format!("Failed to start sanitization: {}", e.message());
                    let ui_cc = ui_c.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_cc.upgrade() {
                            ui.set_wipe_error_msg(err_str.into());
                        }
                    });
                    return;
                }
            };

            // Store job_id in device entry and clear challenge UI
            {
                let jid = job_id.clone();
                let dev = dev_id.clone();
                let ui_cc = ui_c.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_cc.upgrade() {
                        ui.set_active_challenge("".into());
                        let model = ui.get_devices();
                        for i in 0..model.row_count() {
                            if let Some(mut d) = model.row_data(i) {
                                if d.id == dev.as_str() {
                                    d.job_id = jid.clone().into();
                                    d.phase = "wiping".into();
                                    model.set_row_data(i, d);
                                    break;
                                }
                            }
                        }
                    }
                });
            }

            // Stream events
            let stream_req = StreamEventsRequest { job_id: job_id.clone() };
            if let Ok(mut stream) = client
                .stream_events(stream_req)
                .await
                .map(|r| r.into_inner())
            {
                while let Ok(Some(event)) = stream.message().await {
                    let step = event.step_name.clone();
                    let progress = event.progress_percent;
                    let status_msg = event.message.clone();
                    let status = event.status.clone();
                    let dev = dev_id.clone();
                    let ui_cc = ui_c.clone();

                    let phase = if status == "Failed" {
                        "error".to_string()
                    } else {
                        match step.to_lowercase().as_str() {
                            "probing" | "wiping" => "wiping",
                            "verifying" | "carving" => "verifying",
                            "done" => "done",
                            _ => "wiping",
                        }
                        .to_string()
                    };

                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_cc.upgrade() {
                            if status == "Failed" {
                                ui.set_wipe_error_msg(status_msg.clone().into());
                            }
                            let model = ui.get_devices();
                            for i in 0..model.row_count() {
                                if let Some(mut d) = model.row_data(i) {
                                    if d.id == dev.as_str() {
                                        d.status = status_msg.clone().into();
                                        d.progress = progress;
                                        d.phase = phase.clone().into();
                                        model.set_row_data(i, d);
                                        break;
                                    }
                                }
                            }
                        }
                    });
                }
            }
        });
    });

    // ── 5. Generate certificate ────────────────────────────────────────────
    let rt_cert = rt.clone();
    let ui_cert = ui.as_weak();
    ui.on_generate_cert(move |job_id, operator_id| {
        let jid = job_id.to_string();
        let _op = operator_id.to_string();
        let ui_c = ui_cert.clone();

        rt_cert.spawn(async move {
            match CertificateServiceClient::connect("http://127.0.0.1:9090").await {
                Ok(mut client) => {
                    match client
                        .get_certificate(GetCertificateRequest { job_id: jid })
                        .await
                    {
                        Ok(res) => {
                            let qr = res.into_inner().qr_payload;
                            let msg = format!(
                                "✅ Certificate signed & saved!  QR: {}",
                                qr
                            );
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(ui) = ui_c.upgrade() {
                                    ui.set_cert_generating(false);
                                    ui.set_cert_status_msg(msg.into());
                                }
                            });
                        }
                        Err(e) => {
                            let msg = format!("Cert error: {}", e);
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(ui) = ui_c.upgrade() {
                                    ui.set_cert_generating(false);
                                    ui.set_cert_status_msg(msg.into());
                                }
                            });
                        }
                    }
                }
                Err(_) => {
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_c.upgrade() {
                            ui.set_cert_generating(false);
                            ui.set_cert_status_msg(
                                "Failed to connect to CertificateService".into(),
                            );
                        }
                    });
                }
            }
        });
    });

    // ── 6. Verify certificate ──────────────────────────────────────────────
    let rt_verify = rt.clone();
    let ui_verify = ui.as_weak();
    ui.on_verify_certificate(move |device_id| {
        let dev_id = device_id.to_string();
        let ui_c = ui_verify.clone();

        rt_verify.spawn(async move {
            match CertificateServiceClient::connect("http://127.0.0.1:9090").await {
                Ok(mut client) => {
                    match client
                        .verify_certificate(VerifyCertificateRequest { device_id: dev_id })
                        .await
                    {
                        Ok(res) => {
                            let resp = res.into_inner();
                            let is_valid = resp.is_valid;
                            let manifest = resp.manifest_json;
                            let err_msg = resp.error_message;
                            
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(ui) = ui_c.upgrade() {
                                    if is_valid {
                                        ui.set_verify_status("success".into());
                                        ui.set_verify_manifest_json(manifest.into());
                                    } else {
                                        ui.set_verify_status("error".into());
                                        ui.set_verify_error_message(err_msg.into());
                                    }
                                }
                            });
                        }
                        Err(e) => {
                            let msg = format!("RPC error: {}", e);
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(ui) = ui_c.upgrade() {
                                    ui.set_verify_status("error".into());
                                    ui.set_verify_error_message(msg.into());
                                }
                            });
                        }
                    }
                }
                Err(_) => {
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_c.upgrade() {
                            ui.set_verify_status("error".into());
                            ui.set_verify_error_message("Failed to connect to Daemon".into());
                        }
                    });
                }
            }
        });
    });

    // ── 7. Start recovery ──────────────────────────────────────────────
    let rt_recover = rt.clone();
    let ui_recover = ui.as_weak();
    ui.on_start_recovery(move |device_id, output_dir, profile| {
        let dev_id = device_id.to_string();
        let out_dir = output_dir.to_string();
        let prof = profile.to_string();
        let ui_c = ui_recover.clone();

        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = ui_c.upgrade() {
                ui.set_recovery_status("scanning".into());
            }
        });

        let ui_c = ui_recover.clone();
        rt_recover.spawn(async move {
            match shunya_proto::v1::recovery_service_client::RecoveryServiceClient::connect("http://127.0.0.1:9090").await {
                Ok(mut client) => {
                    let req = shunya_proto::v1::StartRecoveryRequest {
                        source_device_id: dev_id,
                        output_directory: out_dir,
                        profile: prof,
                    };
                    match client.start_recovery(req).await {
                        Ok(_) => {
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(ui) = ui_c.upgrade() {
                                    ui.set_recovery_status("done".into());
                                }
                            });
                        }
                        Err(e) => {
                            let err_msg = format!("Recovery failed: {}", e.message());
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(ui) = ui_c.upgrade() {
                                    ui.set_recovery_status("error".into());
                                    ui.set_recovery_error_msg(err_msg.into());
                                }
                            });
                        }
                    }
                }
                Err(_) => {
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_c.upgrade() {
                            ui.set_recovery_status("error".into());
                        }
                    });
                }
            }
        });
    });

    ui.run()
}
