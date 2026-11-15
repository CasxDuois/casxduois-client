//! Bo dieu hop giao thuc Minecraft 26.2 (Protocol 770 / Configuration Phase & PalettedContainer).

use bytes::{Buf, BufMut, BytesMut};
use casx_core::codec::{read_string, read_varint, write_string, write_varint};
use casx_core::compression::{format_packet_payload, parse_packet_payload};
use casx_core::crypto::generate_offline_uuid;
use casx_core::world::{CanonicalBlockState, ChunkSection};
use casx_core::{
    ClientAction, ConnectionState, GameVersion, ProtocolAdapter, ProtocolError, ServerEvent,
};

pub struct Proto26_2 {
    state: ConnectionState,
    compression_threshold: Option<i32>,
    pub world_min_y: i32,
    pub world_section_count: usize,
}

impl Default for Proto26_2 {
    fn default() -> Self {
        Self::new()
    }
}

impl Proto26_2 {
    pub fn new() -> Self {
        Self {
            state: ConnectionState::Handshaking,
            compression_threshold: None,
            world_min_y: -64,
            world_section_count: 24, // Y = -64..320
        }
    }

    /// Giai ma PalettedContainer trong Chunk Section cua 26.2:
    /// Ho tro ca Single-Valued (0 bits), Indirect Palette (4..8 bits), va Direct Palette (>= 15 bits).
    pub fn decode_section_26_2(buf: &mut impl Buf) -> Result<ChunkSection, ProtocolError> {
        if buf.remaining() < 3 {
            return Err(ProtocolError::UnexpectedEof);
        }

        let non_air_count = buf.get_u16();
        let bits_per_entry = buf.get_u8() as usize;

        let mut section = ChunkSection::default();
        section.non_air_count = non_air_count;

        if bits_per_entry == 0 {
            // Single-valued palette: toan bo 4096 khoi co cung mot trang thai
            let state_id = read_varint(buf)? as u16;
            let _data_len = read_varint(buf)?;
            let state = CanonicalBlockState::new(state_id, 0);
            section.blocks.fill(state);
        } else if (4..=8).contains(&bits_per_entry) {
            // Indirect Palette: doc bang tra cuu palette roi doc mang bit u64
            let palette_len = read_varint(buf)? as usize;
            let mut palette = Vec::with_capacity(palette_len);
            for _ in 0..palette_len {
                palette.push(read_varint(buf)? as u16);
            }

            let data_array_len = read_varint(buf)? as usize;
            if buf.remaining() < data_array_len * 8 {
                return Err(ProtocolError::UnexpectedEof);
            }

            let mut data_words = Vec::with_capacity(data_array_len);
            for _ in 0..data_array_len {
                data_words.push(buf.get_u64());
            }

            let entries_per_word = 64 / bits_per_entry;
            let mask = (1u64 << bits_per_entry) - 1;

            for i in 0..4096 {
                let word_idx = i / entries_per_word;
                let sub_idx = i % entries_per_word;
                if word_idx < data_words.len() {
                    let palette_idx = ((data_words[word_idx] >> (sub_idx * bits_per_entry)) & mask) as usize;
                    let block_id = if palette_idx < palette.len() {
                        palette[palette_idx]
                    } else {
                        0
                    };
                    section.blocks[i] = CanonicalBlockState::new(block_id, 0);
                }
            }
        } else {
            // Direct Palette (bits_per_entry >= 15): moi muc la ID trang thai truc tiep
            let data_array_len = read_varint(buf)? as usize;
            if buf.remaining() < data_array_len * 8 {
                return Err(ProtocolError::UnexpectedEof);
            }

            let mut data_words = Vec::with_capacity(data_array_len);
            for _ in 0..data_array_len {
                data_words.push(buf.get_u64());
            }

            let entries_per_word = 64 / bits_per_entry;
            let mask = (1u64 << bits_per_entry) - 1;

            for i in 0..4096 {
                let word_idx = i / entries_per_word;
                let sub_idx = i % entries_per_word;
                if word_idx < data_words.len() {
                    let state_id = ((data_words[word_idx] >> (sub_idx * bits_per_entry)) & mask) as u16;
                    section.blocks[i] = CanonicalBlockState::new(state_id, 0);
                }
            }
        }

        Ok(section)
    }
}

