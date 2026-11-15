//! Immediate-mode UI overlay for launcher viewport and in-game HUD using egui.

use casx_core::GameVersion;
use egui::{Align2, Color32, Pos2, Rounding, Stroke, Vec2};

/// Player account authentication type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountType {
    Offline,
    ElyBy,
    Microsoft,
}

/// Active UI screen mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenMode {
    Hub,         // Launcher Viewport
    InGame,      // 3D In-Game HUD
    ModSettings, // Settings overlay (Right Shift)
}

/// User interface actions dispatched to the client engine.
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

/// Simplified item representation for HUD hotbar.
#[derive(Debug, Clone)]
pub struct HotbarSlot {
    pub display_name: String,
    pub count: u8,
}

/// Unified egui user interface state.
pub struct CasxUiApp {
    pub screen_mode: ScreenMode,
    pub account_type: AccountType,
    pub username: String,
    pub server_address: String,
    pub selected_version: GameVersion,

    // In-game HUD state
    pub fps_ping_enabled: bool,
    pub keystrokes_enabled: bool,
    pub cps_counter_enabled: bool,
    pub scoreboard_enabled: bool,

    // Keystroke inputs
    pub key_w: bool,
    pub key_a: bool,
    pub key_s: bool,
    pub key_d: bool,
    pub key_space: bool,

    // Hotbar state (9 slots)
    pub hotbar_slots: [Option<HotbarSlot>; 9],
    pub selected_hotbar_slot: usize,

    // Chat state
    pub chat_messages: Vec<String>,
    pub chat_input: String,
    pub is_chat_open: bool,

    // Scoreboard state
    pub scoreboard_title: String,
    pub scoreboard_lines: Vec<(String, i32)>,

    // Tab list
    pub is_tab_held: bool,
    pub tab_list_players: Vec<(String, u32)>,
}

impl Default for CasxUiApp {
    fn default() -> Self {
        let mut slots: [Option<HotbarSlot>; 9] = Default::default();
        slots[0] = Some(HotbarSlot {
            display_name: "Diamond Sword".to_string(),
            count: 1,
        });
        slots[1] = Some(HotbarSlot {
            display_name: "Golden Apple".to_string(),
            count: 64,
        });
        slots[2] = Some(HotbarSlot {
            display_name: "Bow".to_string(),
            count: 1,
        });

        Self {
            screen_mode: ScreenMode::Hub,
            account_type: AccountType::Offline,
            username: "CasxPlayer".to_string(),
            server_address: "127.0.0.1:25565".to_string(),
            selected_version: GameVersion::V1_12_2,

            fps_ping_enabled: true,
            keystrokes_enabled: true,
            cps_counter_enabled: true,
            scoreboard_enabled: true,

            key_w: false,
            key_a: false,
            key_s: false,
            key_d: false,
            key_space: false,

            hotbar_slots: slots,
            selected_hotbar_slot: 0,

            chat_messages: vec!["[System] Welcome to CasxDuois Client Engine!".to_string()],
            chat_input: String::new(),
            is_chat_open: false,

            scoreboard_title: "CasxDuois Server".to_string(),
            scoreboard_lines: vec![
                ("Online Players: 1".to_string(), 3),
                ("Ping: 12ms".to_string(), 2),
                ("casxduois.io".to_string(), 1),
            ],

            is_tab_held: false,
            tab_list_players: vec![("CasxPlayer".to_string(), 12)],
        }
    }
}

impl CasxUiApp {
    /// Renders current UI overlays onto egui Context and returns engine actions.
    pub fn update(
        &mut self,
        ctx: &egui::Context,
        in_game: bool,
        fps: f64,
        ping_ms: u32,
    ) -> UiAction {
        let mut action = UiAction::None;

        if !in_game {
            action = self.render_hub(ctx);
        } else {
            self.render_in_game_hud(ctx, fps, ping_ms);

            if let Some(act) = self.render_hotbar(ctx) {
                action = act;
            }

            if let Some(act) = self.render_chat_panel(ctx) {
                action = act;
            }

            if self.scoreboard_enabled {
                self.render_scoreboard(ctx);
            }

            if self.is_tab_held {
                self.render_tab_list(ctx);
            }

            if self.screen_mode == ScreenMode::ModSettings {
                self.render_mod_settings(ctx);
            }
        }

        action
    }

    /// Renders launcher viewport and version selector.
    fn render_hub(&mut self, ctx: &egui::Context) -> UiAction {
        let mut action = UiAction::None;

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
                        egui::RichText::new(
                            "High-Performance Rust-Native Multiplayer Minecraft Client",
                        )
                        .size(16.0)
                        .color(Color32::from_rgb(120, 140, 180)),
                    );

