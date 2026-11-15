// crates/casx_net/src/session/v776.rs
// Trinh quan ly phien Minecraft 26.2 (Protocol 776) kem trang thai Configuration

use bytes::{Bytes, BytesMut};
use casx_core::ProtocolError;
use crate::codec::frame::RawPacket;
use crate::codec::compression::PacketCompressor;
use crate::codec::varint::{write_varint_to_buf, read_varint_from_buf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStateV776 {
    Handshaking,
    Login,
    Configuration,
    Play,
    Disconnected,
}

#[derive(Debug, Clone)]
pub enum SessionEventV776 {
    LoginSuccess { uuid: String, username: String },
    ConfigurationStarted,
    DimensionRegistryReceived { min_y: i32, height: u32 },
    ConfigurationFinished,
    KeepAliveReceived { id: i64 },
    PlayerPosSync { x: f64, y: f64, z: f64, yaw: f32, pitch: f32, teleport_id: i32 },
    ChunkDataReceived { chunk_x: i32, chunk_z: i32, data_len: usize },
    Disconnect { reason: String },
}

pub struct SessionV776 {
    pub state: SessionStateV776,
    pub nickname: String,
    pub compressor: Option<PacketCompressor>,
    pub incoming_buffer: BytesMut,
    pub outgoing_buffer: BytesMut,
    pub min_y: i32,
    pub height: u32,
}

impl SessionV776 {
    pub const PROTOCOL_VERSION: i32 = 776;

    pub fn new(nickname: &str) -> Self {
        Self {
            state: SessionStateV776::Handshaking,
            nickname: nickname.to_string(),
            compressor: None,
            incoming_buffer: BytesMut::with_capacity(65536),
            outgoing_buffer: BytesMut::with_capacity(65536),
            min_y: -64,
            height: 384,
        }
    }

    /// Tao goi tin Handshake (0x00) va Login Start (0x00) cho Protocol 776
    pub fn create_handshake_and_login(&mut self, host: &str, port: u16) {
        // 1. Handshake Packet (0x00)
        let mut hs_payload = BytesMut::new();
        write_varint_to_buf(Self::PROTOCOL_VERSION, &mut hs_payload);
        write_varint_to_buf(host.len() as i32, &mut hs_payload);
        hs_payload.extend_from_slice(host.as_bytes());
        hs_payload.extend_from_slice(&port.to_be_bytes());
        write_varint_to_buf(2, &mut hs_payload); // next state: 2 (Login)

        let hs_packet = RawPacket::new(0x00, hs_payload.freeze());
        hs_packet.encode(&mut self.outgoing_buffer);

        self.state = SessionStateV776::Login;

        // 2. Login Start Packet (0x00)
        let mut login_payload = BytesMut::new();
        write_varint_to_buf(self.nickname.len() as i32, &mut login_payload);
        login_payload.extend_from_slice(self.nickname.as_bytes());
        // has_player_uuid = false (0x00)
        login_payload.extend_from_slice(&[0u8]);

        let login_packet = RawPacket::new(0x00, login_payload.freeze());
        login_packet.encode(&mut self.outgoing_buffer);
    }

    /// Xu ly du lieu bytes nhan tu socket TCP
    pub fn process_incoming_bytes(&mut self, bytes: &[u8]) -> Result<Vec<SessionEventV776>, ProtocolError> {
        self.incoming_buffer.extend_from_slice(bytes);
        let mut events = Vec::new();

        loop {
            let maybe_packet = if let Some(ref comp) = self.compressor {
                comp.decompress_packet(&mut self.incoming_buffer)?
            } else {
                RawPacket::decode(&mut self.incoming_buffer)?
            };

            if let Some(packet) = maybe_packet {
                if let Some(event) = self.handle_packet(packet)? {
                    events.push(event);
                }
            } else {
                break;
            }
        }

        Ok(events)
    }

    fn handle_packet(&mut self, packet: RawPacket) -> Result<Option<SessionEventV776>, ProtocolError> {
        match self.state {
            SessionStateV776::Login => match packet.id {
                0x03 => {
                    // Set Compression (threshold: VarInt)
                    let mut payload = BytesMut::from(&packet.payload[..]);
                    if let Some(threshold) = read_varint_from_buf(&mut payload)? {
                        self.compressor = Some(PacketCompressor::new(threshold));
                    }
                    Ok(None)
                }
                0x02 => {
                    // Login Success
                    let mut payload = BytesMut::from(&packet.payload[..]);
                    let uuid = if payload.len() >= 16 {
                        let uuid_bytes = payload.split_to(16);
                        format!("{:x?}", uuid_bytes)
                    } else {
                        "unknown-uuid".to_string()
                    };
                    let username = self.read_string(&mut payload).unwrap_or_else(|_| self.nickname.clone());

                    // GUI BAT BUOC: Login Acknowledged (0x03) de chuyen sang Configuration
                    self.send_login_acknowledged()?;
                    self.state = SessionStateV776::Configuration;

                    // Tu dong gui Client Information ban dau
                    self.send_client_information()?;

                    Ok(Some(SessionEventV776::LoginSuccess { uuid, username }))
                }
                0x00 => {
                    let mut payload = BytesMut::from(&packet.payload[..]);
                    let reason = self.read_string(&mut payload).unwrap_or_else(|_| "Disconnected".to_string());
                    self.state = SessionStateV776::Disconnected;
                    Ok(Some(SessionEventV776::Disconnect { reason }))
                }
                _ => Ok(None),
            },
            SessionStateV776::Configuration => match packet.id {
                0x0E => {
                    // Serverbound Known Packs -> phan hoi Select Known Packs
                    self.send_known_packs_reply()?;
                    Ok(None)
                }
                0x07 => {
                    // Registry Data (minecraft:dimension_type)
                    // Gia su lay duoc min_y = -64 va height = 384
                    self.min_y = -64;
                    self.height = 384;
                    Ok(Some(SessionEventV776::DimensionRegistryReceived { min_y: self.min_y, height: self.height }))
                }
                0x03 => {
                    // Finish Configuration tu Server -> gui lai Acknowledge Finish Configuration (0x03)
                    self.send_acknowledge_finish_configuration()?;
                    self.state = SessionStateV776::Play;
                    Ok(Some(SessionEventV776::ConfigurationFinished))
                }
                _ => Ok(None),
            },
            SessionStateV776::Play => match packet.id {
                0x24 => {
                    // Keep Alive (Play S->C)
                    if packet.payload.len() >= 8 {
                        let id = i64::from_be_bytes(packet.payload[0..8].try_into().unwrap());
                        self.send_keep_alive(id)?;
                        Ok(Some(SessionEventV776::KeepAliveReceived { id }))
                    } else {
                        Ok(None)
                    }
                }
                0x40 => {
                    // Synchronize Player Position (Player Position And Look S->C)
                    let mut payload = BytesMut::from(&packet.payload[..]);
                    if payload.len() >= 33 {
                        let x = f64::from_be_bytes(payload[0..8].try_into().unwrap());
                        let y = f64::from_be_bytes(payload[8..16].try_into().unwrap());
                        let z = f64::from_be_bytes(payload[16..24].try_into().unwrap());
                        let yaw = f32::from_be_bytes(payload[24..28].try_into().unwrap());
                        let pitch = f32::from_be_bytes(payload[28..32].try_into().unwrap());
                        let _flags = payload[32];
                        let mut rest = payload.split_off(33);
                        let teleport_id = read_varint_from_buf(&mut rest)?.unwrap_or(0);

                        // Gui Teleport Confirm (0x00)
                        self.send_teleport_confirm(teleport_id)?;

                        Ok(Some(SessionEventV776::PlayerPosSync { x, y, z, yaw, pitch, teleport_id }))
                    } else {
                        Ok(None)
                    }
                }
                0x27 => {
                    // Chunk Data and Update Light (Play S->C)
                    let payload = BytesMut::from(&packet.payload[..]);
                    if payload.len() >= 8 {
                        let chunk_x = i32::from_be_bytes(payload[0..4].try_into().unwrap());
                        let chunk_z = i32::from_be_bytes(payload[4..8].try_into().unwrap());
                        Ok(Some(SessionEventV776::ChunkDataReceived { chunk_x, chunk_z, data_len: payload.len() }))
                    } else {
                        Ok(None)
                    }
                }
                _ => Ok(None),
            },
            _ => Ok(None),
        }
    }

    /// Gui Login Acknowledged (C->S 0x03) o trang thai Login de vao Configuration
    pub fn send_login_acknowledged(&mut self) -> Result<(), ProtocolError> {
        let packet = RawPacket::new(0x03, Bytes::new());
        self.queue_packet(&packet)
    }

    /// Gui Client Information (C->S 0x00) o trang thai Configuration
    pub fn send_client_information(&mut self) -> Result<(), ProtocolError> {
        let mut payload = BytesMut::new();
        // locale "en_US"
        let locale = "en_US";
        write_varint_to_buf(locale.len() as i32, &mut payload);
        payload.extend_from_slice(locale.as_bytes());
        payload.extend_from_slice(&[10u8]); // view distance 10
        write_varint_to_buf(0, &mut payload); // chat mode: enabled
        payload.extend_from_slice(&[1u8]); // chat colors: true
        payload.extend_from_slice(&[0x7Fu8]); // skin parts
        write_varint_to_buf(1, &mut payload); // main hand: right

        let packet = RawPacket::new(0x00, payload.freeze());
        self.queue_packet(&packet)
    }

    /// Phan hoi Select Known Packs (C->S 0x07)
    pub fn send_known_packs_reply(&mut self) -> Result<(), ProtocolError> {
        let mut payload = BytesMut::new();
        write_varint_to_buf(1, &mut payload); // 1 pack
        let namespace = "minecraft";
        write_varint_to_buf(namespace.len() as i32, &mut payload);
        payload.extend_from_slice(namespace.as_bytes());
        let id = "core";
        write_varint_to_buf(id.len() as i32, &mut payload);
        payload.extend_from_slice(id.as_bytes());
        let version = "1.21.4";
        write_varint_to_buf(version.len() as i32, &mut payload);
        payload.extend_from_slice(version.as_bytes());

        let packet = RawPacket::new(0x07, payload.freeze());
        self.queue_packet(&packet)
    }

    /// Gui Acknowledge Finish Configuration (C->S 0x03)
    pub fn send_acknowledge_finish_configuration(&mut self) -> Result<(), ProtocolError> {
        let packet = RawPacket::new(0x03, Bytes::new());
        self.queue_packet(&packet)
    }

    /// Gui Keep Alive (C->S 0x15) trong Play
    pub fn send_keep_alive(&mut self, id: i64) -> Result<(), ProtocolError> {
        let packet = RawPacket::new(0x15, Bytes::copy_from_slice(&id.to_be_bytes()));
        self.queue_packet(&packet)
    }

    /// Gui Teleport Confirm (C->S 0x00) trong Play
    pub fn send_teleport_confirm(&mut self, teleport_id: i32) -> Result<(), ProtocolError> {
        let mut payload = BytesMut::new();
        write_varint_to_buf(teleport_id, &mut payload);
        let packet = RawPacket::new(0x00, payload.freeze());
        self.queue_packet(&packet)
    }

    fn queue_packet(&mut self, packet: &RawPacket) -> Result<(), ProtocolError> {
        if let Some(ref comp) = self.compressor {
            comp.compress_packet(packet, &mut self.outgoing_buffer)
        } else {
            packet.encode(&mut self.outgoing_buffer);
            Ok(())
        }
    }

    fn read_string(&self, buf: &mut BytesMut) -> Result<String, ProtocolError> {
        let len = read_varint_from_buf(buf)?
            .ok_or(ProtocolError::UnexpectedEof)? as usize;
        if buf.len() < len {
            return Err(ProtocolError::UnexpectedEof);
        }
        let str_bytes = buf.split_to(len);
        String::from_utf8(str_bytes.to_vec()).map_err(ProtocolError::InvalidString)
    }
}
