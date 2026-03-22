use crate::framebuffer_spy::FramebufferSpyConfig;

pub struct Device {
    pub digitizer_path: &'static str,
    pub digitizer_data_translator: fn(&Device, i32, i32, i32) -> (i32, i32, i32),
    pub max_digitizer_width: f64,
    pub max_digitizer_height: f64,
    pub override_framebuffer_config: Option<&'static FramebufferConfig>,
}

pub struct FramebufferConfig {
    pub framebuffer_file: Option<&'static str>,
    pub address: usize,
    pub width: u32,
    pub height: u32,
    pub fb_size: usize,
    pub image_data_translator: fn(&FramebufferConfig, &[u8], &mut [u8]),
}

impl From<FramebufferSpyConfig> for FramebufferConfig {
    fn from(value: FramebufferSpyConfig) -> Self {
        let (pixel_size, image_data_translator): (u32, fn(&FramebufferConfig, &[u8], &mut [u8])) = match value.r#type {
            2 => (4, rgba_image_data_translator),
            1 => (2, rgb565_image_data_translator),
            _ => panic!()
        };
        let fb_size = (value.bpl * value.height) as usize;
        Self {
            framebuffer_file: None,
            address: value.address,
            fb_size,
            height: value.height,
            image_data_translator,
            width: value.bpl / pixel_size
        }
    }
}

pub const RM1_FRAMEBUFFER_CONFIG: FramebufferConfig = FramebufferConfig {
    framebuffer_file: Some("/dev/fb0"),
    address: 0,
    fb_size: 1872 * 1408 * 2,
    height: 1872,
    width: 1408,
    image_data_translator: rgb565_image_data_translator,
};

pub enum ReMarkableDevice {
    RM1,
    RM2,
    PaperPro,
    PaperProMove,
}

fn rgba_image_data_translator(config: &FramebufferConfig, in_data: &[u8], out_data: &mut [u8]) {
    for i in 0..(config.width * config.height) as usize {
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
        ((f64::from(y) / device.max_digitizer_height) * 100.0) as i32,
        (((device.max_digitizer_width - f64::from(x)) / device.max_digitizer_width) * 100.0) as i32,
        i32::min(d, 1),
    )
}

fn rm1_digitizer_translator(device: &Device, x: i32, y: i32, _d: i32) -> (i32, i32, i32) {
    (
        ((f64::from(y) / device.max_digitizer_width) * 100.0) as i32,
        (((device.max_digitizer_height - f64::from(x)) / device.max_digitizer_height) * 100.0)
            as i32,
        1,
    )
}

fn rgb565_image_data_translator(config: &FramebufferConfig, in_data: &[u8], out_data: &mut [u8]) {
    for i in 0..(config.width * config.height) as usize {
        let a = in_data[2 * i + 1] as u16;
        let b = in_data[2 * i] as u16;
        let total = (a << 8) | b;
        let r5 = (total >> 11) & 0x1F;
        let g6 = (total >> 5) & 0x3F;
        let b5 = total & 0x1F;
        let r8 = ((r5 * 255) / 31) as u8;
        let g8 = ((g6 * 255) / 63) as u8;
        let b8 = ((b5 * 255) / 31) as u8;
        out_data[4 * i] = r8;
        out_data[4 * i + 1] = g8;
        out_data[4 * i + 2] = b8;
        out_data[4 * i + 3] = 0xFF;
    }
}

pub fn get_device_info(r#type: ReMarkableDevice) -> Device {
    match r#type {
        ReMarkableDevice::PaperPro => Device {
            digitizer_path: "/dev/input/event2",
            digitizer_data_translator: rmpp_digitizer_translator,
            max_digitizer_width: 11180.0,
            max_digitizer_height: 15340.0,
            override_framebuffer_config: None,
        },
        ReMarkableDevice::PaperProMove => Device {
            digitizer_path: "/dev/input/event2",
            digitizer_data_translator: rmpp_digitizer_translator,
            max_digitizer_width: 6760.0,
            max_digitizer_height: 11960.0,
            override_framebuffer_config: None,
        },
        ReMarkableDevice::RM2 => Device {
            digitizer_path: "/dev/input/event1",

            digitizer_data_translator: rm2_digitizer_translator,
            max_digitizer_width: 20967.0,
            max_digitizer_height: 15725.0,
            override_framebuffer_config: None,
        },
        ReMarkableDevice::RM1 => Device {
            digitizer_path: "/dev/input/event0",

            digitizer_data_translator: rm1_digitizer_translator,
            max_digitizer_width: 15725.0,
            max_digitizer_height: 20967.0,
            override_framebuffer_config: Some(&RM1_FRAMEBUFFER_CONFIG),
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
        Some(ReMarkableDevice::RM1)
    }
}
