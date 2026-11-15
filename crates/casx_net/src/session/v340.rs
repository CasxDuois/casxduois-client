// crates/casx_net/src/session/v340.rs
// Minecraft 1.12.2 (Protocol 340) session manager

use crate::codec::compression::PacketCompressor;
use crate::codec::frame::RawPacket;
use crate::codec::varint::{read_varint_from_buf, write_varint_to_buf};
use bytes::{Bytes, BytesMut};
use casx_core::ProtocolError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    Handshaking,
    Login,
    Play,
    Disconnected,
}

#[derive(Debug, Clone)]
pub enum SessionEventV340 {
    LoginSuccess {
        uuid: String,
        username: String,
    },
    KeepAliveReceived {
        id: i64,
    },
    JoinGame {
        entity_id: i32,
        gamemode: u8,
        dimension: i32,
    },
    PlayerPosLook {
        x: f64,
        y: f64,
        z: f64,
        yaw: f32,
        pitch: f32,
        teleport_id: i32,
    },
    ChunkData {
        chunk_x: i32,
        chunk_z: i32,
        ground_up_continuous: bool,
        primary_bit_mask: u16,
        data: Vec<u8>,
    },
    BlockChange {
        pos: u64,
        block_state: u32,
    },
    MultiBlockChange {
        chunk_x: i32,
        chunk_z: i32,
        records: Vec<(u8, u8, u8, u32)>,
    },
    Disconnect {
        reason: String,
    },
}

pub struct SessionV340 {
    pub state: SessionState,
    pub nickname: String,
    pub compressor: Option<PacketCompressor>,
    pub incoming_buffer: BytesMut,
    pub outgoing_buffer: BytesMut,
}

impl SessionV340 {
    pub const PROTOCOL_VERSION: i32 = 340;

    pub fn new(nickname: &str) -> Self {
        Self {
            state: SessionState::Handshaking,
            nickname: nickname.to_string(),
            compressor: None,
            incoming_buffer: BytesMut::with_capacity(65536),
            outgoing_buffer: BytesMut::with_capacity(65536),
        }
    }

    /// Creates Handshake (0x00) and Login Start (0x00) packets
    pub fn create_handshake_and_login(&mut self, host: &str, port: u16) {
        // 1. Handshake Packet (0x00)
        let mut hs_payload = BytesMut::new();
        write_varint_to_buf(Self::PROTOCOL_VERSION, &mut hs_payload);
        // host string
        write_varint_to_buf(host.len() as i32, &mut hs_payload);
        hs_payload.extend_from_slice(host.as_bytes());
        // port u16
        hs_payload.extend_from_slice(&port.to_be_bytes());
        // next state: 2 (Login)
        write_varint_to_buf(2, &mut hs_payload);

        let hs_packet = RawPacket::new(0x00, hs_payload.freeze());
        hs_packet.encode(&mut self.outgoing_buffer);

        self.state = SessionState::Login;

        // 2. Login Start Packet (0x00)
        let mut login_payload = BytesMut::new();
        write_varint_to_buf(self.nickname.len() as i32, &mut login_payload);
        login_payload.extend_from_slice(self.nickname.as_bytes());

        let login_packet = RawPacket::new(0x00, login_payload.freeze());
        login_packet.encode(&mut self.outgoing_buffer);
    }

