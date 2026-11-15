//! Bo phan tich van ban chat da dinh dang (§ legacy va JSON Component).

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Doan van ban kem thuoc tinh mau sac va dinh dang kieu chu
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StyledTextSpan {
    pub text: String,
    pub color: [u8; 3],
    pub bold: bool,
    pub italic: bool,
    pub underlined: bool,
    pub strikethrough: bool,
    pub obfuscated: bool,
}

impl Default for StyledTextSpan {
    fn default() -> Self {
        Self {
            text: String::new(),
            color: [255, 255, 255], // Mac dinh mau trang
            bold: false,
            italic: false,
            underlined: false,
            strikethrough: false,
            obfuscated: false,
        }
    }
}

/// Chuyen doi ma mau ky tu co dien (0-9, a-f) sang RGB
pub fn legacy_color_code_to_rgb(code: char) -> Option<[u8; 3]> {
    match code.to_ascii_lowercase() {
        '0' => Some([0, 0, 0]),
        '1' => Some([0, 0, 170]),
        '2' => Some([0, 170, 0]),
        '3' => Some([0, 170, 170]),
        '4' => Some([170, 0, 0]),
        '5' => Some([170, 0, 170]),
        '6' => Some([255, 170, 0]),
        '7' => Some([170, 170, 170]),
        '8' => Some([85, 85, 85]),
        '9' => Some([85, 85, 255]),
        'a' => Some([85, 255, 85]),
        'b' => Some([85, 255, 255]),
        'c' => Some([255, 85, 85]),
        'd' => Some([255, 85, 255]),
        'e' => Some([255, 255, 85]),
        'f' => Some([255, 255, 255]),
        _ => None,
    }
}

/// Chuyen doi ten mau Minecraft hoac chuoi Hex `#RRGGBB` sang RGB
pub fn named_or_hex_to_rgb(name: &str) -> [u8; 3] {
    if let Some(hex) = name.strip_prefix('#') {
        if hex.len() == 6 {
            if let Ok(val) = u32::from_str_radix(hex, 16) {
                return [((val >> 16) & 0xFF) as u8, ((val >> 8) & 0xFF) as u8, (val & 0xFF) as u8];
            }
        }
    }
    match name.to_ascii_lowercase().as_str() {
        "black" => [0, 0, 0],
        "dark_blue" => [0, 0, 170],
        "dark_green" => [0, 170, 0],
        "dark_aqua" => [0, 170, 170],
        "dark_red" => [170, 0, 0],
        "dark_purple" => [170, 0, 170],
        "gold" => [255, 170, 0],
        "gray" => [170, 170, 170],
        "dark_gray" => [85, 85, 85],
        "blue" => [85, 85, 255],
        "green" => [85, 255, 85],
        "aqua" => [85, 255, 255],
        "red" => [255, 85, 85],
        "light_purple" => [255, 85, 255],
        "yellow" => [255, 255, 85],
        _ => [255, 255, 255],
    }
}

/// Phan tich chuoi chua ma mau Section Sign `§` thanh danh sach StyledTextSpan
pub fn parse_legacy_section_text(input: &str, base_style: StyledTextSpan) -> Vec<StyledTextSpan> {
    let mut spans = Vec::new();
    let mut current = base_style;
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '§' {
            if let Some(&code) = chars.peek() {
                chars.next();
                // Kiem tra ma mau Hex: §#RRGGBB
                if code == '#' {
                    let mut hex_str = String::with_capacity(6);
                    for _ in 0..6 {
                        if let Some(&h) = chars.peek() {
                            if h.is_ascii_hexdigit() {
                                hex_str.push(h);
                                chars.next();
                            } else {
                                break;
                            }
                        }
                    }
                    if hex_str.len() == 6 {
                        if !current.text.is_empty() {
                            spans.push(current.clone());
                            current.text.clear();
                        }
                        if let Ok(val) = u32::from_str_radix(&hex_str, 16) {
                            current.color = [((val >> 16) & 0xFF) as u8, ((val >> 8) & 0xFF) as u8, (val & 0xFF) as u8];
                        }
                        continue;
                    }
                }

                if let Some(rgb) = legacy_color_code_to_rgb(code) {
                    if !current.text.is_empty() {
                        spans.push(current.clone());
                        current.text.clear();
                    }
                    current.color = rgb;
                    current.bold = false;
                    current.italic = false;
                    current.underlined = false;
                    current.strikethrough = false;
                    current.obfuscated = false;
                } else {
                    match code.to_ascii_lowercase() {
                        'k' | 'l' | 'm' | 'n' | 'o' => {
                            if !current.text.is_empty() {
                                spans.push(current.clone());
                                current.text.clear();
                            }
                            match code.to_ascii_lowercase() {
                                'k' => current.obfuscated = true,
                                'l' => current.bold = true,
                                'm' => current.strikethrough = true,
                                'n' => current.underlined = true,
                                'o' => current.italic = true,
                                _ => {}
                            }
                        }
                        'r' => {
                            if !current.text.is_empty() {
                                spans.push(current.clone());
                                current.text.clear();
                            }
                            current = StyledTextSpan::default();
                        }
                        _ => {
                            current.text.push('§');
                            current.text.push(code);
                        }
                    }
                }
            }
        } else {
            current.text.push(ch);
        }
    }

    if !current.text.is_empty() {
        spans.push(current);
    }

    spans
}

