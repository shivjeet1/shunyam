slint::include_modules!();

use shunya_proto::v1::device_service_client::DeviceServiceClient;
use shunya_proto::v1::job_service_client::JobServiceClient;
use shunya_proto::v1::certificate_service_client::CertificateServiceClient;
use shunya_proto::v1::{
    ListDevicesRequest, StartJobRequest, GetChallengeRequest,
    StreamEventsRequest, GetCertificateRequest,
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
    let rt_poll = rt.clone();
    let ui_poll = ui.as_weak();
    rt_poll.spawn(async move {
        let mut was_connected = false;
        loop {
            let ok = tokio::time::timeout(
                Duration::from_millis(600),
                DeviceServiceClient::connect("http://127.0.0.1:9090"),
            )
            .await
            .map(|r| r.is_ok())
            .unwrap_or(false);

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

            // On first successful connect: fetch devices and switch to "ready"
            if ok && !was_connected {
                if let Ok(Ok(mut client)) = tokio::time::timeout(
                    Duration::from_millis(600),
                    DeviceServiceClient::connect("http://127.0.0.1:9090"),
                )
                .await
                {
                    if let Ok(resp) = client.list_devices(ListDevicesRequest {}).await {
                        let devs: Vec<Device> = resp
                            .into_inner()
                            .devices
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
                            })
                            .collect();

                        let ui_c2 = ui_poll.clone();
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_c2.upgrade() {
                                ui.set_devices(
                                    Rc::new(VecModel::from(devs)).into(),
                                );
                                ui.set_ui_state("ready".into());
                            }
                        });
                    }
                }
            }

            // If daemon dropped while inside app: revert to setup
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
                Command::new(&path)
                    .stdout(Stdio::from(f.try_clone().unwrap()))
                    .stderr(Stdio::from(f))
                    .spawn()
                    .is_ok()
            } else {
                Command::new(&path).spawn().is_ok()
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
                        "❌ shunyad binary not found. Run `make install` first.".into(),
                    );
                }
            });
        } else {
            let msg = format!("Spawned shunyad from {}", used_path);
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_c.upgrade() {
                    ui.set_daemon_status_msg(msg.into());
                }
            });
        }
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

        rt_wipe.spawn(async move {
            let mut client =
                match JobServiceClient::connect("http://127.0.0.1:9090").await {
                    Ok(c) => c,
                    Err(_) => return,
                };

            let req = StartJobRequest {
                device_id: dev_id.clone(),
                challenge_response: response,
                requested_method: method,
            };

            let job_id = match client.start_job(req).await {
                Ok(res) => res.into_inner().job_id,
                Err(e) => {
                    println!("start_job error: {}", e);
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
                    let dev = dev_id.clone();
                    let ui_cc = ui_c.clone();

                    let phase = match step.as_str() {
                        "Probing" | "Wiping" => "wiping",
                        "Verifying" | "Carving" => "verifying",
                        "Done" => "done",
                        _ => "wiping",
                    }
                    .to_string();

                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_cc.upgrade() {
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

    ui.run()
}