    /// Processes inbound bytes from TCP socket and returns list of decoded events
    pub fn process_incoming_bytes(
        &mut self,
        bytes: &[u8],
    ) -> Result<Vec<SessionEventV340>, ProtocolError> {
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

    fn handle_packet(
        &mut self,
        packet: RawPacket,
    ) -> Result<Option<SessionEventV340>, ProtocolError> {
        match self.state {
            SessionState::Login => match packet.id {
                0x03 => {
                    // Set Compression (threshold: VarInt)
                    let mut payload = BytesMut::from(&packet.payload[..]);
                    if let Some(threshold) = read_varint_from_buf(&mut payload)? {
                        self.compressor = Some(PacketCompressor::new(threshold));
                    }
                    Ok(None)
                }
                0x02 => {
                    // Login Success (uuid: String, username: String)
                    let mut payload = BytesMut::from(&packet.payload[..]);
                    let uuid = self.read_string(&mut payload)?;
                    let username = self.read_string(&mut payload)?;
                    self.state = SessionState::Play;
                    Ok(Some(SessionEventV340::LoginSuccess { uuid, username }))
                }
                0x00 => {
                    // Disconnect
                    let mut payload = BytesMut::from(&packet.payload[..]);
                    let reason = self
                        .read_string(&mut payload)
                        .unwrap_or_else(|_| "Unknown".to_string());
                    self.state = SessionState::Disconnected;
                    Ok(Some(SessionEventV340::Disconnect { reason }))
                }
                _ => Ok(None),
            },
            SessionState::Play => match packet.id {
                0x1F => {
                    // Keep Alive (S->C)
                    if packet.payload.len() >= 8 {
                        let id = i64::from_be_bytes(packet.payload[0..8].try_into().unwrap());
                        self.send_keep_alive(id)?;
                        Ok(Some(SessionEventV340::KeepAliveReceived { id }))
                    } else {
                        Ok(None)
                    }
                }
                0x23 => {
                    // Join Game
                    if packet.payload.len() >= 4 {
                        let entity_id =
                            i32::from_be_bytes(packet.payload[0..4].try_into().unwrap());
                        let gamemode = if packet.payload.len() > 4 {
                            packet.payload[4]
                        } else {
                            0
                        };
                        let dimension = if packet.payload.len() >= 9 {
                            i32::from_be_bytes(packet.payload[5..9].try_into().unwrap())
                        } else {
                            0
                        };
                        Ok(Some(SessionEventV340::JoinGame {
                            entity_id,
                            gamemode,
                            dimension,
                        }))
                    } else {
                        Ok(None)
                    }
                }
                0x2F => {
                    // Player Position And Look (S->C)
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

                        // Automatically send Teleport Confirm (0x00) and Player Pos Look (0x0E)
                        self.send_teleport_confirm(teleport_id)?;
                        self.send_player_pos_look(x, y, z, yaw, pitch, true)?;

                        Ok(Some(SessionEventV340::PlayerPosLook {
                            x,
                            y,
                            z,
                            yaw,
                            pitch,
                            teleport_id,
                        }))
                    } else {
                        Ok(None)
                    }
                }
                0x20 => {
                    // Chunk Data (S->C)
                    let mut payload = BytesMut::from(&packet.payload[..]);
                    if payload.len() >= 10 {
                        let chunk_x = i32::from_be_bytes(payload[0..4].try_into().unwrap());
                        let chunk_z = i32::from_be_bytes(payload[4..8].try_into().unwrap());
                        let ground_up_continuous = payload[8] != 0;
                        let mut rest = payload.split_off(9);
                        let primary_bit_mask = read_varint_from_buf(&mut rest)?.unwrap_or(0) as u16;
                        let size = read_varint_from_buf(&mut rest)?.unwrap_or(0) as usize;
                        let data = if rest.len() >= size {
                            rest.split_to(size).to_vec()
                        } else {
                            rest.to_vec()
                        };

                        Ok(Some(SessionEventV340::ChunkData {
                            chunk_x,
                            chunk_z,
                            ground_up_continuous,
                            primary_bit_mask,
                            data,
                        }))
                    } else {
                        Ok(None)
                    }
                }
                0x1A => {
                    // Disconnect (Play)
                    let mut payload = BytesMut::from(&packet.payload[..]);
                    let reason = self
                        .read_string(&mut payload)
                        .unwrap_or_else(|_| "Disconnected".to_string());
                    self.state = SessionState::Disconnected;
                    Ok(Some(SessionEventV340::Disconnect { reason }))
                }
                _ => Ok(None),
            },
            _ => Ok(None),
        }
    }

    /// Sends Keep Alive (C->S 0x0B)
    pub fn send_keep_alive(&mut self, id: i64) -> Result<(), ProtocolError> {
        let packet = RawPacket::new(0x0B, Bytes::copy_from_slice(&id.to_be_bytes()));
        self.queue_packet(&packet)
    }

    /// Sends Teleport Confirm (C->S 0x00)
    pub fn send_teleport_confirm(&mut self, teleport_id: i32) -> Result<(), ProtocolError> {
        let mut payload = BytesMut::new();
        write_varint_to_buf(teleport_id, &mut payload);
        let packet = RawPacket::new(0x00, payload.freeze());
        self.queue_packet(&packet)
    }

    /// Sends Player Position And Look (C->S 0x0E)
    pub fn send_player_pos_look(
        &mut self,
        x: f64,
        y: f64,
        z: f64,
        yaw: f32,
        pitch: f32,
        on_ground: bool,
    ) -> Result<(), ProtocolError> {
        let mut payload = BytesMut::with_capacity(33);
        payload.extend_from_slice(&x.to_be_bytes());
        payload.extend_from_slice(&y.to_be_bytes());
        payload.extend_from_slice(&z.to_be_bytes());
        payload.extend_from_slice(&yaw.to_be_bytes());
        payload.extend_from_slice(&pitch.to_be_bytes());
        payload.extend_from_slice(&[if on_ground { 1 } else { 0 }]);

        let packet = RawPacket::new(0x0E, payload.freeze());
        self.queue_packet(&packet)
    }

    /// Sends Player Position (C->S 0x0D)
    pub fn send_player_position(
        &mut self,
        x: f64,
        y: f64,
        z: f64,
        on_ground: bool,
    ) -> Result<(), ProtocolError> {
        let mut payload = BytesMut::with_capacity(25);
        payload.extend_from_slice(&x.to_be_bytes());
        payload.extend_from_slice(&y.to_be_bytes());
        payload.extend_from_slice(&z.to_be_bytes());
        payload.extend_from_slice(&[if on_ground { 1 } else { 0 }]);

        let packet = RawPacket::new(0x0D, payload.freeze());
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
        let len = read_varint_from_buf(buf)?.ok_or(ProtocolError::UnexpectedEof)? as usize;
        if buf.len() < len {
            return Err(ProtocolError::UnexpectedEof);
        }
        let str_bytes = buf.split_to(len);
        String::from_utf8(str_bytes.to_vec()).map_err(ProtocolError::InvalidString)
    }
}
