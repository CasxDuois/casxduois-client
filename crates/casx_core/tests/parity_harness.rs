//! Bo kiem thu doi chieu vi sai (Differential Parity Harness) cho casx_core.

use casx_core::chat_parser::{parse_json_text_component, parse_legacy_section_text, StyledTextSpan};
use casx_core::codec::{
    decode_block_pos_1_12_2, decode_block_pos_26_2, encode_block_pos_1_12_2, encode_block_pos_26_2,
};
use casx_core::crypto::{format_uuid, generate_offline_uuid, mojang_server_hash};
use casx_core::nbt::{read_classic_nbt, read_nameless_nbt, write_classic_nbt, write_nameless_nbt, NbtTag};
use bytes::BytesMut;
use std::collections::HashMap;

#[test]
fn test_parity_block_position_encoding_between_versions() {
    let test_coords = [
        (0, 64, 0),
        (-15420, 255, 8912),
        (29_999_984, -64, -29_999_984),
        (-29_999_984, 2047, 29_999_984),
        (1, -2048, -1),
    ];

    for (x, y, z) in test_coords {
        let packed_1_12_2 = encode_block_pos_1_12_2(x, y, z);
        assert_eq!(decode_block_pos_1_12_2(packed_1_12_2), (x, y, z));

        let packed_26_2 = encode_block_pos_26_2(x, y, z);
        assert_eq!(decode_block_pos_26_2(packed_26_2), (x, y, z));
    }
}

#[test]
fn test_parity_offline_uuid_v3_matches_vanilla() {
    let uuid = generate_offline_uuid("Notch");
    let formatted = format_uuid(uuid);
    assert_eq!(formatted, "b50ad385-829d-3141-a216-7e7d7539ba7f");
}

#[test]
fn test_parity_mojang_server_hash_twos_complement() {
    let hash_notch = mojang_server_hash("Notch", b"", b"");
    assert_eq!(hash_notch, "4ed1f46bbe04bc756bcb17c0c7ce3e4632f06a48");

    let hash_jeb = mojang_server_hash("jeb_", b"", b"");
    assert_eq!(hash_jeb, "-7c9d5b0044c130109a5d7b5fb5c317c02b4e28c1");

    let hash_simon = mojang_server_hash("simon", b"", b"");
    assert_eq!(hash_simon, "88e16a1019277b15d58faf0541e11910eb756f6");
}

#[test]
fn test_parity_classic_and_nameless_nbt() {
    let mut compound = HashMap::new();
    compound.insert("ServerBrand".to_string(), NbtTag::String("CasxDuois-Engine".to_string()));
    compound.insert("ViewDistance".to_string(), NbtTag::Int(16));
    compound.insert("SimulationDistance".to_string(), NbtTag::Int(10));
    compound.insert("ReducedDebugInfo".to_string(), NbtTag::Byte(0));

    let root = NbtTag::Compound(compound);

    // Kiem tra Classic NBT (1.12.2)
    let mut buf_classic = BytesMut::new();
    write_classic_nbt(&mut buf_classic, "", &root).expect("Ghi Classic NBT");
    let mut reader_classic = buf_classic.freeze();
    let (_, decoded_classic) = read_classic_nbt(&mut reader_classic).expect("Doc Classic NBT");
    assert_eq!(decoded_classic.get_str("ServerBrand"), Some("CasxDuois-Engine"));
    assert_eq!(decoded_classic.get_int("ViewDistance"), Some(16));

    // Kiem tra Nameless NBT (26.2)
    let mut buf_nameless = BytesMut::new();
    write_nameless_nbt(&mut buf_nameless, &root).expect("Ghi Nameless NBT");
    let mut reader_nameless = buf_nameless.freeze();
    let decoded_nameless = read_nameless_nbt(&mut reader_nameless).expect("Doc Nameless NBT");
    assert_eq!(decoded_nameless.get_str("ServerBrand"), Some("CasxDuois-Engine"));
    assert_eq!(decoded_nameless.get_int("SimulationDistance"), Some(10));
}

#[test]
fn test_parity_chat_formatting_pipeline() {
    let raw_legacy = "§6[Server] §aChao mung §fTester §#00AAFFda ket noi!";
    let spans = parse_legacy_section_text(raw_legacy, StyledTextSpan::default());
    assert_eq!(spans.len(), 4);
    assert_eq!(spans[0].text, "[Server] ");
    assert_eq!(spans[0].color, [255, 170, 0]); // gold
    assert_eq!(spans[1].text, "Chao mung ");
    assert_eq!(spans[1].color, [85, 255, 85]); // green
    assert_eq!(spans[2].text, "Tester ");
    assert_eq!(spans[2].color, [255, 255, 255]); // white
    assert_eq!(spans[3].text, "da ket noi!");
    assert_eq!(spans[3].color, [0, 170, 255]); // hex #00AAFF

    let raw_json = r##"{"text":"CasxDuois","color":"#FF0055","bold":true}"##;
    let json_spans = parse_json_text_component(raw_json);
    assert_eq!(json_spans.len(), 1);
    assert_eq!(json_spans[0].text, "CasxDuois");
    assert_eq!(json_spans[0].color, [255, 0, 85]);
    assert!(json_spans[0].bold);
}

