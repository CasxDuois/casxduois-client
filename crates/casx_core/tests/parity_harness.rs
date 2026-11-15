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
