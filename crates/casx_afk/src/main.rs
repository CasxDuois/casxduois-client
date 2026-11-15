// crates/casx_afk/src/main.rs
//! Headless Eco-AFK client runner for Minecraft 1.12.2 (Protocol 340).

use clap::Parser;
use log::{debug, error, info, warn};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::interval;

use casx_core::jmath::DVec3;
use casx_net::session::v340::{SessionEventV340, SessionV340};
use casx_platform::PlatformTelemetry;
use casx_sim::PlayerSimulation;
use casx_world::WorldStorage;

#[derive(Parser, Debug)]
#[command(name = "casx-afk")]
#[command(about = "CasxDuois headless 20 TPS client runner for Minecraft 1.12.2 (Protocol 340)")]
struct CliArgs {
    /// Minecraft server address (host:port)
    #[arg(short, long, default_value = "127.0.0.1:25565")]
    server: String,

    /// Offline player username
    #[arg(short, long, default_value = "CasxBot")]
    nick: String,

    /// Maximum number of ticks to simulate (0 runs indefinitely)
    #[arg(long, default_value_t = 0)]
    max_ticks: u64,

    /// Run local offline physics test with a solid ground plane
    #[arg(long)]
    offline_test: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args = CliArgs::parse();
    info!("Starting headless client session (protocol: v340 / 1.12.2)");

    let mut telemetry = PlatformTelemetry::default();
    let rss_kb = telemetry.process_memory_kb();
    debug!(
        "Simulation tick interval: 50ms (20 TPS) | process RSS: {} KiB",
        rss_kb
    );
    info!("Connecting to {} as '{}'", args.server, args.nick);

    let parts: Vec<&str> = args.server.split(':').collect();
    let host = parts[0];
    let port: u16 = if parts.len() > 1 {
        parts[1].parse().unwrap_or(25565)
    } else {
        25565
    };

    let mut session = SessionV340::new(&args.nick);
    let mut world = WorldStorage::new();
    let mut simulation = PlayerSimulation::new(DVec3::new(0.0, 70.0, 0.0));

    let stream_result = TcpStream::connect((host, port)).await;

    let mut stream = match stream_result {
        Ok(s) => {
            info!("Connected to {} successfully", args.server);
            let _ = s.set_nodelay(true);
            Some(s)
        }
        Err(e) => {
            if args.offline_test {
                warn!(
                    "Connection to {} failed: {}. Running local offline physics test.",
                    args.server, e
                );
                world.generate_flat_test_world();
                for tick in 1..=20 {
                    simulation.tick(&world);
                    debug!(
                        "tick={} pos=({:.2}, {:.2}, {:.2}) on_ground={}",
                        tick,
                        simulation.pos.x,
                        simulation.pos.y,
                        simulation.pos.z,
                        simulation.on_ground
                    );
                }
                info!(
                    "Offline physics test completed: landed at Y = {:.2}, on_ground = {}",
                    simulation.pos.y, simulation.on_ground
                );
                return Ok(());
            } else {
                error!("Connection to {} failed: {}", args.server, e);
                std::process::exit(1);
            }
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

    info!("Starting 20 TPS simulation loop");

    loop {
        tick_timer.tick().await;
        ticks_elapsed += 1;

        if let Some(ref mut s) = stream {
            tokio::select! {
                read_res = s.read(&mut read_buf) => {
                    match read_res {
                        Ok(n) if n > 0 => {
                            let events = session.process_incoming_bytes(&read_buf[..n])?;
                            for event in events {
                                match event {
                                    SessionEventV340::LoginSuccess { uuid, username } => {
                                        info!("Logged in successfully. UUID: {}, User: {}", uuid, username);
                                    }
                                    SessionEventV340::JoinGame { entity_id, gamemode, dimension } => {
                                        info!("Joined game. Entity ID: {}, Gamemode: {}, Dimension: {}", entity_id, gamemode, dimension);
                                    }
                                    SessionEventV340::PlayerPosLook { x, y, z, yaw, pitch, teleport_id } => {
                                        info!("Spawn position: ({:.2}, {:.2}, {:.2}), Teleport ID: {}", x, y, z, teleport_id);
                                        simulation.set_position(DVec3::new(x, y, z), yaw, pitch);
                                    }
                                    SessionEventV340::ChunkData { chunk_x, chunk_z, ground_up_continuous, primary_bit_mask, data } => {
                                        let _ = world.apply_chunk_packet_v340(chunk_x, chunk_z, ground_up_continuous, primary_bit_mask, &data);
                                    }
                                    SessionEventV340::KeepAliveReceived { id } => {
                                        info!("Keep-Alive #{} acknowledged", id);
                                    }
                                    SessionEventV340::Disconnect { reason } => {
                                        error!("Disconnected from server: {}", reason);
                                        return Ok(());
                                    }
                                    _ => {}
                                }
                            }
                        }
                        Ok(_) => {
                            warn!("Connection closed by server");
                            break;
                        }
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                        Err(e) => {
                            error!("Socket read error: {}", e);
                            break;
                        }
                    }
                }
                _ = tokio::time::sleep(Duration::from_millis(1)) => {}
            }

            simulation.tick(&world);
            let _ = session.send_player_position(
                simulation.pos.x,
                simulation.pos.y,
                simulation.pos.z,
                simulation.on_ground,
            );

            if !session.outgoing_buffer.is_empty() {
                s.write_all(&session.outgoing_buffer).await?;
                session.outgoing_buffer.clear();
            }
        }

        if args.max_ticks > 0 && ticks_elapsed >= args.max_ticks {
            info!(
                "Reached max tick limit ({}). Exiting cleanly.",
                args.max_ticks
            );
            break;
        }
    }

    info!("Headless client session finished successfully");
    Ok(())
}