                    ui.add_space(40.0);
                    let version_label = match self.selected_version {
                        GameVersion::V1_12_2 => "Selected: Minecraft 1.12.2 (v340)",
                        GameVersion::V26_2 => "Selected: Minecraft 26.2 (v776)",
                    };
                    ui.label(
                        egui::RichText::new(version_label)
                            .size(15.0)
                            .color(Color32::from_rgb(100, 220, 160)),
                    );
                });
            });

        egui::TopBottomPanel::bottom("launcher_control_bar")
            .min_height(70.0)
            .frame(egui::Frame::none().fill(Color32::from_rgb(22, 27, 38)))
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.add_space(20.0);

                    ui.label(egui::RichText::new("Account:").color(Color32::WHITE));
                    ui.selectable_value(&mut self.account_type, AccountType::Offline, "Offline");
                    ui.selectable_value(&mut self.account_type, AccountType::ElyBy, "Ely.by");
                    ui.selectable_value(
                        &mut self.account_type,
                        AccountType::Microsoft,
                        "Microsoft",
                    );

                    ui.add_space(15.0);
                    ui.label(egui::RichText::new("Username:").color(Color32::WHITE));
                    ui.add(egui::TextEdit::singleline(&mut self.username).desired_width(110.0));

                    ui.add_space(15.0);
                    let btn_ver_text = match self.selected_version {
                        GameVersion::V1_12_2 => "[1.12.2]",
                        GameVersion::V26_2 => "[26.2]",
                    };
                    if ui
                        .button(
                            egui::RichText::new(btn_ver_text)
                                .strong()
                                .color(Color32::YELLOW),
                        )
                        .clicked()
                    {
                        self.selected_version = match self.selected_version {
                            GameVersion::V1_12_2 => GameVersion::V26_2,
                            GameVersion::V26_2 => GameVersion::V1_12_2,
                        };
                        action = UiAction::SwitchVersion(self.selected_version);
                    }

                    ui.add_space(15.0);
                    ui.label(egui::RichText::new("Server:").color(Color32::WHITE));
                    ui.add(
                        egui::TextEdit::singleline(&mut self.server_address).desired_width(160.0),
                    );

                    ui.add_space(25.0);
                    let connect_btn = egui::Button::new(
                        egui::RichText::new("CONNECT")
                            .size(16.0)
                            .strong()
                            .color(Color32::BLACK),
                    )
                    .fill(Color32::from_rgb(0, 220, 130))
                    .min_size(Vec2::new(170.0, 42.0));

                    if ui.add(connect_btn).clicked() {
                        let parts: Vec<&str> = self.server_address.split(':').collect();
                        let host = parts.first().unwrap_or(&"127.0.0.1").to_string();
                        let port = parts
                            .get(1)
                            .and_then(|p| p.parse::<u16>().ok())
                            .unwrap_or(25565);

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

    /// Renders in-game HUD widgets.
    fn render_in_game_hud(&self, ctx: &egui::Context, fps: f64, ping_ms: u32) {
        if self.fps_ping_enabled {
            egui::Area::new(egui::Id::new("hud_fps_ping"))
                .fixed_pos(Pos2::new(12.0, 12.0))
                .show(ctx, |ui| {
                    let frame = egui::Frame::none()
                        .fill(Color32::from_rgba_premultiplied(0, 0, 0, 150))
                        .rounding(Rounding::same(4.0))
                        .stroke(Stroke::new(
                            1.0f32,
                            Color32::from_rgba_premultiplied(255, 255, 255, 40),
                        ))
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

    /// Renders 9-slot hotbar at the bottom of the screen.
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
                                .stroke(Stroke::new(
                                    if is_selected { 2.0_f32 } else { 1.0_f32 },
                                    border_col,
                                ))
                                .rounding(Rounding::same(4.0))
                                .inner_margin(4.0);

                            frame.show(ui, |ui| {
                                ui.set_min_size(Vec2::new(42.0, 42.0));
                                if let Some(slot) = &self.hotbar_slots[slot_idx] {
                                    let btn_txt = format!(
                                        "{}\nx{}",
                                        slot.display_name.chars().next().unwrap_or('?'),
                                        slot.count
                                    );
                                    if ui.button(egui::RichText::new(btn_txt).size(11.0)).clicked()
                                    {
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

    /// Renders chat panel in the bottom-left of the viewport.
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
                    let start_idx = self.chat_messages.len().saturating_sub(6);
                    for msg in &self.chat_messages[start_idx..] {
                        ui.label(egui::RichText::new(msg).size(13.0).color(Color32::WHITE));
                    }

                    if self.is_chat_open {
                        ui.separator();
                        let response = ui.text_edit_singleline(&mut self.chat_input);
                        if response.lost_focus()
                            && ctx.input(|i| i.key_pressed(egui::Key::Enter))
                            && !self.chat_input.trim().is_empty()
                        {
                            let msg = self.chat_input.clone();
                            self.chat_messages.push(format!("[You] {}", msg));
                            self.chat_input.clear();
                            self.is_chat_open = false;
                            send_action = Some(UiAction::SendChatMessage(msg));
                        }
                    }
                });
            });

        send_action
    }

    /// Renders scoreboard on the right side of the screen.
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
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(
                                        egui::RichText::new(format!("{}", score))
                                            .size(12.0)
                                            .color(Color32::RED),
                                    );
                                },
                            );
                        });
                    }
                });
            });
    }

    /// Renders player tab list overlay in top center.
    fn render_tab_list(&self, ctx: &egui::Context) {
        egui::Area::new(egui::Id::new("hud_tab_list"))
            .anchor(Align2::CENTER_TOP, Vec2::new(0.0, 40.0))
            .show(ctx, |ui| {
                let frame = egui::Frame::none()
                    .fill(Color32::from_rgba_premultiplied(10, 10, 15, 220))
                    .rounding(Rounding::same(6.0))
                    .stroke(Stroke::new(
                        1.0_f32,
                        Color32::from_rgba_premultiplied(255, 255, 255, 60),
                    ))
                    .inner_margin(12.0);

                frame.show(ui, |ui| {
                    ui.set_min_width(280.0);
                    ui.heading("ONLINE PLAYERS");
                    ui.separator();

                    for (name, ping) in &self.tab_list_players {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(name).strong().color(Color32::WHITE));
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(
                                        egui::RichText::new(format!("{} ms", ping))
                                            .size(12.0)
                                            .color(if *ping < 50 {
                                                Color32::GREEN
                                            } else {
                                                Color32::YELLOW
                                            }),
                                    );
                                },
                            );
                        });
                    }
                });
            });
    }

    fn render_key_box(ui: &mut egui::Ui, text: &str, active: bool, w: f32, h: f32) {
        let (fill, stroke_col, text_col) = if active {
            (
                Color32::from_rgba_premultiplied(255, 255, 255, 180),
                Color32::WHITE,
                Color32::BLACK,
            )
        } else {
            (
                Color32::from_rgba_premultiplied(0, 0, 0, 120),
                Color32::from_rgba_premultiplied(255, 255, 255, 50),
                Color32::WHITE,
            )
        };

        let frame = egui::Frame::none()
            .fill(fill)
            .rounding(Rounding::same(3.0))
            .stroke(Stroke::new(1.0f32, stroke_col));

        frame.show(ui, |ui| {
            ui.set_min_size(Vec2::new(w, h));
            ui.centered_and_justified(|ui| {
                ui.label(
                    egui::RichText::new(text)
                        .size(13.0)
                        .strong()
                        .color(text_col),
                );
            });
        });
    }

    /// Renders settings overlay (Right Shift).
    fn render_mod_settings(&mut self, ctx: &egui::Context) {
        egui::Window::new("Client Settings (Right Shift)")
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .resizable(false)
            .collapsible(false)
            .min_width(320.0)
            .show(ctx, |ui| {
                ui.heading("Native Client Settings");
                ui.add_space(8.0);

                ui.checkbox(&mut self.fps_ping_enabled, "Show FPS & Ping");
                ui.checkbox(&mut self.keystrokes_enabled, "Show Keystrokes (WASD)");
                ui.checkbox(&mut self.cps_counter_enabled, "Show LMB / RMB CPS");
                ui.checkbox(&mut self.scoreboard_enabled, "Show Scoreboard");

                ui.add_space(12.0);
                if ui.button("Close Settings").clicked() {
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

        app.selected_version = GameVersion::V26_2;
        assert_eq!(app.selected_version, GameVersion::V26_2);
    }

    #[test]
    fn test_hotbar_slot_selection_and_chat_messaging() {
        let mut app = CasxUiApp {
            selected_hotbar_slot: 3,
            ..Default::default()
        };
        assert_eq!(app.selected_hotbar_slot, 3);

        app.chat_messages.push("Hello from unit test".to_string());
        assert!(app.chat_messages.iter().any(|m| m.contains("unit test")));
    }
}
