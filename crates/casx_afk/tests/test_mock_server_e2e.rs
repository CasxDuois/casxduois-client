// crates/casx_afk/tests/test_mock_server_e2e.rs
// Kiem thu tich hop dau cuoi (E2E) giua Mock Server 1.12.2 va client casx-net / casx-sim

use bytes::{BufMut, Bytes, BytesMut};
use casx_core::jmath::DVec3;
use casx_core::palette::SectionPaletteV340;
use casx_net::codec::compression::PacketCompressor;
use casx_net::codec::frame::RawPacket;
use casx_net::codec::varint::write_varint_to_buf;
use casx_net::session::v340::{SessionEventV340, SessionV340};
use casx_sim::PlayerSimulation;
use casx_world::region::{ChunkColumnV340, ChunkSectionV340};
use casx_world::WorldStorage;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[tokio::test]
async fn test_mock_server_1122_full_handshake_landing_and_keepalive() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    // 1. Task gia lap Server 1.12.2
    let server_handle = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut read_buf = BytesMut::with_capacity(4096);
        let mut raw_buf = [0u8; 1024];

        // 1. Nhan Handshake (0x00)
        let n = socket.read(&mut raw_buf).await.unwrap();
        read_buf.extend_from_slice(&raw_buf[..n]);
        let hs_packet = RawPacket::decode(&mut read_buf).unwrap().unwrap();
        assert_eq!(hs_packet.id, 0x00);

        // 2. Nhan Login Start (0x00)
        let login_packet = RawPacket::decode(&mut read_buf).unwrap().unwrap();
        assert_eq!(login_packet.id, 0x00);

        // 3. Server gui Set Compression (0x03) threshold = 128
        let mut comp_payload = BytesMut::new();
        write_varint_to_buf(128, &mut comp_payload);
        let comp_packet = RawPacket::new(0x03, comp_payload.freeze());
        let mut send_buf = BytesMut::new();
        comp_packet.encode(&mut send_buf);
        socket.write_all(&send_buf).await.unwrap();

        let compressor = PacketCompressor::new(128);

        // 4. Server gui Login Success (0x02)
        let mut login_succ_payload = BytesMut::new();
        let uuid = "069a79f4-44e9-4726-a5be-fca90e38aaf5";
        let username = "CasxBot";
        write_varint_to_buf(uuid.len() as i32, &mut login_succ_payload);
        login_succ_payload.extend_from_slice(uuid.as_bytes());
        write_varint_to_buf(username.len() as i32, &mut login_succ_payload);
        login_succ_payload.extend_from_slice(username.as_bytes());

        let login_succ_packet = RawPacket::new(0x02, login_succ_payload.freeze());
        let mut send_buf2 = BytesMut::new();
        compressor.compress_packet(&login_succ_packet, &mut send_buf2).unwrap();
        socket.write_all(&send_buf2).await.unwrap();

        // 5. Server gui Join Game (0x23)
        let mut jg_payload = BytesMut::new();
        jg_payload.extend_from_slice(&42i32.to_be_bytes()); // entity id
        jg_payload.put_u8(0); // gamemode
        jg_payload.extend_from_slice(&0i32.to_be_bytes()); // dimension
        let jg_packet = RawPacket::new(0x23, jg_payload.freeze());
        let mut send_buf3 = BytesMut::new();
        compressor.compress_packet(&jg_packet, &mut send_buf3).unwrap();
        socket.write_all(&send_buf3).await.unwrap();

        // 6. Server gui Player Pos Look (0x2F) spawn tai (0.5, 66.0, 0.5)
        let mut pos_payload = BytesMut::new();
        pos_payload.extend_from_slice(&0.5f64.to_be_bytes());
        pos_payload.extend_from_slice(&66.0f64.to_be_bytes());
        pos_payload.extend_from_slice(&0.5f64.to_be_bytes());
        pos_payload.extend_from_slice(&0.0f32.to_be_bytes());
        pos_payload.extend_from_slice(&0.0f32.to_be_bytes());
        pos_payload.put_u8(0); // flags
        write_varint_to_buf(1, &mut pos_payload); // teleport_id = 1
        let pos_packet = RawPacket::new(0x2F, pos_payload.freeze());
        let mut send_buf4 = BytesMut::new();
        compressor.compress_packet(&pos_packet, &mut send_buf4).unwrap();
        socket.write_all(&send_buf4).await.unwrap();

        // 7. Server gui Keep Alive (0x1F) id = 987654321
        let keep_alive_id: i64 = 987654321;
        let ka_packet = RawPacket::new(0x1F, Bytes::copy_from_slice(&keep_alive_id.to_be_bytes()));
        let mut send_buf5 = BytesMut::new();
        compressor.compress_packet(&ka_packet, &mut send_buf5).unwrap();
        socket.write_all(&send_buf5).await.unwrap();

        // Cho phan hoi tu Client
        tokio::time::sleep(Duration::from_millis(200)).await;
        let n = socket.read(&mut raw_buf).await.unwrap();
        read_buf.extend_from_slice(&raw_buf[..n]);

        // Doc packet Client gui lai qua compressor
        let reply1 = compressor.decompress_packet(&mut read_buf).unwrap().unwrap();
        assert_eq!(reply1.id, 0x00); // Teleport Confirm

        let reply2 = compressor.decompress_packet(&mut read_buf).unwrap().unwrap();
        assert_eq!(reply2.id, 0x0E); // Player Pos Look

        let reply3 = compressor.decompress_packet(&mut read_buf).unwrap().unwrap();
        assert_eq!(reply3.id, 0x0B); // Keep Alive Reply!
        assert_eq!(&reply3.payload[..], &keep_alive_id.to_be_bytes()[..]);
    });

    // 2. Client chay ket noi va mo phong
    let mut client_socket = tokio::net::TcpStream::connect(addr).await.unwrap();
    let mut session = SessionV340::new("CasxBot");
    let mut world = WorldStorage::new();

    // Nap san 1 chunk co san da tai Y=64 vao world
    let mut data = vec![0u64; 256];
    data[0] |= 1u64;
    let palette = vec![0, 16];
    let section = SectionPaletteV340::from_packet_data(4, Some(palette), data).unwrap();
    let mut col = ChunkColumnV340::new(0, 0);
    col.sections.insert(4, ChunkSectionV340::new_v340(
        4,
        section,
        vec![0; 2048],
        None,
    ));
    world.chunks.insert((0, 0), col);

    let mut simulation = PlayerSimulation::new(DVec3::new(0.5, 70.0, 0.5));

    session.create_handshake_and_login("127.0.0.1", addr.port());
    client_socket.write_all(&session.outgoing_buffer).await.unwrap();
    session.outgoing_buffer.clear();

    let mut buf = [0u8; 4096];
    for _ in 0..10 {
        tokio::time::sleep(Duration::from_millis(20)).await;
        if let Ok(Ok(n)) = tokio::time::timeout(Duration::from_millis(50), client_socket.read(&mut buf)).await {
            if n > 0 {
                let events = session.process_incoming_bytes(&buf[..n]).unwrap();
                for event in events {
                    if let SessionEventV340::PlayerPosLook { x, y, z, yaw, pitch, .. } = event {
                        simulation.set_position(DVec3::new(x, y, z), yaw, pitch);
                    }
                }
            }
        }

        // Tinh toan vat ly
        simulation.tick(&world);

        // Gui du lieu phan hoi
        if !session.outgoing_buffer.is_empty() {
            client_socket.write_all(&session.outgoing_buffer).await.unwrap();
            session.outgoing_buffer.clear();
        }
    }

    server_handle.await.unwrap();

    // Kiem tra nguoi choi da dap xuong san da tai Y = 65.0 an toan
    assert!(simulation.on_ground);
    assert!((simulation.pos.y - 65.0).abs() < 1e-4);
}
