// crates/casx_net/tests/test_v340_session.rs
// Integration test for Minecraft 1.12.2 (Protocol 340) session.

use bytes::BytesMut;
use casx_net::codec::frame::RawPacket;
use casx_net::codec::varint::write_varint_to_buf;
use casx_net::session::v340::{SessionEventV340, SessionV340};

#[test]
fn test_v340_handshake_and_login_flow() {
    let mut session = SessionV340::new("CasxBot");
    session.create_handshake_and_login("127.0.0.1", 25565);

    // Verify outbound buffer contains Handshake and Login Start
    assert!(!session.outgoing_buffer.is_empty());
    session.outgoing_buffer.clear();

    // 1. Simulate server Set Compression (0x03) threshold = 256
    let mut comp_payload = BytesMut::new();
    write_varint_to_buf(256, &mut comp_payload);
    let comp_packet = RawPacket::new(0x03, comp_payload.freeze());
    let mut server_bytes = BytesMut::new();
    comp_packet.encode(&mut server_bytes);

    let events = session.process_incoming_bytes(&server_bytes).unwrap();
    assert!(events.is_empty());
    assert!(session.compressor.is_some());

    // 2. Simulate server Login Success (0x02) compressed
    let mut login_succ_payload = BytesMut::new();
    let uuid = "069a79f4-44e9-4726-a5be-fca90e38aaf5";
    let username = "CasxBot";
    write_varint_to_buf(uuid.len() as i32, &mut login_succ_payload);
    login_succ_payload.extend_from_slice(uuid.as_bytes());
    write_varint_to_buf(username.len() as i32, &mut login_succ_payload);
    login_succ_payload.extend_from_slice(username.as_bytes());

    let login_succ_packet = RawPacket::new(0x02, login_succ_payload.freeze());
    let mut server_bytes2 = BytesMut::new();
    session
        .compressor
        .as_ref()
        .unwrap()
        .compress_packet(&login_succ_packet, &mut server_bytes2)
        .unwrap();

    let events2 = session.process_incoming_bytes(&server_bytes2).unwrap();
    assert_eq!(events2.len(), 1);
    match &events2[0] {
        SessionEventV340::LoginSuccess {
            uuid: u,
            username: un,
        } => {
            assert_eq!(u, uuid);
            assert_eq!(un, username);
        }
        _ => panic!("Expected LoginSuccess"),
    }
}

#[test]
fn test_v340_keep_alive_auto_reply() {
    let mut session = SessionV340::new("CasxBot");
    session.state = casx_net::session::v340::SessionState::Play;

    // Simulate server sending Keep Alive (S->C 0x1F) with ID = 1234567890123456L
    let keep_alive_id: i64 = 1234567890123456;
    let packet = RawPacket::new(
        0x1F,
        bytes::Bytes::copy_from_slice(&keep_alive_id.to_be_bytes()),
    );
    let mut server_bytes = BytesMut::new();
    packet.encode(&mut server_bytes);

    let events = session.process_incoming_bytes(&server_bytes).unwrap();
    assert_eq!(events.len(), 1);
    match events[0] {
        SessionEventV340::KeepAliveReceived { id } => assert_eq!(id, keep_alive_id),
        _ => panic!("Expected KeepAliveReceived"),
    }

    // Verify session automatically generated Keep Alive reply (C->S 0x0B)
    assert!(!session.outgoing_buffer.is_empty());
    let reply = RawPacket::decode(&mut session.outgoing_buffer)
        .unwrap()
        .unwrap();
    assert_eq!(reply.id, 0x0B);
    assert_eq!(&reply.payload[..], &keep_alive_id.to_be_bytes()[..]);
}
