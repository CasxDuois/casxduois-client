//! Giao dien hop nhat HoYoPlay Hub x Legacy Launcher x Lunar Client HUD bang egui.

use casx_core::GameVersion;
use egui::{Align2, Color32, Pos2, Rounding, Stroke, Vec2};

/// Kieu tai khoan nguoi choi
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountType {
    Offline,
    ElyBy,
    Microsoft,
}

/// Che do man hinh hien tai cua ung dung
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenMode {
    Hub,         // Sanh cho HoYoPlay Hub
    InGame,      // Trong the gioi 3D
    ModSettings, // Bang cai dat Mod (Right Shift)
}

/// Hanh dong phat sinh tu giao dien nguoi dung can Engine xu ly
#[derive(Debug, Clone, PartialEq)]
pub enum UiAction {
    None,
    ConnectServer {
        host: String,
        port: u16,
        username: String,
        version: GameVersion,
    },
    Disconnect,
    SwitchVersion(GameVersion),
    SendChatMessage(String),
    SelectHotbarSlot(usize),
}

/// Widget HUD trong game co the keo-tha va hut tu tinh
#[derive(Debug, Clone)]
pub struct HudWidget {
    pub name: &'static str,
    pub pos: Pos2,
    pub enabled: bool,
}

/// Thong tin mot o tren thanh Hotbar 9 o
#[derive(Debug, Clone, PartialEq)]
pub struct HotbarSlot {
    pub item_id: i32,
    pub count: u8,
    pub display_name: String,
}

/// Bo dieu phoi toan bo giao dien CasxDuois Client
pub struct CasxUiApp {
    pub screen_mode: ScreenMode,
    pub account_type: AccountType,
    pub username: String,
    pub server_address: String,
    pub selected_version: GameVersion,

    // Lunar Client HUD Settings
    pub keystrokes_enabled: bool,
    pub cps_counter_enabled: bool,
    pub fps_ping_enabled: bool,
    pub scoreboard_enabled: bool,
    pub lmb_cps: u32,
    pub rmb_cps: u32,
    pub key_w: bool,
    pub key_a: bool,
    pub key_s: bool,
    pub key_d: bool,
    pub key_space: bool,

    // In-Game State
    pub selected_hotbar_slot: usize,
    pub hotbar_slots: [Option<HotbarSlot>; 9],
    pub chat_messages: Vec<String>,
    pub chat_input: String,
    pub is_chat_open: bool,
    pub scoreboard_title: String,
    pub scoreboard_lines: Vec<(String, i32)>,
    pub tab_list_players: Vec<(String, u32)>,
    pub is_tab_held: bool,
}

impl Default for CasxUiApp {
    fn default() -> Self {
        Self {
            screen_mode: ScreenMode::Hub,
            account_type: AccountType::Offline,
            username: "CasxPlayer".to_string(),
            server_address: "127.0.0.1:25565".to_string(),
            selected_version: GameVersion::V1_12_2,
            keystrokes_enabled: true,
            cps_counter_enabled: true,
            fps_ping_enabled: true,
            scoreboard_enabled: true,
            lmb_cps: 0,
            rmb_cps: 0,
            key_w: false,
            key_a: false,
            key_s: false,
            key_d: false,
            key_space: false,
            selected_hotbar_slot: 0,
            hotbar_slots: [
                Some(HotbarSlot { item_id: 276, count: 1, display_name: "Diamond Sword".into() }),
                Some(HotbarSlot { item_id: 278, count: 1, display_name: "Diamond Pickaxe".into() }),
                Some(HotbarSlot { item_id: 364, count: 64, display_name: "Steak".into() }),
                None,
                None,
                None,
                None,
                None,
                None,
            ],
            chat_messages: vec![
                "[System] Chuyen vao the gioi thanh cong.".to_string(),
                "[Server] Chao mung ban den voi CasxDuois Engine!".to_string(),
            ],
            chat_input: String::new(),
            is_chat_open: false,
            scoreboard_title: "CASXDUOIS NETWORK".to_string(),
            scoreboard_lines: vec![
                ("Online: 1".to_string(), 10),
                ("Ping: 24ms".to_string(), 9),
                ("Rank: Default".to_string(), 8),
                ("www.casxduois.io".to_string(), 1),
            ],
            tab_list_players: vec![
                ("CasxPlayer".to_string(), 24),
            ],
            is_tab_held: false,
        }
    }
}

