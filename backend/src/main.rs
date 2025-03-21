mod devices;

use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Cursor};
use std::os::fd::AsRawFd;
use std::time::Duration;

use anyhow::{bail, Result};
use appload_client::{AppLoad, AppLoadBackend, BackendReplier, Message, MSG_SYSTEM_NEW_COORDINATOR};
use async_trait::async_trait;
use devices::{detect_device, get_device_info};
use evdev::{AbsoluteAxisCode, Device, EventSummary, KeyCode, SynchronizationCode};
use lazy_static::lazy_static;
use routerify::{Router, RouterService};
use tokio::sync::Mutex;
use tokio::time::sleep;

use std::convert::Infallible;
use std::net::SocketAddr;

use hyper::body::Body;
use hyper::{Request, Response, Server};

const SCREEN_POLL_RATE: u64 = 50;
const PORT: u16 = 3000;

lazy_static! {
    static ref IMAGE_DATA: Mutex<Vec<u8>> = Mutex::new(Vec::default());
    static ref POINTER_POSITION: Mutex<(i32, i32, i32)> = Mutex::new((0, 0, 0));
}

async fn update_pointer_pos_forever() -> Result<()>{
    let device_type = detect_device().unwrap();
    let device_info = get_device_info(device_type);
    let mut evdev_device = Device::open(device_info.digitizer_path).unwrap().into_event_stream().unwrap();
    let mut x: i32 = 0;
    let mut y: i32 = 0;
    let mut d: i32 = 0;
    loop {
        let event = evdev_device.next_event().await?;
        match event.destructure() {
            EventSummary::Synchronization(_, SynchronizationCode::SYN_REPORT, _) => {
                // Flush to the global structures
                *POINTER_POSITION.lock().await = (device_info.digitizer_data_translator)(x, y, d);
            }
            EventSummary::AbsoluteAxis(_, AbsoluteAxisCode::ABS_X, value) => {
                x = value;
            }
            EventSummary::AbsoluteAxis(_, AbsoluteAxisCode::ABS_Y, value) => {
                y = value;
            }
            EventSummary::Key(_, KeyCode::BTN_TOOL_PEN, value) => {
                if value == 0 {
                    d = 0;
                }
            }
            EventSummary::AbsoluteAxis(_, AbsoluteAxisCode::ABS_DISTANCE, value) => {
                d = value;
            }
            _ => {}
        }
    }
}

async fn update_png_image_forever(mem_fd: File, position: usize) -> Result<()> {
    let device_type = detect_device().unwrap();
    let device_info = get_device_info(device_type);
    let mut data = vec![0u8; device_info.fb_size];
    loop {
        sleep(Duration::from_millis(SCREEN_POLL_RATE)).await;
        let mut global_ref = IMAGE_DATA.lock().await;
        let mut c = Cursor::new(&mut *global_ref);
        let mut w = BufWriter::new(&mut c);

        let mut encoder = png::Encoder::new(&mut w, device_info.width, device_info.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        if unsafe { libc::lseek(mem_fd.as_raw_fd(), position as libc::off_t, libc::SEEK_SET) } == -1
        {
            bail!("Failed to read memory!");
        }
        let read_bytes = unsafe {
            libc::read(
                mem_fd.as_raw_fd(),
                data.as_mut_ptr() as *mut libc::c_void,
                device_info.fb_size,
            )
        };
        if read_bytes != device_info.fb_size as isize {
            bail!("Failed to read memory!");
        }
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&(device_info.image_data_translator)(&data))
            .unwrap();
    }
}

async fn home_handler(_: Request<Body>) -> Result<Response<Body>, Infallible> {
    Ok(Response::builder()
        .header("Content-Type", "text/html")
        .body(Body::from(include_str!("page.html")))
        .unwrap())
}

async fn image_handler(_: Request<Body>) -> Result<Response<Body>, Infallible> {
    Ok(Response::builder()
        .header("Content-Type", "image/png")
        .body(Body::from(IMAGE_DATA.lock().await.clone()))
        .unwrap())
}

async fn pointer_handler(_: Request<Body>) -> Result<Response<Body>, Infallible> {
    let data = POINTER_POSITION.lock().await.clone();
    Ok(Response::builder()
        .header("Content-Type", "text/plain")
        .body(Body::from(format!("{} {} {}", data.0, data.1, data.2)))
        .unwrap())
}

fn router() -> Router<Body, Infallible> {
    Router::builder()
        .get("/", home_handler)
        .get("/image", image_handler)
        .get("/pointer", pointer_handler)
        .build()
        .unwrap()
}

async fn run_server() -> Result<()> {
    let router = router();
    let service = RouterService::new(router).unwrap();
    let addr = SocketAddr::from(([0, 0, 0, 0], PORT));
    let server = Server::bind(&addr).serve(service);

    println!("App is running on: {}", addr);
    if let Err(err) = server.await {
        eprintln!("Server error: {}", err);
    }

    Ok(())
}

async fn real_main(pid: u32, sender: BackendReplier<MyBackend>) -> Result<()>{
    println!("Initializing rmStream...");
    if let None = detect_device() {
        sender.send_message(2, "The device you're using is not compatible!".into()).unwrap();
        println!("Device is not compatible!");
        return Ok(());
    }
    eprintln!("Opening xochitl's memory");
    let mem_fd = OpenOptions::new()
        .read(true)
        .open(format!("/proc/{}/mem", pid))?;

    if let Ok(framebuffer_address) = std::env::var("FRAMEBUFFER_SPY_EXTENSION_FBADDR") {
        eprintln!("Framebuffer is at {} according to framebuffer-spy", &framebuffer_address);

        tokio::spawn(update_png_image_forever(mem_fd, usize::from_str_radix(&framebuffer_address[2..], 16).unwrap()));
        tokio::spawn(update_pointer_pos_forever());

        sender.backend.lock().await.ready = true;
        sender.send_message(1, "ready").unwrap();
        run_server().await?;
        Ok(())
    } else {
        sender.send_message(2, "No framebuffer-spy installed".into()).unwrap();
        Ok(())
    }
}


struct MyBackend {
    pub ready: bool,
    ip_addrs: Vec<String>,
    init: bool,
    pid: u32,
}

#[async_trait]
impl AppLoadBackend for MyBackend {
    async fn handle_message(&mut self, functionality: &BackendReplier<MyBackend>, message: Message) {
        match message.msg_type {
            MSG_SYSTEM_NEW_COORDINATOR => {
                if !self.init {
                    self.init = true;
                    tokio::spawn(real_main(self.pid, functionality.clone()));
                }
                functionality.send_message(0, &format!("{},{}", self.ready, self.ip_addrs.join(","))).unwrap();
            }
            m => {
                eprintln!("Unhandled message type: {}", m);
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let mut system = sysinfo::System::new();
    system.refresh_all();
    let pid = system
        .processes_by_name("xochitl".as_ref())
        .nth(0)
        .unwrap()
        .pid()
        .as_u32();
    println!("Xochitl's PID is {}", pid);
    let ip_addrs = sysinfo::Networks::new_with_refreshed_list()
        .iter()
        .flat_map(|e| e.1.ip_networks().iter().map(|e| e.addr))
        .filter(|e| e.is_ipv4() && !e.is_loopback())
        .map(|e| format!("http://{}:{}", e.to_string(), PORT))
        .collect::<Vec<_>>();

    let backend = MyBackend {
        ip_addrs,
        pid,
        ready: false,
        init: false,
    };

    let mut appload = AppLoad::new(backend).unwrap();
    appload.run().await.unwrap();
}
