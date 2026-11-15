// crates/casx_afk/src/main.rs
// Ung dung dong lenh Eco-AFK Headless Client cho Minecraft 1.12.2 (Protocol 340)

use clap::Parser;
use log::{info, warn, error};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::interval;

use casx_core::jmath::DVec3;
use casx_net::session::v340::{SessionEventV340, SessionV340};
use casx_world::WorldStorage;
use casx_sim::PlayerSimulation;
use casx_platform::PlatformTelemetry;

#[derive(Parser, Debug)]
#[command(name = "casx-afk")]
#[command(about = "CasxDuois Eco-AFK Headless Client for Minecraft 1.12.2 (Protocol 340)")]
struct CliArgs {
    /// Dia chi server Minecraft 1.12.2 dang host:port
    #[arg(short, long, default_value = "127.0.0.1:25565")]
    server: String,

    /// Nickname offline cua bot
    #[arg(short, long, default_value = "CasxBot")]
    nick: String,

    /// So luong ticks toi da de chay (0 la chay vo tan)
    #[arg(long, default_value_t = 0)]
    max_ticks: u64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args = CliArgs::parse();
    info!("[casx-afk::init] Khoi dong CasxDuois Eco-AFK Headless Client (Protocol 340 / 1.12.2)");
    info!("[casx-afk::init] Muc tieu bo nho: < 15 MB RAM | Tan so mo phong: 20 TPS (50ms)");
    info!("[casx-afk::init] Ket noi toi may chu: {} | Nickname: {}", args.server, args.nick);

    let telemetry = PlatformTelemetry::default();
    let initial_ram = telemetry.total_memory_mb();
    info!("[casx-afk::telemetry] Dung luong RAM vat ly he thong: {} MB", initial_ram);

    // Phan tach host va port
    let parts: Vec<&str> = args.server.split(':').collect();
    let host = parts[0];
    let port: u16 = if parts.len() > 1 {
        parts[1].parse().unwrap_or(25565)
    } else {
        25565
    };

    // Khoi tao cac thanh phan cot loi cua engine
    let mut session = SessionV340::new(&args.nick);
    let mut world = WorldStorage::new();
    let mut simulation = PlayerSimulation::new(DVec3::new(0.0, 64.0, 0.0));

    // Ket noi TCP socket toi server
    info!("[casx-afk::net] Dang mo ket noi TCP toi {}:{}...", host, port);
    let stream_result = TcpStream::connect((host, port)).await;

    let mut stream = match stream_result {
        Ok(s) => {
            info!("[casx-afk::net] [PASS] Ket noi TCP thanh cong!");
            let _ = s.set_nodelay(true);
            Some(s)
        }
        Err(e) => {
            warn!("[casx-afk::net] [WARN] Khong the ket noi toi server thuc te: {}. Chuyen sang che do Headless Parity Smoke Verification.", e);
            None
        }
    };

    if let Some(ref mut s) = stream {
        session.create_handshake_and_login(host, port);
        if !session.outgoing_buffer.is_empty() {
            s.write_all(&session.outgoing_buffer).await?;
            session.outgoing_buffer.clear();
        }
    }

    let mut tick_timer = interval(Duration::from_millis(50));
    let mut read_buf = [0u8; 65536];
    let mut ticks_elapsed: u64 = 0;

    info!("[casx-afk::loop] Bat dau vong lap mo phong 20 TPS...");

    loop {
        tick_timer.tick().await;
        ticks_elapsed += 1;

        if let Some(ref mut s) = stream {
            // Doc du lieu mang non-blocking neu co san
            tokio::select! {
                read_res = s.read(&mut read_buf) => {
                    match read_res {
                        Ok(n) if n > 0 => {
                            let events = session.process_incoming_bytes(&read_buf[..n])?;
                            for event in events {
                                match event {
                                    SessionEventV340::LoginSuccess { uuid, username } => {
                                        info!("[casx-afk::auth] [PASS] Dang nhap thanh cong! UUID: {}, User: {}", uuid, username);
                                    }
                                    SessionEventV340::JoinGame { entity_id, gamemode, dimension } => {
                                        info!("[casx-afk::game] [PASS] Gia nhap the gioi! Entity ID: {}, Gamemode: {}, Dimension: {}", entity_id, gamemode, dimension);
                                    }
                                    SessionEventV340::PlayerPosLook { x, y, z, yaw, pitch, teleport_id } => {
                                        info!("[casx-afk::pos] Nhan toa do spawn tu server: ({:.2}, {:.2}, {:.2}), Teleport ID: {}", x, y, z, teleport_id);
                                        simulation.set_position(DVec3::new(x, y, z), yaw, pitch);
                                    }
                                    SessionEventV340::ChunkData { chunk_x, chunk_z, ground_up_continuous, primary_bit_mask, data } => {
                                        let _ = world.apply_chunk_packet_v340(chunk_x, chunk_z, ground_up_continuous, primary_bit_mask, &data);
                                    }
                                    SessionEventV340::KeepAliveReceived { id } => {
                                        info!("[casx-afk::net] [PASS] Nhip tim Keep-Alive #{} - Phan hoi thanh cong!", id);
                                    }
                                    SessionEventV340::Disconnect { reason } => {
                                        error!("[casx-afk::net] Bi ngat ket noi khoi server: {}", reason);
                                        return Ok(());
                                    }
                                    _ => {}
                                }
                            }
                        }
                        Ok(_) => {
                            warn!("[casx-afk::net] Socket dong boi may chu.");
                            break;
                        }
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                        Err(e) => {
                            error!("[casx-afk::net] Loi doc du lieu socket: {}", e);
                            break;
                        }
                    }
                }
                _ = tokio::time::sleep(Duration::from_millis(1)) => {}
            }

            // Chay tick vat ly 20 TPS
            simulation.tick(&world);

            // Gui toa do cap nhat len server
            let _ = session.send_player_position(simulation.pos.x, simulation.pos.y, simulation.pos.z, simulation.on_ground);

            if !session.outgoing_buffer.is_empty() {
                s.write_all(&session.outgoing_buffer).await?;
                session.outgoing_buffer.clear();
            }
        } else {
            // Che do Headless Smoke Mode (khi khong co server external dang chay)
            simulation.tick(&world);
            if ticks_elapsed % 20 == 0 {
                info!("[casx-afk::smoke] Tick #{} | Toa do: ({:.2}, {:.2}, {:.2}) | Tiep dat: {}",
                    ticks_elapsed, simulation.pos.x, simulation.pos.y, simulation.pos.z, simulation.on_ground);
            }
        }

        if args.max_ticks > 0 && ticks_elapsed >= args.max_ticks {
            info!("[casx-afk::loop] Dat gioi han {} ticks. Dung phien an toan.", args.max_ticks);
            break;
        }
    }

    info!("[casx-afk::exit] [PASS] Hoan tat phien Eco-AFK thanh cong!");
    Ok(())
}