impl CasxUiApp {
    /// Ve toan bo giao dien dua theo che do man hinh va du lieu thoi gian thuc
    pub fn render(
        &mut self,
        ctx: &egui::Context,
        in_game: bool,
        fps: f64,
        ping_ms: u32,
    ) -> UiAction {
        let mut action = UiAction::None;

        if !in_game {
            // 1. Man hinh sanh cho HoYoPlay Hub x Legacy Quick-Switch Bar
            action = self.render_hub(ctx);
        } else {
            // 2. Lop phu Lunar Client HUD trong game
            self.render_in_game_hud(ctx, fps, ping_ms);

            // 3. Hotbar 9 slots o day man hinh
            if let Some(act) = self.render_hotbar(ctx) {
                action = act;
            }

            // 4. Khung Chat o goc duoi ben trai
            if let Some(act) = self.render_chat_panel(ctx) {
                action = act;
            }

            // 5. Bang diem Scoreboard o ben phai
            if self.scoreboard_enabled {
                self.render_scoreboard(ctx);
            }

            // 6. Danh sach nguoi choi Tab List o tren cung
            if self.is_tab_held {
                self.render_tab_list(ctx);
            }

            // 7. Bang dieu khien Mod Menu khi nhan Right Shift
            if self.screen_mode == ScreenMode::ModSettings {
                self.render_mod_settings(ctx);
            }
        }

        action
    }

    /// Ve Sanh cho HoYoPlay Hub va thanh Legacy Quick-Switch Bar
    fn render_hub(&mut self, ctx: &egui::Context) -> UiAction {
        let mut action = UiAction::None;

        // Banner chinh giua phong cach HoYoPlay Hub
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(Color32::from_rgb(15, 18, 26)))
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(80.0);
                    ui.heading(
                        egui::RichText::new("CASXDUOIS CLIENT")
                            .size(38.0)
                            .strong()
                            .color(Color32::from_rgb(220, 230, 255)),
                    );
                    ui.label(
                        egui::RichText::new("Multiplayer-Only State-Driven Minecraft Engine")
                            .size(16.0)
                            .color(Color32::from_rgb(120, 140, 180)),
                    );

