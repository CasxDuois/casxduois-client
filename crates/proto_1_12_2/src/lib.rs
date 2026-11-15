//! Bo dieu hop giao thuc Minecraft 1.12.2 (Protocol 340).

use bytes::{Buf, BufMut, BytesMut};
use casx_core::codec::{
    decode_block_pos_1_12_2, read_string, read_varint, write_string, write_varint,
};
use casx_core::compression::{format_packet_payload, parse_packet_payload};
use casx_core::world::{CanonicalBlockState, ChunkSection};
use casx_core::{
    ClientAction, ConnectionState, GameVersion, ProtocolAdapter, ProtocolError, ServerEvent,
};

pub struct Proto1_12_2 {
    state: ConnectionState,
    compression_threshold: Option<i32>,
}

impl Default for Proto1_12_2 {
    fn default() -> Self {
        Self::new()
    }
}

impl Proto1_12_2 {
    pub fn new() -> Self {
        Self {
            state: ConnectionState::Handshaking,
            compression_threshold: None,
        }
    }

    /// Giai ma mot mang khoi trong 1 Section cua 1.12.2:
    /// Moi Section gom 4096 khoi: 4096 bytes (Block ID thap) + 2048 bytes (Metadata 4-bit) + 2048 bytes (BlockLight)
    pub fn decode_section_1_12_2(buf: &mut impl Buf, has_skylight: bool) -> Result<ChunkSection, ProtocolError> {
        if buf.remaining() < 4096 + 2048 + 2048 {
            return Err(ProtocolError::UnexpectedEof);
        }

        let mut block_ids = [0u8; 4096];
        buf.copy_to_slice(&mut block_ids);

        let mut meta_bytes = [0u8; 2048];
        buf.copy_to_slice(&mut meta_bytes);

        let mut block_light = [0u8; 2048];
        buf.copy_to_slice(&mut block_light);

        let mut sky_light = [255u8; 2048];
        if has_skylight {
            if buf.remaining() < 2048 {
                return Err(ProtocolError::UnexpectedEof);
            }
            buf.copy_to_slice(&mut sky_light);
        }

        let mut section = ChunkSection {
            blocks: [CanonicalBlockState::AIR; 4096],
            block_light,
            sky_light,
            non_air_count: 0,
        };

        for i in 0..4096 {
            let id = block_ids[i] as u16;
            let meta_byte = meta_bytes[i >> 1];
            let meta = if (i & 1) == 0 {
                meta_byte & 0x0F
            } else {
                (meta_byte >> 4) & 0x0F
            } as u16;

            if id != 0 {
                section.non_air_count += 1;
                section.blocks[i] = CanonicalBlockState::new(id, meta);
            }
        }

        Ok(section)
    }
}

impl ProtocolAdapter for Proto1_12_2 {
    fn version(&self) -> GameVersion {
        GameVersion::V1_12_2
    }

    fn current_state(&self) -> ConnectionState {
        self.state
    }

    fn compression_threshold(&self) -> Option<i32> {
        self.compression_threshold
    }