impl ProtocolAdapter for Proto26_2 {
    fn version(&self) -> GameVersion {
        GameVersion::V26_2
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
                let mut hs = BytesMut::new();
                write_varint(&mut hs, 0x00);
                write_varint(&mut hs, 770); // Protocol 770 (26.2 Next-Gen)
                write_string(&mut hs, &server_host);
                hs.put_u16(server_port);
                write_varint(&mut hs, 2);   // Next state: Login (2)
                let framed_hs = format_packet_payload(&hs, None)?;
                out_buf.put(framed_hs);

                // 2. Goi LoginStart (ID 0x00 tren 26.2 gom ca Username va UUID 128-bit)
                let mut login = BytesMut::new();
                write_varint(&mut login, 0x00);
                write_string(&mut login, &username);
                let uuid = generate_offline_uuid(&username);
                login.put_u128(uuid);
                let framed_login = format_packet_payload(&login, None)?;
                out_buf.put(framed_login);

                self.state = ConnectionState::Login;
            }
            ClientAction::KeepAliveResponse { keep_alive_id } => {
                let mut payload = BytesMut::new();
                write_varint(&mut payload, 0x18); // KeepAlive ID tren 26.2 Play
                payload.put_i64(keep_alive_id);
                let framed = format_packet_payload(&payload, self.compression_threshold)?;
                out_buf.put(framed);
            }
            ClientAction::ConfirmTeleport { teleport_id } => {
                let mut payload = BytesMut::new();
                write_varint(&mut payload, 0x00); // ConfirmTeleportation ID 0x00
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
                write_varint(&mut payload, 0x1B); // SetPlayerPositionAndRotation tren 26.2 Play
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
                write_varint(&mut payload, 0x06); // ChatMessage tren 26.2 Play
                write_string(&mut payload, &message);
                let framed = format_packet_payload(&payload, self.compression_threshold)?;
                out_buf.put(framed);
            }
            ClientAction::ClickContainerSlot {
                window_id,
                state_id,
                slot,
                button,
                mode,
                ..
            } => {
                let mut payload = BytesMut::new();
                write_varint(&mut payload, 0x0E); // ClickContainer tren 26.2
                payload.put_u8(window_id);
                write_varint(&mut payload, state_id);
                payload.put_i16(slot);
                payload.put_u8(button);
                write_varint(&mut payload, mode);
                write_varint(&mut payload, 0); // 0 changed slots
                payload.put_i16(-1);           // Empty carried item
                let framed = format_packet_payload(&payload, self.compression_threshold)?;
                out_buf.put(framed);
            }
            ClientAction::CloseContainerWindow { window_id } => {
                let mut payload = BytesMut::new();
                write_varint(&mut payload, 0x0F);
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
                _ => break,
            };

            let varint_len = in_buf.len() - cursor.remaining();
            if in_buf.len() < varint_len + packet_len {
                break;
            }

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
                            let uuid_bytes = if payload_buf.remaining() >= 16 {
                                payload_buf.get_u128()
                            } else {
                                0
                            };
                            let username = read_string(&mut payload_buf, 16)?;

                            // 26.2: Sau khi nhan LoginSuccess (0x02), bat buoc phai gui LoginAcknowledged (0x03)
                            let mut ack = BytesMut::new();
                            write_varint(&mut ack, 0x03); // LoginAcknowledged ID 0x03
                            let framed_ack = format_packet_payload(&ack, self.compression_threshold)?;
                            out_auto_reply.put(framed_ack);