                    ui.add_space(40.0);
                    let version_label = match self.selected_version {
                        GameVersion::V1_12_2 => "Dang chon: Minecraft 1.12.2 (Protocol 340)",
                        GameVersion::V26_2 => "Dang chon: Minecraft 26.2 Next-Gen (Protocol 776)",
                    };
                    ui.label(
                        egui::RichText::new(version_label)
                            .size(15.0)
                            .color(Color32::from_rgb(100, 220, 160)),
                    );
                });
            });

        // Thanh dieu khien Legacy Quick-Switch Bar o day man hinh
        egui::TopBottomPanel::bottom("legacy_quick_switch_bar")
            .min_height(70.0)
            .frame(egui::Frame::none().fill(Color32::from_rgb(22, 27, 38)))
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.add_space(20.0);

                    // Chon tai khoan (Offline / Ely.by / Microsoft)
                    ui.label(egui::RichText::new("Tai khoan:").color(Color32::WHITE));
                    ui.selectable_value(&mut self.account_type, AccountType::Offline, "Offline");
                    ui.selectable_value(&mut self.account_type, AccountType::ElyBy, "Ely.by");
                    ui.selectable_value(&mut self.account_type, AccountType::Microsoft, "Microsoft");

                    ui.add_space(15.0);
                    ui.label(egui::RichText::new("Ten:").color(Color32::WHITE));
                    ui.add(egui::TextEdit::singleline(&mut self.username).desired_width(110.0));

                    ui.add_space(15.0);
                    // Nut trao doi nong phien ban (Hot-Switch 1.12.2 <-> 26.2)
                    let btn_ver_text = match self.selected_version {
                        GameVersion::V1_12_2 => "[1.12.2]",
                        GameVersion::V26_2 => "[26.2]",
                    };
                    if ui.button(egui::RichText::new(btn_ver_text).strong().color(Color32::YELLOW)).clicked() {
                        self.selected_version = match self.selected_version {
                            GameVersion::V1_12_2 => GameVersion::V26_2,
                            GameVersion::V26_2 => GameVersion::V1_12_2,
                        };
                        action = UiAction::SwitchVersion(self.selected_version);
                    }

                    ui.add_space(15.0);
                    ui.label(egui::RichText::new("Server:").color(Color32::WHITE));
                    ui.add(egui::TextEdit::singleline(&mut self.server_address).desired_width(160.0));

                    ui.add_space(25.0);
                    // Nut ket noi lon noi bat
                    let connect_btn = egui::Button::new(
                        egui::RichText::new("KET NOI SERVER")
                            .size(16.0)
                            .strong()
                            .color(Color32::BLACK),
                    )
                    .fill(Color32::from_rgb(0, 220, 130))
                    .min_size(Vec2::new(170.0, 42.0));

                    if ui.add(connect_btn).clicked() {
                        let parts: Vec<&str> = self.server_address.split(':').collect();
                        let host = parts.first().unwrap_or(&"127.0.0.1").to_string();
                        let port = parts.get(1).and_then(|p| p.parse::<u16>().ok()).unwrap_or(25565);

                        action = UiAction::ConnectServer {
                            host,
                            port,
                            username: self.username.clone(),
                            version: self.selected_version,
                        };
                    }
                });
            });

        action
    }

    /// Ve cac Widget HUD Lunar Client tren man hinh
    fn render_in_game_hud(&self, ctx: &egui::Context, fps: f64, ping_ms: u32) {
        // 1. FPS & Ping Widget (Goc tren ben trai)
        if self.fps_ping_enabled {
            egui::Area::new(egui::Id::new("hud_fps_ping"))
                .fixed_pos(Pos2::new(12.0, 12.0))
                .show(ctx, |ui| {
                    let frame = egui::Frame::none()
                        .fill(Color32::from_rgba_premultiplied(0, 0, 0, 150))
                        .rounding(Rounding::same(4.0))
                        .stroke(Stroke::new(1.0f32, Color32::from_rgba_premultiplied(255, 255, 255, 40)))
                        .inner_margin(6.0);

                    frame.show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(format!("{:.0} FPS | {} ms", fps, ping_ms))
                                .size(14.0)
                                .color(Color32::GREEN)
                                .strong(),
                        );
                    });
                });
        }

        // 2. Keystrokes Widget (Goc giua ben trai)
        if self.keystrokes_enabled {
            egui::Area::new(egui::Id::new("hud_keystrokes"))
                .fixed_pos(Pos2::new(12.0, 50.0))
                .show(ctx, |ui| {
                    let frame = egui::Frame::none()
                        .fill(Color32::from_rgba_premultiplied(0, 0, 0, 140))
                        .rounding(Rounding::same(4.0))
                        .inner_margin(4.0);

                    frame.show(ui, |ui| {
                        ui.vertical_centered(|ui| {
                            Self::render_key_box(ui, "W", self.key_w, 40.0, 36.0);
                            ui.horizontal(|ui| {
                                Self::render_key_box(ui, "A", self.key_a, 40.0, 36.0);
                                Self::render_key_box(ui, "S", self.key_s, 40.0, 36.0);
                                Self::render_key_box(ui, "D", self.key_d, 40.0, 36.0);
                            });
                            Self::render_key_box(ui, "SPACE", self.key_space, 126.0, 24.0);
                        });
                    });
                });
        }
    }

    /// Ve thanh Hotbar 9 o o day man hinh
    fn render_hotbar(&mut self, ctx: &egui::Context) -> Option<UiAction> {
        let mut clicked_slot = None;

        egui::TopBottomPanel::bottom("hud_hotbar")
            .min_height(50.0)
            .frame(egui::Frame::none().fill(Color32::TRANSPARENT))
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.horizontal(|ui| {
                        for slot_idx in 0..9 {
                            let is_selected = slot_idx == self.selected_hotbar_slot;
                            let border_col = if is_selected {
                                Color32::WHITE
                            } else {
                                Color32::from_rgba_premultiplied(100, 100, 100, 160)
                            };

                            let frame = egui::Frame::none()
                                .fill(Color32::from_rgba_premultiplied(20, 20, 20, 200))
                                .stroke(Stroke::new(if is_selected { 2.0_f32 } else { 1.0_f32 }, border_col))
                                .rounding(Rounding::same(4.0))
                                .inner_margin(4.0);

                            frame.show(ui, |ui| {
                                ui.set_min_size(Vec2::new(42.0, 42.0));
                                if let Some(slot) = &self.hotbar_slots[slot_idx] {
                                    let btn_txt = format!("{}\nx{}", slot.display_name.chars().next().unwrap_or('?'), slot.count);
                                    if ui.button(egui::RichText::new(btn_txt).size(11.0)).clicked() {
                                        clicked_slot = Some(slot_idx);
                                    }
                                } else {
                                    if ui.button(egui::RichText::new(" ").size(11.0)).clicked() {
                                        clicked_slot = Some(slot_idx);
                                    }
                                }
                            });
                        }
                    });
                });
            });

        if let Some(idx) = clicked_slot {
            self.selected_hotbar_slot = idx;
            Some(UiAction::SelectHotbarSlot(idx))
        } else {
            None
        }
    }

    /// Ve khung Chat o goc duoi ben trai man hinh
    fn render_chat_panel(&mut self, ctx: &egui::Context) -> Option<UiAction> {
        let mut send_action = None;

        egui::Area::new(egui::Id::new("hud_chat_panel"))
            .fixed_pos(Pos2::new(12.0, 320.0))
            .show(ctx, |ui| {
                let frame = egui::Frame::none()
                    .fill(Color32::from_rgba_premultiplied(0, 0, 0, 120))
                    .rounding(Rounding::same(4.0))
                    .inner_margin(6.0);

                frame.show(ui, |ui| {
                    ui.set_width(340.0);
                    // Hien thi toi da 6 tin nhan gan nhat
                    let start_idx = self.chat_messages.len().saturating_sub(6);
                    for msg in &self.chat_messages[start_idx..] {
                        ui.label(egui::RichText::new(msg).size(13.0).color(Color32::WHITE));
                    }

                    if self.is_chat_open {
                        ui.separator();
                        let response = ui.text_edit_singleline(&mut self.chat_input);
                        if response.lost_focus() && ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                            if !self.chat_input.trim().is_empty() {
                                let msg = self.chat_input.clone();
                                self.chat_messages.push(format!("[Ban] {}", msg));
                                self.chat_input.clear();
                                self.is_chat_open = false;
                                send_action = Some(UiAction::SendChatMessage(msg));
                            }
                        }
                    }
                });
            });

        send_action
    }

    /// Ve bang diem Scoreboard o mep phai man hinh
    fn render_scoreboard(&self, ctx: &egui::Context) {
        egui::Area::new(egui::Id::new("hud_scoreboard"))
            .fixed_pos(Pos2::new(650.0, 100.0))
            .show(ctx, |ui| {
                let frame = egui::Frame::none()
                    .fill(Color32::from_rgba_premultiplied(0, 0, 0, 140))
                    .rounding(Rounding::same(4.0))
                    .inner_margin(8.0);

                frame.show(ui, |ui| {
                    ui.set_min_width(160.0);
                    ui.heading(
                        egui::RichText::new(&self.scoreboard_title)
                            .size(13.0)
                            .strong()
                            .color(Color32::YELLOW),
                    );
                    ui.separator();

                    for (name, score) in &self.scoreboard_lines {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(name).size(12.0).color(Color32::WHITE));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(egui::RichText::new(format!("{}", score)).size(12.0).color(Color32::RED));
                            });
                        });
                    }
                });
            });
    }

    /// Ve danh sach nguoi choi Tab List o tren giua man hinh
    fn render_tab_list(&self, ctx: &egui::Context) {
        egui::Area::new(egui::Id::new("hud_tab_list"))
            .anchor(Align2::CENTER_TOP, Vec2::new(0.0, 40.0))
            .show(ctx, |ui| {
                let frame = egui::Frame::none()
                    .fill(Color32::from_rgba_premultiplied(10, 10, 15, 220))
                    .rounding(Rounding::same(6.0))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgba_premultiplied(255, 255, 255, 60)))
                    .inner_margin(12.0);

                frame.show(ui, |ui| {
                    ui.set_min_width(280.0);
                    ui.heading("DANH SACH NGUOI CHOI");
                    ui.separator();

                    for (name, ping) in &self.tab_list_players {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(name).strong().color(Color32::WHITE));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(
                                    egui::RichText::new(format!("{} ms", ping))
                                        .size(12.0)
                                        .color(if *ping < 50 { Color32::GREEN } else { Color32::YELLOW }),
                                );
                            });
                        });
                    }
                });
            });
    }

    fn render_key_box(ui: &mut egui::Ui, text: &str, active: bool, w: f32, h: f32) {
        let (fill, stroke_col, text_col) = if active {
            (Color32::from_rgba_premultiplied(255, 255, 255, 180), Color32::WHITE, Color32::BLACK)
        } else {
            (Color32::from_rgba_premultiplied(0, 0, 0, 120), Color32::from_rgba_premultiplied(255, 255, 255, 50), Color32::WHITE)
        };

        let frame = egui::Frame::none()
            .fill(fill)
            .rounding(Rounding::same(3.0))
            .stroke(Stroke::new(1.0f32, stroke_col));

        frame.show(ui, |ui| {
            ui.set_min_size(Vec2::new(w, h));
            ui.centered_and_justified(|ui| {
                ui.label(egui::RichText::new(text).size(13.0).strong().color(text_col));
            });
        });
    }

    /// Ve Bang cai dat Mod (Right Shift Mod Settings)
    fn render_mod_settings(&mut self, ctx: &egui::Context) {
        egui::Window::new("Lunar Mod Settings (Right Shift)")
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .resizable(false)
            .collapsible(false)
            .min_width(320.0)
            .show(ctx, |ui| {
                ui.heading("Cai Dat Native Rust Mods");
                ui.add_space(8.0);

                ui.checkbox(&mut self.fps_ping_enabled, "Hien thi FPS & Ping");
                ui.checkbox(&mut self.keystrokes_enabled, "Hien thi Keystrokes (WASD)");
                ui.checkbox(&mut self.cps_counter_enabled, "Hien thi LMB / RMB CPS");
                ui.checkbox(&mut self.scoreboard_enabled, "Hien thi Scoreboard");

                ui.add_space(12.0);
                if ui.button("Dong Cai Dat").clicked() {
                    self.screen_mode = ScreenMode::InGame;
                }
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ui_initial_state_and_version_switch() {
        let mut app = CasxUiApp::default();
        assert_eq!(app.screen_mode, ScreenMode::Hub);
        assert_eq!(app.selected_version, GameVersion::V1_12_2);
        assert_eq!(app.selected_hotbar_slot, 0);

        // Kiem tra chuyen doi phien ban
        app.selected_version = GameVersion::V26_2;
        assert_eq!(app.selected_version, GameVersion::V26_2);
    }

    #[test]
    fn test_hotbar_slot_selection_and_chat_messaging() {
        let mut app = CasxUiApp::default();
        app.selected_hotbar_slot = 3;
        assert_eq!(app.selected_hotbar_slot, 3);

        app.chat_messages.push("Hello from unit test".to_string());
        assert!(app.chat_messages.iter().any(|m| m.contains("unit test")));
    }
}