    fn encode_action(&mut self, action: ClientAction, out_buf: &mut BytesMut) -> Result<(), ProtocolError> {
        match action {
            ClientAction::InitiateLogin {
                server_host,
                server_port,
                username,
            } => {
                // 1. Goi Handshake (ID 0x00)
                let mut handshake_payload = BytesMut::new();
                write_varint(&mut handshake_payload, 0x00); // Packet ID
                write_varint(&mut handshake_payload, 340);  // Protocol 340 (1.12.2)
                write_string(&mut handshake_payload, &server_host);
                handshake_payload.put_u16(server_port);
                write_varint(&mut handshake_payload, 2);    // Next state: Login (2)
                let framed_hs = format_packet_payload(&handshake_payload, None)?;
                out_buf.put(framed_hs);

                // 2. Goi LoginStart (ID 0x00)
                let mut login_payload = BytesMut::new();
                write_varint(&mut login_payload, 0x00);     // Packet ID
                write_string(&mut login_payload, &username);
                let framed_login = format_packet_payload(&login_payload, None)?;
                out_buf.put(framed_login);

                self.state = ConnectionState::Login;
            }
            ClientAction::KeepAliveResponse { keep_alive_id } => {
                let mut payload = BytesMut::new();
                write_varint(&mut payload, 0x0B); // Packet ID KeepAlive 1.12.2
                payload.put_i64(keep_alive_id);
                let framed = format_packet_payload(&payload, self.compression_threshold)?;
                out_buf.put(framed);
            }
            ClientAction::ConfirmTeleport { teleport_id } => {
                let mut payload = BytesMut::new();
                write_varint(&mut payload, 0x00); // TeleportConfirm ID 0x00
                write_varint(&mut payload, teleport_id);
                let framed = format_packet_payload(&payload, self.compression_threshold)?;
                out_buf.put(framed);
            }
            ClientAction::SendPositionAndRotation {
                x,
                y,
                z,
                yaw,
                pitch,
                on_ground,
            } => {
                let mut payload = BytesMut::new();
                write_varint(&mut payload, 0x0E); // PlayerPositionAndLook ID 0x0E
                payload.put_f64(x);
                payload.put_f64(y);
                payload.put_f64(z);
                payload.put_f32(yaw);
                payload.put_f32(pitch);
                payload.put_u8(if on_ground { 1 } else { 0 });
                let framed = format_packet_payload(&payload, self.compression_threshold)?;
                out_buf.put(framed);
            }
            ClientAction::SendChatMessage { message } => {
                let mut payload = BytesMut::new();
                write_varint(&mut payload, 0x02); // ChatMessage ID 0x02
                write_string(&mut payload, &message);
                let framed = format_packet_payload(&payload, self.compression_threshold)?;
                out_buf.put(framed);
            }
            ClientAction::ClickContainerSlot {
                window_id,
                slot,
                button,
                mode,
                action_id,
                ..
            } => {
                let mut payload = BytesMut::new();
                write_varint(&mut payload, 0x07); // ClickWindow ID 0x07
                payload.put_u8(window_id);
                payload.put_i16(slot);
                payload.put_u8(button);
                payload.put_i16(action_id);
                write_varint(&mut payload, mode);
                payload.put_i16(-1); // Empty clicked item
                let framed = format_packet_payload(&payload, self.compression_threshold)?;
                out_buf.put(framed);
            }
            ClientAction::CloseContainerWindow { window_id } => {
                let mut payload = BytesMut::new();
                write_varint(&mut payload, 0x08); // CloseWindow ID 0x08
                payload.put_u8(window_id);
                let framed = format_packet_payload(&payload, self.compression_threshold)?;
                out_buf.put(framed);
            }
        }
        Ok(())
    }

