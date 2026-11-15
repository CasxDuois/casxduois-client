//! Khung giao tiep mang chuan hoa Canonical Bridge (Doc lap hoan toan giua 1.12.2 va 26.2).
//!
//! Tuan thu nguyen tac Version Isolation: casx-world, casx-sim, casx-render, va casx-ui
//! TUYET DOI KHONG import struct goi tin cu the cua phien ban. Toan bo du lieu duoc
//! chuyen hoa thanh CanonicalServerEvent va CanonicalClientCmd.

use crate::codec::varint::write_varint_to_buf;
use crate::session::v340::{SessionEventV340, SessionV340};
use crate::session::v776::{SessionEventV776, SessionV776};
use bytes::BytesMut;
use casx_core::{CanonicalBlockStateId, ChunkPos, DVec3};

/// Su kien chuan hoa tu Server day ve Client (Doc lap phien ban 1.12.2 hay 26.2)
#[derive(Debug, Clone, PartialEq)]
pub enum CanonicalServerEvent {
    PlayReady {
        entity_id: i32,
        min_y: i32,
        world_height: u32,
    },
    TeleportPlayer {
        pos: DVec3,
        yaw: f32,
        pitch: f32,
        teleport_id: i32,
    },
    ChunkColumnData {
        pos: ChunkPos,
        sections: Vec<Vec<CanonicalBlockStateId>>, // Moi section gom dung 4096 khoi (16x16x16)
    },
    BlockUpdate {
        x: i32,
        y: i32,
        z: i32,
        state: CanonicalBlockStateId,
    },
    SystemChat {
        formatted_text: String,
        is_action_bar: bool,
    },
    Disconnect {
        reason: String,
    },
}

/// Lenh chuan hoa tu Client gui len Server
#[derive(Debug, Clone, PartialEq)]
pub enum CanonicalClientCmd {
    MovePlayer {
        pos: DVec3,
        yaw: f32,
        pitch: f32,
        on_ground: bool,
    },
    SendChat(String),
    SwingArm,
    StartDigging { x: i32, y: i32, z: i32, face: u8 },
    FinishDigging { x: i32, y: i32, z: i32, face: u8 },
}

/// Trait dieu hop mang da phien ban
pub trait ProtocolAdapter: Send {
    fn protocol_version(&self) -> u32;
    fn decode_inbound(
        &mut self,
        rx_buf: &mut BytesMut,
        tx_buf: &mut BytesMut, // Cho phep phan hoi tuc thi KeepAlive / Configuration ngay tai tang mang
        events: &mut Vec<CanonicalServerEvent>,
    ) -> Result<(), String>;
    fn encode_outbound(
        &mut self,
        cmd: CanonicalClientCmd,
        tx_buf: &mut BytesMut,
    ) -> Result<(), String>;
}

/// Adapter cho Minecraft 1.12.2 (Protocol 340)
pub struct AdapterV340 {
    pub session: SessionV340,
}

impl AdapterV340 {
    pub fn new(username: &str) -> Self {
        Self {
            session: SessionV340::new(username),
        }
    }
}

impl ProtocolAdapter for AdapterV340 {
    fn protocol_version(&self) -> u32 {
        340
    }

    fn decode_inbound(
        &mut self,
        rx_buf: &mut BytesMut,
        tx_buf: &mut BytesMut,
        events: &mut Vec<CanonicalServerEvent>,
    ) -> Result<(), String> {
        if !rx_buf.is_empty() {
            let data = rx_buf.split().freeze();
            let raw_events = self
                .session
                .process_incoming_bytes(&data)
                .map_err(|e| format!("{:?}", e))?;

            for ev in raw_events {
                match ev {
                    SessionEventV340::LoginSuccess { username, .. } => {
                        log::info!("[AdapterV340] Dang nhap thanh cong: {}", username);
                    }
                    SessionEventV340::JoinGame { entity_id, .. } => {
                        events.push(CanonicalServerEvent::PlayReady {
                            entity_id,
                            min_y: 0,
                            world_height: 256,
                        });
                    }
                    SessionEventV340::PlayerPosLook { x, y, z, yaw, pitch, teleport_id } => {
                        events.push(CanonicalServerEvent::TeleportPlayer {
                            pos: DVec3::new(x, y, z),
                            yaw,
                            pitch,
                            teleport_id,
                        });
                    }
                    SessionEventV340::BlockChange { pos, block_state } => {
                        let x = (pos >> 38) as i32;
                        let y = ((pos >> 26) & 0xFFF) as i32;
                        let z = (pos << 38 >> 38) as i32;
                        events.push(CanonicalServerEvent::BlockUpdate {
                            x,
                            y,
                            z,
                            state: CanonicalBlockStateId(block_state),
                        });
                    }
                    SessionEventV340::Disconnect { reason } => {
                        events.push(CanonicalServerEvent::Disconnect { reason });
                    }
                    _ => {}
                }
            }
        }

        if !self.session.outgoing_buffer.is_empty() {
            tx_buf.extend_from_slice(&self.session.outgoing_buffer.split());
        }

        Ok(())
    }

