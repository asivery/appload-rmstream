mod devices;

use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Cursor, Write};
use std::os::fd::AsRawFd;
use std::time::Duration;

use anyhow::{bail, Result};
use appload_client::{AppLoad, AppLoadBackend, BackendReplier, Message, MSG_SYSTEM_NEW_COORDINATOR};
use async_trait::async_trait;
use devices::{detect_device, get_device_info};
use evdev::{AbsoluteAxisCode, Device, EventSummary, KeyCode, SynchronizationCode};
use flate2::write::DeflateEncoder;
use flate2::Compression;
use lazy_static::lazy_static;
use tokio::sync::{broadcast, Mutex};
use tokio::time::sleep;
use warp::Filter;
use futures::{SinkExt, StreamExt};

const SCREEN_POLL_RATE: u64 = 20;
const PORT: u16 = 3000;

struct ImageDelta {
    offset: u32,
    data: Vec<u8>,
}

impl ImageDelta {
    fn serialize(&self) -> Vec<u8> {
        let mut outbound = Vec::with_capacity(self.data.len() + 4 * 2);
        outbound.extend_from_slice(&self.offset.to_be_bytes());
        outbound.extend_from_slice(&(self.data.len() as u32).to_be_bytes());
        outbound.extend_from_slice(&self.data);

        outbound
    }
}

lazy_static! {
    static ref IMAGE_DATA: Mutex<Vec<u8>> = Mutex::new(Vec::default());
    static ref CHANGES_BROADCASTER: Mutex<broadcast::Sender<Vec<u8>>> = Mutex::new(broadcast::channel(100).0);
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
                let values = (device_info.digitizer_data_translator)(x, y, d);
                let mut packet = vec![2u8];
                packet.extend_from_slice(&values.0.to_be_bytes());
                packet.extend_from_slice(&values.1.to_be_bytes());
                packet.extend_from_slice(&values.2.to_be_bytes());
                let _ = CHANGES_BROADCASTER.lock().await.send(packet);
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

async fn get_current_screen_as_png() -> Result<Vec<u8>> {
    let device_type = detect_device().unwrap();
    let device_info = get_device_info(device_type);
    let mut out = vec![0u8; device_info.fb_size]; // Worst-case scenario
    let mut c = Cursor::new(&mut *out);
    let mut w = BufWriter::new(&mut c);

    let mut encoder = png::Encoder::new(&mut w, device_info.width, device_info.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&IMAGE_DATA.lock().await)?;
    drop(w);
    let size = c.position() as usize;
    drop(c);
    Ok(out[0..size].to_vec())
}

async fn broadcast_changes_forever(mem_fd: File, position: usize) -> Result<()> {
    let device_type = detect_device().unwrap();
    let device_info = get_device_info(device_type);
    let mut data = vec![0u8; device_info.fb_size];
    *IMAGE_DATA.lock().await = vec![0u8; device_info.fb_size];
    loop {
        sleep(Duration::from_millis(SCREEN_POLL_RATE)).await;
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
        (device_info.image_data_translator)(&mut data);

        // Encode deltas
        let mut global_ref = IMAGE_DATA.lock().await;
        let mut deltas = Vec::new();

        let mut current_delta = None;
        for (i, (old, new)) in global_ref.iter().zip(&data).enumerate() {
            match (*old == *new, current_delta.is_none()) {
                (true, true) => {},
                (false, true) => {
                    // There is a difference, and we're not in a delta. => Create a new delta
                    current_delta = Some(ImageDelta {
                        offset: i as u32,
                        data: vec![*new],
                    });
                },
                (true, false) => {
                    // There's no difference, and we're in a delta => Finish delta.
                    deltas.extend_from_slice(&current_delta.unwrap().serialize());
                    current_delta = None;
                },
                (false, false) => {
                    // No changes, and delta exists => Append to delta
                    current_delta.as_mut().unwrap().data.push(*new);
                }
            }
        }
        // If there's a leftover delta, push it
        if let Some(delta) = current_delta {
            deltas.extend_from_slice(&delta.serialize());
        }
        // Update the global reference.
        global_ref.copy_from_slice(&data);
        // Compress and broadcast deltas
        if deltas.len() > 0 {
            let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(&deltas).unwrap();
            let final_size = deltas.len() as u32;
            let mut deltas = encoder.finish().unwrap();
            deltas.insert(0, 1);
            deltas.splice(1..1, final_size.to_be_bytes());
            let _ = CHANGES_BROADCASTER.lock().await.send(deltas);
        }
    }
}

fn run_server() {
    let page = warp::path::end().map(|| warp::reply::html(include_str!("page.html")));
    let ws_page = warp::path("ws").and(warp::ws()).map(|ws: warp::ws::Ws| {
        ws.on_upgrade(websocket_handler)
    });
    let routes = page.or(ws_page).with(warp::cors().allow_any_origin());

    tokio::task::spawn(warp::serve(routes)
        .run(([0, 0, 0, 0], PORT)));
}

async fn websocket_handler(websocket: warp::ws::WebSocket) {
    let device_type = detect_device().unwrap();
    let device_info = get_device_info(device_type);

    let (mut sender, mut _receiver) = websocket.split();
    // Encode initial PNG data.
    if let Err(e) = {
        let png_data = get_current_screen_as_png().await.unwrap();
        let mut initial_data = Vec::new();
        initial_data.extend_from_slice(&device_info.width.to_be_bytes());
        initial_data.extend_from_slice(&device_info.height.to_be_bytes());
        initial_data.extend_from_slice(&png_data);
        match sender.send(warp::ws::Message::binary(initial_data)).await {
            Ok(_) => {
                sender.flush().await
            }
            e => e
        }
    } {
        println!("Error while flushing the initial data. Disconnecting the client: {:?}", e);
        return;
    }
    println!("Initial packet sent!");
    // Now start receiving deltas
    let mut subscriber = CHANGES_BROADCASTER.lock().await.subscribe();
    while let Ok(delta_packet) = subscriber.recv().await {
        if let Err(e) = {
            match sender.send(warp::ws::Message::binary(delta_packet)).await {
                Ok(_) => {
                    sender.flush().await
                }
                e => e
            }
        } {
            println!("Error while sending delta packet. Disconnecting the client: {:?}", e);
            return;
        }
    }

    println!("Client disconnected");
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

        tokio::spawn(broadcast_changes_forever(mem_fd, usize::from_str_radix(&framebuffer_address[2..], 16).unwrap()));
        tokio::spawn(update_pointer_pos_forever());

        sender.backend.lock().await.ready = true;
        sender.send_message(1, "ready").unwrap();
        run_server();
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