    fn decode_packets(
        &mut self,
        in_buf: &mut BytesMut,
        out_auto_reply: &mut BytesMut,
    ) -> Result<Vec<ServerEvent>, ProtocolError> {
        let mut events = Vec::new();

        while in_buf.has_remaining() {
            let mut cursor = in_buf.clone().freeze();
            let packet_len = match read_varint(&mut cursor) {
                Ok(len) if len > 0 => len as usize,
                _ => break, // Chua nhan du byte do dai
            };

            let varint_len = in_buf.len() - cursor.remaining();
            if in_buf.len() < varint_len + packet_len {
                break; // Goi tin chua nhan du byte
            }

            // Cat dung doan packet
            in_buf.advance(varint_len);
            let packet_bytes = in_buf.split_to(packet_len);

            let mut reader = packet_bytes.freeze();
            let decompressed = parse_packet_payload(&mut reader, self.compression_threshold)?;
            let mut payload_buf = BytesMut::from(decompressed.as_slice()).freeze();

            let packet_id = read_varint(&mut payload_buf)?;

            match self.state {
                ConnectionState::Login => {
                    match packet_id {
                        0x00 => {
                            let reason = read_string(&mut payload_buf, 32767)?;
                            events.push(ServerEvent::Disconnected { reason });
                        }
                        0x02 => {
                            let uuid = read_string(&mut payload_buf, 36)?;
                            let username = read_string(&mut payload_buf, 16)?;
                            // 1.12.2: LoginSuccess chuyen thang sang Play (khong co Configuration)
                            self.state = ConnectionState::Play;
                            events.push(ServerEvent::LoginSuccess { uuid, username });
                        }
                        0x03 => {
                            let threshold = read_varint(&mut payload_buf)?;
                            self.compression_threshold = Some(threshold);
                            events.push(ServerEvent::CompressionEnabled { threshold });
                        }
                        _ => {}
                    }
                }
                ConnectionState::Play => {
                    match packet_id {
                        0x23 => {
                            // JoinGame (0x23 tren 1.12.2)
                            let entity_id = payload_buf.get_i32();
                            let _gamemode = payload_buf.get_u8();
                            let dimension = payload_buf.get_i32();
                            let _difficulty = payload_buf.get_u8();
                            let max_players = payload_buf.get_u8();
                            events.push(ServerEvent::JoinServerWorld {
                                entity_id,
                                dimension,
                                max_players,
                            });
                        }
                        0x1F => {
                            // KeepAlive (0x1F tren 1.12.2)
                            let keep_alive_id = payload_buf.get_i64();
                            // Tu dong phan hoi KeepAlive len Server
                            let _ = self.encode_action(ClientAction::KeepAliveResponse { keep_alive_id }, out_auto_reply);
                            events.push(ServerEvent::KeepAliveRequest { keep_alive_id });
                        }
                        0x2F => {
                            // PlayerPositionAndLook (0x2F tren 1.12.2)
                            let x = payload_buf.get_f64();
                            let y = payload_buf.get_f64();
                            let z = payload_buf.get_f64();
                            let yaw = payload_buf.get_f32();
                            let pitch = payload_buf.get_f32();
                            let _flags = payload_buf.get_u8();
                            let teleport_id = read_varint(&mut payload_buf)?;

                            // Tu dong xac nhan TeleportConfirm
                            let _ = self.encode_action(ClientAction::ConfirmTeleport { teleport_id }, out_auto_reply);
                            events.push(ServerEvent::SynchronizePlayerPosition {
                                x,
                                y,
                                z,
                                yaw,
                                pitch,
                                teleport_id,
                            });
                        }
                        0x0F => {
                            // ChatMessage (0x0F tren 1.12.2)
                            let json_message = read_string(&mut payload_buf, 32767)?;
                            let position = if payload_buf.has_remaining() { payload_buf.get_u8() } else { 0 };
                            events.push(ServerEvent::SystemChat {
                                json_message,
                                is_action_bar: position == 2,
                            });
                        }
                        0x0B => {
                            // BlockChange (0x0B tren 1.12.2)
                            let pos_packed = payload_buf.get_u64();
                            let (x, y, z) = decode_block_pos_1_12_2(pos_packed);
                            let block_combined = read_varint(&mut payload_buf)? as u16;
                            let id = block_combined >> 4;
                            let meta = block_combined & 0x0F;
                            events.push(ServerEvent::BlockChange {
                                x,
                                y,
                                z,
                                state: CanonicalBlockState::new(id, meta),
                            });
                        }
                        0x1A => {
                            // Disconnect (0x1A tren 1.12.2 Play)
                            let reason = read_string(&mut payload_buf, 32767)?;
                            events.push(ServerEvent::Disconnected { reason });
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }

        Ok(events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proto_1_12_2_handshake_and_login_flow() {
        let mut proto = Proto1_12_2::new();
        let mut out_buf = BytesMut::new();

        proto
            .encode_action(
                ClientAction::InitiateLogin {
                    server_host: "127.0.0.1".to_string(),
                    server_port: 25565,
                    username: "CasxTester".to_string(),
                },
                &mut out_buf,
            )
            .expect("Ma hoa Handshake phai thanh cong");

        assert_eq!(proto.current_state(), ConnectionState::Login);
        assert!(!out_buf.is_empty());

        // Gia lap Server phan hoi LoginSuccess (0x02)
        let mut server_response = BytesMut::new();
        let mut payload = BytesMut::new();
        write_varint(&mut payload, 0x02);
        write_string(&mut payload, "b50ad385-829d-3141-a216-7e7d7539ba7f");
        write_string(&mut payload, "CasxTester");
        let framed = format_packet_payload(&payload, None).expect("Framing");
        server_response.put(framed);

        let mut auto_reply = BytesMut::new();
        let events = proto
            .decode_packets(&mut server_response, &mut auto_reply)
            .expect("Giai ma LoginSuccess");

        assert_eq!(proto.current_state(), ConnectionState::Play);
        assert_eq!(events.len(), 1);
        match &events[0] {
            ServerEvent::LoginSuccess { username, .. } => {
                assert_eq!(username, "CasxTester");
            }
            _ => panic!("Su kien phai la LoginSuccess"),
        }
    }
}