    fn encode_outbound(
        &mut self,
        cmd: CanonicalClientCmd,
        tx_buf: &mut BytesMut,
    ) -> Result<(), String> {
        match cmd {
            CanonicalClientCmd::MovePlayer { pos, yaw, pitch, on_ground } => {
                let mut payload = BytesMut::new();
                payload.extend_from_slice(&pos.x.to_be_bytes());
                payload.extend_from_slice(&pos.y.to_be_bytes());
                payload.extend_from_slice(&pos.z.to_be_bytes());
                payload.extend_from_slice(&yaw.to_be_bytes());
                payload.extend_from_slice(&pitch.to_be_bytes());
                payload.extend_from_slice(&[if on_ground { 1 } else { 0 }]);

                let mut packet = BytesMut::new();
                write_varint_to_buf(0x0E, &mut packet); // Player Position And Look (C->S)
                packet.extend_from_slice(&payload);

                let mut frame = BytesMut::new();
                write_varint_to_buf(packet.len() as i32, &mut frame);
                frame.extend_from_slice(&packet);

                tx_buf.extend_from_slice(&frame);
            }
            CanonicalClientCmd::SendChat(msg) => {
                let mut packet = BytesMut::new();
                write_varint_to_buf(0x02, &mut packet); // Chat Message (C->S)
                let msg_bytes = msg.as_bytes();
                write_varint_to_buf(msg_bytes.len() as i32, &mut packet);
                packet.extend_from_slice(msg_bytes);

                let mut frame = BytesMut::new();
                write_varint_to_buf(packet.len() as i32, &mut frame);
                frame.extend_from_slice(&packet);

                tx_buf.extend_from_slice(&frame);
            }
            CanonicalClientCmd::SwingArm => {
                let mut packet = BytesMut::new();
                write_varint_to_buf(0x1A, &mut packet); // Animation / Swing Arm
                write_varint_to_buf(0, &mut packet); // Main hand

                let mut frame = BytesMut::new();
                write_varint_to_buf(packet.len() as i32, &mut frame);
                frame.extend_from_slice(&packet);

                tx_buf.extend_from_slice(&frame);
            }
            _ => {}
        }
        Ok(())
    }
}

/// Adapter cho Minecraft 26.2 (Protocol 776)
pub struct AdapterV776 {
    pub session: SessionV776,
}

impl AdapterV776 {
    pub fn new(username: &str) -> Self {
        Self {
            session: SessionV776::new(username),
        }
    }
}

impl ProtocolAdapter for AdapterV776 {
    fn protocol_version(&self) -> u32 {
        776
    }

