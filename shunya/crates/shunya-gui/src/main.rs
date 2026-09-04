slint::include_modules!();

use shunya_proto::v1::device_service_client::DeviceServiceClient;
use shunya_proto::v1::job_service_client::JobServiceClient;
use shunya_proto::v1::{ListDevicesRequest, StartJobRequest, GetChallengeRequest, StreamEventsRequest};
use tokio::runtime::Runtime;
use std::sync::Arc;
use std::rc::Rc;
use slint::{VecModel, SharedString, Model};

fn main() -> Result<(), slint::PlatformError> {
    let ui = MainWindow::new()?;

    // Create a tokio runtime for background gRPC tasks
    let rt = Arc::new(Runtime::new().unwrap());
    let ui_handle = ui.as_weak();

    // 1. Fetch live devices on startup
    rt.spawn(async move {
        let mut client = match DeviceServiceClient::connect("http://127.0.0.1:9090").await {
            Ok(c) => c,
            Err(e) => {
                println!("Failed to connect to shunyad: {}", e);
                return;
            }
        };

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

            let ui_handle_clone = ui_handle.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_handle_clone.upgrade() {
                    let model = Rc::new(VecModel::from(ui_devices));
                    ui.set_devices(model.into());
                }
            });
        }
    });

    let rt_clone2 = rt.clone();
    let ui_handle2 = ui.as_weak();

    // 2. Handle Wipe action
    ui.on_start_wipe(move |device_id, challenge_response| {
        let dev_id = device_id.to_string();
        let response = challenge_response.to_string();
        let ui_handle3 = ui_handle2.clone();

        rt_clone2.spawn(async move {
            let mut client = match JobServiceClient::connect("http://127.0.0.1:9090").await {
                Ok(c) => c,
                Err(e) => {
                    println!("Failed to connect to JobService: {}", e);
                    return;
                }
            };

            let req = StartJobRequest {
                device_id: dev_id.clone(),
                challenge_response: response,
                requested_method: "".to_string(),
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
    
    // 3. Handle Challenge Request
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