/// Phan tich chuoi JSON Text Component thanh danh sach cac StyledTextSpan
pub fn parse_json_text_component(json_str: &str) -> Vec<StyledTextSpan> {
    let mut spans = Vec::new();
    if let Ok(value) = serde_json::from_str::<Value>(json_str) {
        flatten_json_node(&value, StyledTextSpan::default(), &mut spans);
    } else {
        // Neu khong phai JSON hop le, coi nhu van ban thuong
        spans.push(StyledTextSpan {
            text: json_str.to_string(),
            ..Default::default()
        });
    }
    spans
}

fn flatten_json_node(val: &Value, mut inherited: StyledTextSpan, out: &mut Vec<StyledTextSpan>) {
    match val {
        Value::String(s) => {
            // Chuoi ben trong co the chua them ma mau §
            let legacy_parsed = parse_legacy_section_text(s, inherited);
            out.extend(legacy_parsed);
        }
        Value::Array(arr) => {
            for item in arr {
                flatten_json_node(item, inherited.clone(), out);
            }
        }
        Value::Object(map) => {
            if let Some(c) = map.get("color").and_then(|v| v.as_str()) {
                inherited.color = named_or_hex_to_rgb(c);
            }
            if let Some(b) = map.get("bold").and_then(|v| v.as_bool()) {
                inherited.bold = b;
            }
            if let Some(i) = map.get("italic").and_then(|v| v.as_bool()) {
                inherited.italic = i;
            }
            if let Some(u) = map.get("underlined").and_then(|v| v.as_bool()) {
                inherited.underlined = u;
            }
            if let Some(s) = map.get("strikethrough").and_then(|v| v.as_bool()) {
                inherited.strikethrough = s;
            }
            if let Some(o) = map.get("obfuscated").and_then(|v| v.as_bool()) {
                inherited.obfuscated = o;
            }

            if let Some(text) = map.get("text").and_then(|v| v.as_str()) {
                if !text.is_empty() {
                    let legacy_parsed = parse_legacy_section_text(text, inherited.clone());
                    out.extend(legacy_parsed);
                }
            }

            if let Some(extra) = map.get("extra").and_then(|v| v.as_array()) {
                for child in extra {
                    flatten_json_node(child, inherited.clone(), out);
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_legacy_section_colors_and_hex() {
        let text = "§aGreen §lBoldGreen §#FF55AAYellowPink §rReset";
        let spans = parse_legacy_section_text(text, StyledTextSpan::default());

        assert_eq!(spans[0].text, "Green ");
        assert_eq!(spans[0].color, [85, 255, 85]);
        assert!(!spans[0].bold);

        assert_eq!(spans[1].text, "BoldGreen ");
        assert_eq!(spans[1].color, [85, 255, 85]);
        assert!(spans[1].bold);

        assert_eq!(spans[2].text, "YellowPink ");
        assert_eq!(spans[2].color, [255, 85, 170]);

        assert_eq!(spans[3].text, "Reset");
        assert_eq!(spans[3].color, [255, 255, 255]);
    }

    #[test]
    fn test_json_chat_component_complex() {
        let json = r##"{
            "text": "Server: ",
            "color": "gold",
            "bold": true,
            "extra": [
                {
                    "text": "Chao mung ",
                    "color": "green",
                    "bold": false
                },
                {
                    "text": "CasxDuois",
                    "color": "#00FFAA",
                    "underlined": true
                }
            ]
        }"##;

        let spans = parse_json_text_component(json);
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[0].text, "Server: ");
        assert_eq!(spans[0].color, [255, 170, 0]);
        assert!(spans[0].bold);

        assert_eq!(spans[1].text, "Chao mung ");
        assert_eq!(spans[1].color, [85, 255, 85]);
        assert!(!spans[1].bold);

        assert_eq!(spans[2].text, "CasxDuois");
        assert_eq!(spans[2].color, [0, 255, 170]);
        assert!(spans[2].underlined);
    }
}