                            // Chuyen sang trang thai Configuration
                            self.state = ConnectionState::Configuration;
                            events.push(ServerEvent::LoginSuccess {
                                uuid: format!("{:032x}", uuid_bytes),
                                username,
                            });
                        }
                        0x03 => {
                            let threshold = read_varint(&mut payload_buf)?;
                            self.compression_threshold = Some(threshold);
                            events.push(ServerEvent::CompressionEnabled { threshold });
                        }
                        _ => {}
                    }
                }
                ConnectionState::Configuration => {
                    match packet_id {
                        0x03 => {
                            // FinishConfiguration (0x03): Server hoan tat cau hinh
                            // Client phai gui phan hoi AcknowledgeFinishConfiguration (0x03)
                            let mut ack = BytesMut::new();
                            write_varint(&mut ack, 0x03);
                            let framed = format_packet_payload(&ack, self.compression_threshold)?;
                            out_auto_reply.put(framed);

                            self.state = ConnectionState::Play;
                            events.push(ServerEvent::ConfigurationComplete);
                        }
                        0x0E => {
                            // SelectKnownPacks (0x0E tren 26.2 Configuration)
                            // Phan hoi SelectKnownPacks (0x07) rong de dong y dung core pack
                            let mut reply = BytesMut::new();
                            write_varint(&mut reply, 0x07);
                            write_varint(&mut reply, 0); // 0 known packs
                            let framed = format_packet_payload(&reply, self.compression_threshold)?;
                            out_auto_reply.put(framed);
                        }
                        _ => {}
                    }
                }
                ConnectionState::Play => {
                    match packet_id {
                        0x2B => {
                            // LoginPlay (0x2B tren 26.2)
                            let entity_id = payload_buf.get_i32();
                            events.push(ServerEvent::JoinServerWorld {
                                entity_id,
                                dimension: 0,
                                max_players: 100,
                            });
                        }
                        0x26 => {
                            // KeepAlive (0x26 tren 26.2)
                            let keep_alive_id = payload_buf.get_i64();
                            let _ = self.encode_action(ClientAction::KeepAliveResponse { keep_alive_id }, out_auto_reply);
                            events.push(ServerEvent::KeepAliveRequest { keep_alive_id });
                        }
                        0x40 => {
                            // SynchronizePlayerPosition (0x40 tren 26.2)
                            let x = payload_buf.get_f64();
                            let y = payload_buf.get_f64();
                            let z = payload_buf.get_f64();
                            let yaw = payload_buf.get_f32();
                            let pitch = payload_buf.get_f32();
                            let _flags = payload_buf.get_u32();
                            let teleport_id = read_varint(&mut payload_buf)?;

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
    fn test_proto_26_2_configuration_phase_transition() {
        let mut proto = Proto26_2::new();
        let mut out_buf = BytesMut::new();

        proto
            .encode_action(
                ClientAction::InitiateLogin {
                    server_host: "127.0.0.1".to_string(),
                    server_port: 25565,
                    username: "ModernTester".to_string(),
                },
                &mut out_buf,
            )
            .expect("InitiateLogin");

        assert_eq!(proto.current_state(), ConnectionState::Login);

        // Gia lap Server phan hoi LoginSuccess (0x02)
        let mut server_buf = BytesMut::new();
        let mut payload = BytesMut::new();
        write_varint(&mut payload, 0x02);
        payload.put_u128(123456789);
        write_string(&mut payload, "ModernTester");
        let framed = format_packet_payload(&payload, None).expect("Framing");
        server_buf.put(framed);

        let mut auto_reply = BytesMut::new();
        let events = proto
            .decode_packets(&mut server_buf, &mut auto_reply)
            .expect("LoginSuccess decode");

        // Tren 26.2, phai chuyen sang Configuration State, va gui LoginAcknowledged (0x03)
        assert_eq!(proto.current_state(), ConnectionState::Configuration);
        assert!(!auto_reply.is_empty());
        assert_eq!(events.len(), 1);

        // Gia lap Server gui FinishConfiguration (0x03)
        let mut config_buf = BytesMut::new();
        let mut finish_payload = BytesMut::new();
        write_varint(&mut finish_payload, 0x03);
        let framed_finish = format_packet_payload(&finish_payload, None).expect("Framing");
        config_buf.put(framed_finish);

        let mut auto_reply2 = BytesMut::new();
        let events2 = proto
            .decode_packets(&mut config_buf, &mut auto_reply2)
            .expect("FinishConfiguration decode");

        assert_eq!(proto.current_state(), ConnectionState::Play);
        assert_eq!(events2.len(), 1);
        match &events2[0] {
            ServerEvent::ConfigurationComplete => {}
            _ => panic!("Phai nhan ConfigurationComplete"),
        }
    }

    #[test]
    fn test_proto_26_2_paletted_container_single_valued() {
        let mut buf = BytesMut::new();
        buf.put_u16(4096); // non_air_count
        buf.put_u8(0);     // bits_per_entry = 0 (single-valued)
        write_varint(&mut buf, 15); // Block ID 15
        write_varint(&mut buf, 0);  // Data array len 0

        let mut reader = buf.freeze();
        let section = Proto26_2::decode_section_26_2(&mut reader).expect("Decode section");

        assert_eq!(section.non_air_count, 4096);
        assert_eq!(section.blocks[0], CanonicalBlockState::new(15, 0));
        assert_eq!(section.blocks[4095], CanonicalBlockState::new(15, 0));
    }
}