use casx_core::ctl_server::CasxCtlServer;
use casx_core::native_mods::{CasxNativeMod, FreelookMod, HookContext};
use casx_core::physics::{Aabb, LocalPlayerPhysics};
use casx_core::raycast::{ray_intersects_aabb, raycast_block, BlockFace};
use casx_core::world::{CanonicalBlockState, ChunkColumn, ClientWorldCache};
use glam::{DVec3, IVec3};

#[test]
fn test_parity_freefall_gravity_20_tps() {
    let world = ClientWorldCache::new();
    let mut physics = LocalPlayerPhysics::default();
    physics.curr_pos = DVec3::new(0.0, 100.0, 0.0);
    physics.prev_pos = physics.curr_pos;

    // Mo phong 10 ticks roi tu do khong co vat can
    for _ in 0..10 {
        physics.step_physics_tick(&world, 0.0, 0.0, false);
    }

    // Kiem tra van toc va vi tri theo cong thuc v_y = (v_y - 0.08) * 0.98
    assert!(physics.curr_pos.y < 96.5 && physics.curr_pos.y > 95.5);
    assert!(!physics.on_ground);
}

#[test]
fn test_parity_ground_landing_and_collision() {
    let mut world = ClientWorldCache::new();
    let mut chunk = ChunkColumn::new(0, 0, 0, 16);
    let stone = CanonicalBlockState::new(1, 0);

    for x in 0..16 {
        for z in 0..16 {
            chunk.set_block(x, 60, z, stone);
        }
    }
    world.insert_chunk(chunk);

    let mut physics = LocalPlayerPhysics::default();
    physics.curr_pos = DVec3::new(8.0, 63.0, 8.0);

    for _ in 0..20 {
        physics.step_physics_tick(&world, 0.0, 0.0, false);
    }

    assert_eq!(physics.curr_pos.y, 61.0);
    assert!(physics.on_ground);
}

#[test]
fn test_parity_3d_dda_raycast_voxel_faces() {
    let mut world = ClientWorldCache::new();
    let mut chunk = ChunkColumn::new(0, 0, 0, 16);
    let stone = CanonicalBlockState::new(1, 0);
    chunk.set_block(5, 64, 5, stone);
    world.insert_chunk(chunk);

    // 1. Nhin tu phia Tay sang Dong (X+): phai cham vao mat West cua khoi
    let origin_west = DVec3::new(2.0, 64.5, 5.5);
    let hit_west = raycast_block(&world, origin_west, DVec3::new(1.0, 0.0, 0.0), 5.0).expect("Hit West");
    assert_eq!(hit_west.block_pos, IVec3::new(5, 64, 5));
    assert_eq!(hit_west.face, BlockFace::West);

    // 2. Nhin tu tren xuong (Y-): phai cham vao mat Top cua khoi
    let origin_top = DVec3::new(5.5, 68.0, 5.5);
    let hit_top = raycast_block(&world, origin_top, DVec3::new(0.0, -1.0, 0.0), 5.0).expect("Hit Top");
    assert_eq!(hit_top.block_pos, IVec3::new(5, 64, 5));
    assert_eq!(hit_top.face, BlockFace::Top);
}

#[test]
fn test_parity_ray_intersects_aabb_pvp_hitbox() {
    let player_aabb = Aabb::new(
        DVec3::new(9.7, 60.0, 9.7),
        DVec3::new(10.3, 61.8, 10.3),
    );

    let eye_origin = DVec3::new(10.0, 61.5, 7.0);
    let attack_dir = DVec3::new(0.0, 0.0, 1.0);

    let hit = ray_intersects_aabb(eye_origin, attack_dir, &player_aabb, 4.0);
    assert!(hit.is_some());
    let distance = hit.unwrap();
    assert!((distance - 2.7).abs() < 1e-4);
}

#[test]
fn test_parity_freelook_independent_angles() {
    let mut freelook = FreelookMod::default();
    let mut render_yaw = 0.0f32;
    let mut render_pitch = 0.0f32;
    let mut render_fov = 70.0f32;

    let mut ctx = HookContext {
        player_pos: DVec3::ZERO,
        server_yaw: 90.0,
        server_pitch: 15.0,
        render_yaw: &mut render_yaw,
        render_pitch: &mut render_pitch,
        render_fov: &mut render_fov,
        is_alt_held: true,
        is_zoom_held: false,
    };

    freelook.on_camera_update(&mut ctx);
    assert_eq!(*ctx.render_yaw, 90.0);

    freelook.free_yaw = 270.0;
    freelook.on_camera_update(&mut ctx);
    assert_eq!(*ctx.render_yaw, 270.0);
    assert_eq!(ctx.server_yaw, 90.0); // Khong bi thay doi goc nhin gui len server
}

#[test]
fn test_parity_ctl_json_rpc_telemetry_flow() {
    let ctl = CasxCtlServer::default();

    let req_status = r#"{"jsonrpc":"2.0","method":"casx.status","id":100}"#;
    let res = ctl.handle_request(req_status);
    let json: serde_json::Value = serde_json::from_str(&res).expect("Valid JSON");
    assert_eq!(json["result"]["in_game"], false);
    assert_eq!(json["id"], 100);

    let req_chat = r#"{"jsonrpc":"2.0","method":"casx.chat","params":{"message":"/login password123"},"id":101}"#;
    let res_chat = ctl.handle_request(req_chat);
    let json_chat: serde_json::Value = serde_json::from_str(&res_chat).expect("Valid JSON");
    assert_eq!(json_chat["result"]["sent"], true);
}
