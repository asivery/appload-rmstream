pub struct Device {
    pub width: u32,
    pub height: u32,
    pub fb_size: usize,
    pub image_data_translator: fn(&Vec<u8>) -> Vec<u8>,
    pub digitizer_path: &'static str,
    pub digitizer_data_translator: fn(i32, i32, i32) -> (i32, i32, i32),
}

// Todo: Add RM2
pub enum ReMarkableDevice {
    PaperPro,
}

fn rmpp_image_data_translator(data: &Vec<u8>) -> Vec<u8> {
    data[..1624 * 2154 * 4]
        .chunks(4)
        .flat_map(|e| [e[2], e[1], e[0], e[3]])
        .collect::<Vec<_>>()
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