    fn decode_inbound(
        &mut self,
        rx_buf: &mut BytesMut,
        tx_buf: &mut BytesMut,
        events: &mut Vec<CanonicalServerEvent>,
    ) -> Result<(), String> {
        if !rx_buf.is_empty() {
            let data = rx_buf.split().freeze();
            let raw_events = self
                .session
                .process_incoming_bytes(&data)
                .map_err(|e| format!("{:?}", e))?;

            for ev in raw_events {
                match ev {
                    SessionEventV776::LoginSuccess { username, .. } => {
                        log::info!("[AdapterV776] Dang nhap thanh cong: {}", username);
                    }
                    SessionEventV776::ConfigurationFinished => {
                        events.push(CanonicalServerEvent::PlayReady {
                            entity_id: 1,
                            min_y: self.session.min_y,
                            world_height: self.session.height,
                        });
                    }
                    SessionEventV776::PlayerPosSync { x, y, z, yaw, pitch, teleport_id } => {
                        events.push(CanonicalServerEvent::TeleportPlayer {
                            pos: DVec3::new(x, y, z),
                            yaw,
                            pitch,
                            teleport_id,
                        });
                    }
                    SessionEventV776::Disconnect { reason } => {
                        events.push(CanonicalServerEvent::Disconnect { reason });
                    }
                    _ => {}
                }
            }
        }

        if !self.session.outgoing_buffer.is_empty() {
            tx_buf.extend_from_slice(&self.session.outgoing_buffer.split());
        }

        Ok(())
    }

    fn encode_outbound(
        &mut self,
        cmd: CanonicalClientCmd,
        tx_buf: &mut BytesMut,
    ) -> Result<(), String> {
        match cmd {
            CanonicalClientCmd::MovePlayer { pos, yaw, pitch, on_ground } => {
                let mut payload = BytesMut::new();
                payload.extend_from_slice(&pos.x.to_be_bytes());
                payload.extend_from_slice(&pos.y.to_be_bytes());
                payload.extend_from_slice(&pos.z.to_be_bytes());
                payload.extend_from_slice(&yaw.to_be_bytes());
                payload.extend_from_slice(&pitch.to_be_bytes());
                payload.extend_from_slice(&[if on_ground { 1 } else { 0 }]);

                let mut packet = BytesMut::new();
                write_varint_to_buf(0x1B, &mut packet); // Set Player Position And Rotation (26.2 Play C->S)
                packet.extend_from_slice(&payload);

                let mut frame = BytesMut::new();
                write_varint_to_buf(packet.len() as i32, &mut frame);
                frame.extend_from_slice(&packet);

                tx_buf.extend_from_slice(&frame);
            }
            CanonicalClientCmd::SendChat(msg) => {
                let mut packet = BytesMut::new();
                write_varint_to_buf(0x06, &mut packet); // Chat Message (26.2 Play C->S)
                let msg_bytes = msg.as_bytes();
                write_varint_to_buf(msg_bytes.len() as i32, &mut packet);
                packet.extend_from_slice(msg_bytes);
                packet.extend_from_slice(&0i64.to_be_bytes()); // Timestamp
                packet.extend_from_slice(&0i64.to_be_bytes()); // Salt
                write_varint_to_buf(0, &mut packet); // Signature count 0

                let mut frame = BytesMut::new();
                write_varint_to_buf(packet.len() as i32, &mut frame);
                frame.extend_from_slice(&packet);

                tx_buf.extend_from_slice(&frame);
            }
            CanonicalClientCmd::SwingArm => {
                let mut packet = BytesMut::new();
                write_varint_to_buf(0x36, &mut packet); // Swing Arm 26.2
                write_varint_to_buf(0, &mut packet);

                let mut frame = BytesMut::new();
                write_varint_to_buf(packet.len() as i32, &mut frame);
                frame.extend_from_slice(&packet);

                tx_buf.extend_from_slice(&frame);
            }
            _ => {}
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_v340_and_v776_creation() {
        let mut adapter_v340 = AdapterV340::new("Test340");
        assert_eq!(adapter_v340.protocol_version(), 340);

        let mut adapter_v776 = AdapterV776::new("Test776");
        assert_eq!(adapter_v776.protocol_version(), 776);

        // Kiem tra encode MovePlayer qua Adapter
        let mut tx_buf = BytesMut::new();
        let cmd = CanonicalClientCmd::MovePlayer {
            pos: DVec3::new(10.0, 64.0, -15.0),
            yaw: 90.0,
            pitch: 0.0,
            on_ground: true,
        };

        assert!(adapter_v340.encode_outbound(cmd.clone(), &mut tx_buf).is_ok());
        assert!(!tx_buf.is_empty());

        let mut tx_buf_776 = BytesMut::new();
        assert!(adapter_v776.encode_outbound(cmd, &mut tx_buf_776).is_ok());
        assert!(!tx_buf_776.is_empty());
    }
}
