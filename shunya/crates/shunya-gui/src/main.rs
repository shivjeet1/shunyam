slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let ui = MainWindow::new()?;

    // In a real application, we would fetch these from the shunyad daemon over gRPC.
    // For MVP, we use the mocked properties injected in the .slint file, but we can also
    // dynamically push to them like this:
    /*
    let devices_model = std::rc::Rc::new(slint::VecModel::from(vec![
        Device {
            id: "/dev/nvme0n1".into(),
            model: "Samsung 980 PRO".into(),
            capacity: "1.0 TB".into(),
            status: "Ready".into(),
        }
    ]));
    ui.set_devices(devices_model.into());
    */

    let ui_handle = ui.as_weak();
    
    ui.on_start_wipe(move || {
        let ui = ui_handle.unwrap();
        // Just mock updating the status of the selected device to wiping
        println!("Start wipe clicked! (Mock action)");
        
        // This is where we would call shunyad StartJob RPC
    });

    ui.run()
}
