pub struct Device {
    pub width: u32,
    pub height: u32,
    pub fb_size: usize,
    pub image_data_translator: fn(&Device, &[u8], &mut [u8]),
    pub digitizer_path: &'static str,
    pub digitizer_data_translator: fn(&Device, i32, i32, i32) -> (i32, i32, i32),
    pub max_digitizer_width: f64,
    pub max_digitizer_height: f64,
}

pub enum ReMarkableDevice {
    RM2,
    PaperPro,
    PaperProMove,
}

fn rgba_image_data_translator(device: &Device, in_data: &[u8], out_data: &mut [u8]) {
    for i in 0..(device.width * device.height) as usize {
        let a = in_data[4 * i + 2];
        let b = in_data[4 * i + 1];
        let c = in_data[4 * i];
        let d = in_data[4 * i + 3];
        out_data[4 * i] = a;
        out_data[4 * i + 1] = b;
        out_data[4 * i + 2] = c;
        out_data[4 * i + 3] = d;
    }
}

fn rmpp_digitizer_translator(device: &Device, x: i32, y: i32, d: i32) -> (i32, i32, i32) {
    (
        ((f64::from(x) / device.max_digitizer_width) * 100.0) as i32,
        ((f64::from(y) / device.max_digitizer_height) * 100.0) as i32,
        i32::min(d, 1),
    )
}

fn rm2_digitizer_translator(device: &Device, x: i32, y: i32, d: i32) -> (i32, i32, i32) {
    (
        (((device.max_digitizer_height - f64::from(y)) / device.max_digitizer_height) * 100.0) as i32,
        ((f64::from(x) / device.max_digitizer_width) * 100.0) as i32,
        i32::min(d, 1),
    )
}

fn rgb565_image_data_translator(device: &Device, in_data: &[u8], out_data: &mut [u8]) {
    for i in 0..(device.width * device.height) as usize {
        let a = in_data[2 * i + 1] as u16;
        let b = in_data[2 * i] as u16;
        let total = (a << 8) | b;
        let r = ((total >> 11) as u8 & 0b11111) << 3;
        let g = ((total >> 5) as u8 & 0b111111) << 2;
        let b = ((total >> 0) as u8 & 0b11111) << 3;
        out_data[4 * i] = r;
        out_data[4 * i + 1] = g;
        out_data[4 * i + 2] = b;
        out_data[4 * i + 3] = 0xFF;
    }
}

pub fn get_device_info(r#type: ReMarkableDevice) -> Device {
    match r#type {
        ReMarkableDevice::PaperPro => Device {
            fb_size: 1632 * 2154 * 4,
            height: 2154,
            width: 1632,
            image_data_translator: rgba_image_data_translator,
            digitizer_path: "/dev/input/event2",
            digitizer_data_translator: rmpp_digitizer_translator,
            max_digitizer_width: 11180.0,
            max_digitizer_height: 15340.0,
        },
        ReMarkableDevice::PaperProMove => Device {
            fb_size: 960 * 1696 * 4,
            height: 1696,
            width: 960,
            image_data_translator: rgba_image_data_translator,
            digitizer_path: "/dev/input/event2",
            digitizer_data_translator: rmpp_digitizer_translator,
            max_digitizer_width: 6760.0,
            max_digitizer_height: 11960.0,
        },
        ReMarkableDevice::RM2 => Device {
            fb_size: 1872 * 1404 * 2,
            height: 1872,
            width: 1404,
            image_data_translator: rgb565_image_data_translator,
            digitizer_path: "/dev/input/event1",

            digitizer_data_translator: rm2_digitizer_translator,
            max_digitizer_width: 20967.0,
            max_digitizer_height: 15725.0,
        },
    }
}

pub fn detect_device() -> Option<ReMarkableDevice> {
    let device_type_file = std::fs::read_to_string("/sys/devices/soc0/machine")
        .unwrap()
        .to_lowercase();
    if device_type_file.contains("chiappa") {
        Some(ReMarkableDevice::PaperProMove)
    } else if device_type_file.contains("ferrari") {
        Some(ReMarkableDevice::PaperPro)
    } else if device_type_file.contains("2.0") {
        Some(ReMarkableDevice::RM2)
    } else {
        None
    }
}
