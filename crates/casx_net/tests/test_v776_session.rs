// crates/casx_net/tests/test_v776_session.rs
// Kiem thu tich hop luong phien Minecraft 26.2 (Protocol 776 / Configuration Phase)

use bytes::BytesMut;
use casx_net::session::v776::{SessionEventV776, SessionStateV776, SessionV776};
use casx_net::codec::frame::RawPacket;
use casx_net::codec::varint::write_varint_to_buf;

#[test]
fn test_v776_full_login_configuration_and_play_transition() {
    let mut session = SessionV776::new("CasxNextGen");
    session.create_handshake_and_login("127.0.0.1", 25565);

    assert_eq!(session.state, SessionStateV776::Login);
    session.outgoing_buffer.clear();

    // 1. Server gui Set Compression (0x03) threshold = 256
    let mut comp_payload = BytesMut::new();
    write_varint_to_buf(256, &mut comp_payload);
    let comp_packet = RawPacket::new(0x03, comp_payload.freeze());
    let mut server_bytes = BytesMut::new();
    comp_packet.encode(&mut server_bytes);

    let _ = session.process_incoming_bytes(&server_bytes).unwrap();
    assert!(session.compressor.is_some());

    // 2. Server gui Login Success (0x02) qua bo nen
    let mut login_succ_payload = BytesMut::new();
    let uuid_bytes = [1u8; 16];
    login_succ_payload.extend_from_slice(&uuid_bytes);
    let username = "CasxNextGen";
    write_varint_to_buf(username.len() as i32, &mut login_succ_payload);
    login_succ_payload.extend_from_slice(username.as_bytes());

    let login_succ_packet = RawPacket::new(0x02, login_succ_payload.freeze());
    let mut server_bytes2 = BytesMut::new();
    session.compressor.as_ref().unwrap().compress_packet(&login_succ_packet, &mut server_bytes2).unwrap();

    let events = session.process_incoming_bytes(&server_bytes2).unwrap();
    assert_eq!(events.len(), 1);
    match &events[0] {
        SessionEventV776::LoginSuccess { username: un, .. } => assert_eq!(un, username),
        _ => panic!("Expected LoginSuccess"),
    }

    // Xac nhan da tu dong chuyen sang Configuration
    assert_eq!(session.state, SessionStateV776::Configuration);

    // Kiem tra buffer co chua Login Acknowledged (0x03) va Client Information (0x00)
    assert!(!session.outgoing_buffer.is_empty());
    session.outgoing_buffer.clear();

    // 3. Server gui Registry Data (0x07) trong Configuration
    let reg_packet = RawPacket::new(0x07, bytes::Bytes::new());
    let mut server_bytes3 = BytesMut::new();
    session.compressor.as_ref().unwrap().compress_packet(&reg_packet, &mut server_bytes3).unwrap();

    let events3 = session.process_incoming_bytes(&server_bytes3).unwrap();
    assert_eq!(events3.len(), 1);
    match events3[0] {
        SessionEventV776::DimensionRegistryReceived { min_y, height } => {
            assert_eq!(min_y, -64);
            assert_eq!(height, 384);
        }
        _ => panic!("Expected DimensionRegistryReceived"),
    }

    // 4. Server gui Finish Configuration (0x03)
    let finish_packet = RawPacket::new(0x03, bytes::Bytes::new());
    let mut server_bytes4 = BytesMut::new();
    session.compressor.as_ref().unwrap().compress_packet(&finish_packet, &mut server_bytes4).unwrap();

    let events4 = session.process_incoming_bytes(&server_bytes4).unwrap();
    assert_eq!(events4.len(), 1);
    match events4[0] {
        SessionEventV776::ConfigurationFinished => {}
        _ => panic!("Expected ConfigurationFinished"),
    }

    // Xac nhan da chuyen thanh cong sang Play
    assert_eq!(session.state, SessionStateV776::Play);

    // Kiem tra Client da gui Acknowledge Finish Configuration (0x03)
    assert!(!session.outgoing_buffer.is_empty());
}
