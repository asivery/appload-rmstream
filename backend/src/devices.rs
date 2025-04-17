pub struct Device {
    pub width: u32,
    pub height: u32,
    pub fb_size: usize,
    pub image_data_translator: fn(&mut Vec<u8>),
    pub digitizer_path: &'static str,
    pub digitizer_data_translator: fn(i32, i32, i32) -> (i32, i32, i32),
}

// Todo: Add RM2
pub enum ReMarkableDevice {
    PaperPro,
}

fn rmpp_image_data_translator(data: &mut Vec<u8>) {
    for i in 0..(1624 * 2154) {
        let a = data[4 * i + 2];
        let b = data[4 * i + 1];
        let c = data[4 * i + 0];
        let d = data[4 * i + 3];
        data[4 * i + 0] = a;
        data[4 * i + 1] = b;
        data[4 * i + 2] = c;
        data[4 * i + 3] = d;
    }
}

fn rmpp_digitizer_translator(x: i32, y: i32, d: i32) -> (i32, i32, i32) {
    (
        ((f64::from(x) / 11180.0) * 100.0) as i32,
        ((f64::from(y) / 15340.0) * 100.0) as i32,
        i32::min(d, 1),
    )
}

pub fn get_device_info(r#type: ReMarkableDevice) -> Device {
    match r#type {
        ReMarkableDevice::PaperPro => Device {
            fb_size: 1624 * 2154 * 4,
            height: 2154,
            width: 1624,
            image_data_translator: rmpp_image_data_translator,
            digitizer_path: "/dev/input/event2",
            digitizer_data_translator: rmpp_digitizer_translator,
        },
    }
}

pub fn detect_device() -> Option<ReMarkableDevice> {
    let device_type_file = std::fs::read_to_string("/sys/devices/soc0/machine")
        .unwrap()
        .to_lowercase();
    if device_type_file.contains("ferrari") {
        Some(ReMarkableDevice::PaperPro)
    } else {
        None
    }
}
