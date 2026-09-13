slint::include_modules!();

use shunya_proto::v1::device_service_client::DeviceServiceClient;
use shunya_proto::v1::job_service_client::JobServiceClient;
use shunya_proto::v1::{ListDevicesRequest, StartJobRequest, GetChallengeRequest, StreamEventsRequest};
use tokio::runtime::Runtime;
use std::sync::Arc;
use std::rc::Rc;
use slint::{VecModel, Model, SharedString};
use std::time::Duration;
use std::process::{Command, Stdio};
use std::fs::File;

fn main() -> Result<(), slint::PlatformError> {
    let ui = MainWindow::new()?;

    let rt = Arc::new(Runtime::new().unwrap());
    let ui_handle = ui.as_weak();

    // 1. Daemon Poller and Device Fetcher
    let rt_clone = rt.clone();
    let ui_handle_poll = ui_handle.clone();
    rt_clone.spawn(async move {
        let mut was_connected = false;
        loop {
            // Use timeout to prevent hanging on network stack
            let connect_future = DeviceServiceClient::connect("http://127.0.0.1:9090");
            let connect_result = tokio::time::timeout(Duration::from_millis(500), connect_future).await;
            
            let is_connected = match connect_result {
                Ok(Ok(_)) => true,
                _ => false,
            };
            
            let ui_handle_clone = ui_handle_poll.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_handle_clone.upgrade() {
                    ui.set_daemon_connected(is_connected);
                    if is_connected {
                        ui.set_daemon_starting(false);
                        ui.set_daemon_status_msg("".into());
                    }
                }
            });

            if is_connected && !was_connected {
                // Just became connected, fetch devices!
                if let Ok(Ok(mut client)) = tokio::time::timeout(Duration::from_millis(500), DeviceServiceClient::connect("http://127.0.0.1:9090")).await {
                    if let Ok(response) = client.list_devices(ListDevicesRequest {}).await {
                        let devices = response.into_inner().devices;
                        let mut ui_devices = Vec::new();
                        
                        for dev in devices {
                            let capacity_str = format!("{:.1} GB", dev.capacity_bytes as f64 / 1_000_000_000.0);
                            ui_devices.push(Device {
                                id: dev.id.into(),
                                model: dev.model.into(),
                                capacity: capacity_str.into(),
                                status: "Ready".into(),
                                progress: 0.0,
                            });
                        }

                        let ui_handle_clone2 = ui_handle_poll.clone();
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_handle_clone2.upgrade() {
                                let model = Rc::new(VecModel::from(ui_devices));
                                ui.set_devices(model.into());
                                // Do not jump straight to active; go to "ready" confirmation screen
                                ui.set_ui_state("ready".into());
                            }
                        });
                    }
                }
            }
            
            if !is_connected && was_connected {
                let ui_handle_clone3 = ui_handle_poll.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_handle_clone3.upgrade() {
                        let empty: Vec<Device> = Vec::new();
                        let model = Rc::new(VecModel::from(empty));
                        ui.set_devices(model.into());
                        // Forcibly revert to setup screen if daemon crashes
                        ui.set_ui_state("setup".into());
                    }
                });
            }

            was_connected = is_connected;
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });

    // 2. Handle Start Daemon
    let ui_handle_daemon = ui.as_weak();
    ui.on_start_daemon(move || {
        let home = std::env::var("HOME").unwrap_or_default();
        let paths = vec![
            "shunyad".to_string(),
            format!("{}/.local/bin/shunyad", home),
            "/usr/local/bin/shunyad".to_string(),
            "./build/shunyad".to_string(),
            "../build/shunyad".to_string(),
            "../../build/shunyad".to_string(),
        ];
        
        let mut spawned = false;
        let mut spawn_path = String::new();
        for path in paths {
            if let Ok(file) = File::create("/tmp/shunyad-gui.log") {
                if Command::new(&path)
                    .stdout(Stdio::from(file.try_clone().unwrap()))
                    .stderr(Stdio::from(file))
                    .spawn()
                    .is_ok() 
                {
                    spawned = true;
                    spawn_path = path;
                    break;
                }
            } else {
                if Command::new(&path).spawn().is_ok() {
                    spawned = true;
                    spawn_path = path;
                    break;
                }
            }
        }
        
        let ui_handle_clone = ui_handle_daemon.clone();
        if !spawned {
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_handle_clone.upgrade() {
                    ui.set_daemon_starting(false);
                    ui.set_daemon_status_msg("Failed to find or spawn shunyad binary.".into());
                }
            });
        } else {
            let msg = format!("Spawned shunyad from {}", spawn_path);
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_handle_clone.upgrade() {
                    // Do not reset daemon_starting yet, let the polling loop reset it when connected
                    ui.set_daemon_status_msg(msg.into());
                }
            });
        }
    });

    let rt_clone2 = rt.clone();
    let ui_handle2 = ui.as_weak();
    // 3. Handle Wipe action
    ui.on_start_wipe(move |device_id, challenge_response, requested_method| {
        let dev_id = device_id.to_string();
        let response = challenge_response.to_string();
        let method = requested_method.to_string();
        let ui_handle3 = ui_handle2.clone();

        rt_clone2.spawn(async move {
            let mut client = match DeviceServiceClient::connect("http://127.0.0.1:9090").await {
                Ok(_) => match JobServiceClient::connect("http://127.0.0.1:9090").await {
                    Ok(c) => c,
                    Err(_) => return,
                },
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
                    println!("Failed to start job: {}", e);
                    return;
                }
            };

            let stream_req = StreamEventsRequest { job_id: job_id.clone() };
            if let Ok(mut stream) = client.stream_events(stream_req).await.map(|r| r.into_inner()) {
                while let Ok(Some(event)) = stream.message().await {
                    let step = event.step_name.clone();
                    let progress = event.progress_percent;
                    let dev_id_clone = dev_id.clone();
                    
                    let ui_handle_clone = ui_handle3.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_handle_clone.upgrade() {
                            let devices_model = ui.get_devices();
                            for i in 0..devices_model.row_count() {
                                if let Some(mut d) = devices_model.row_data(i) {
                                    if d.id == dev_id_clone.as_str() {
                                        d.status = step.clone().into();
                                        d.progress = progress;
                                        devices_model.set_row_data(i, d);
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
    
    // 4. Handle Generate Cert
    let rt_clone_cert = rt.clone();
    let ui_handle_cert = ui.as_weak();
    ui.on_generate_cert(move |device_id, operator_id| {
        let dev_id = device_id.to_string();
        let _op_id = operator_id.to_string(); // In a real app we'd pass this to the backend
        let ui_handle_clone = ui_handle_cert.clone();
        
        rt_clone_cert.spawn(async move {
            let mut client = match shunya_proto::v1::certificate_service_client::CertificateServiceClient::connect("http://127.0.0.1:9090").await {
                Ok(c) => c,
                Err(_) => {
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_handle_clone.upgrade() {
                            ui.set_cert_generating(false);
                            ui.set_cert_status_msg("Failed to connect to Cert Service".into());
                        }
                    });
                    return;
                }
            };
            
            // Generate a dummy job_id or fetch real one
            let req = shunya_proto::v1::GetCertificateRequest {
                job_id: dev_id.clone(), // Normally job_id, using dev_id for demo
            };
            
            match client.get_certificate(req).await {
                Ok(_) => {
                    let ui_clone = ui_handle_clone.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_clone.upgrade() {
                            ui.set_cert_generating(false);
                            ui.set_cert_status_msg("Certificate Generated Successfully and saved to disk!".into());
                        }
                    });
                }
                Err(e) => {
                    let ui_clone = ui_handle_clone.clone();
                    let msg = format!("Failed to generate: {}", e);
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_clone.upgrade() {
                            ui.set_cert_generating(false);
                            ui.set_cert_status_msg(msg.into());
                        }
                    });
                }
            }
        });
    });
    
    // 4. Handle Challenge Request
    let rt_clone3 = rt.clone();
    let ui_handle4 = ui.as_weak();
    
    ui.on_request_challenge(move |device_id| {
        let dev_id = device_id.to_string();
        let ui_handle5 = ui_handle4.clone();
        
        rt_clone3.spawn(async move {
            let mut client = match JobServiceClient::connect("http://127.0.0.1:9090").await {
                Ok(c) => c,
                Err(_) => return,
            };
            
            if let Ok(res) = client.get_challenge(GetChallengeRequest { device_id: dev_id }).await {
                let challenge = res.into_inner().challenge_string;
                let ui_handle_clone = ui_handle5.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_handle_clone.upgrade() {
                        ui.set_active_challenge(challenge.into());
                    }
                });
            }
        });
    });

    ui.run()
}
