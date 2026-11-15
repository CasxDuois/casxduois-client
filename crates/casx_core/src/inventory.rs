//! Quan ly kho do nguoi choi va Container Window (Menu Ruong/Shop Server).

use crate::nbt::NbtTag;

/// Vat pham trong mot o kho do
#[derive(Debug, Clone, PartialEq)]
pub struct ItemStack {
    pub item_id: i32,
    pub count: u8,
    pub damage_or_meta: i16,
    pub nbt: Option<NbtTag>,
}

impl Default for ItemStack {
    fn default() -> Self {
        Self::empty()
    }
}

impl ItemStack {
    pub const EMPTY: Self = Self {
        item_id: -1,
        count: 0,
        damage_or_meta: 0,
        nbt: None,
    };

    pub fn empty() -> Self {
        Self::EMPTY
    }

    pub fn new(item_id: i32, count: u8, damage_or_meta: i16) -> Self {
        Self {
            item_id,
            count,
            damage_or_meta,
            nbt: None,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.item_id <= 0 || self.count == 0
    }
}

/// Giao dien cua so Container cua Server (Ruong, Ban che tao, Menu NPC)
#[derive(Debug, Clone)]
pub struct ContainerWindow {
    pub window_id: u8,
    pub title: String,
    pub slots: Vec<ItemStack>,
    pub cursor_item: ItemStack,
    pub action_number: i16, // So thu tu hanh dong cho 1.12.2
    pub state_id: i32,      // Dinh danh trang thai cho 26.2
}

impl ContainerWindow {
    pub fn new(window_id: u8, title: &str, slot_count: usize) -> Self {
        let mut slots = Vec::with_capacity(slot_count);
        for _ in 0..slot_count {
            slots.push(ItemStack::empty());
        }
        Self {
            window_id,
            title: title.to_string(),
            slots,
            cursor_item: ItemStack::empty(),
            action_number: 1,
            state_id: 0,
        }
    }

    pub fn get_slot(&self, idx: usize) -> Option<&ItemStack> {
        self.slots.get(idx)
    }

    pub fn set_slot(&mut self, idx: usize, item: ItemStack) {
        if idx < self.slots.len() {
            self.slots[idx] = item;
        }
    }

    pub fn next_action_number(&mut self) -> i16 {
        let current = self.action_number;
        self.action_number = self.action_number.wrapping_add(1);
        if self.action_number == 0 {
            self.action_number = 1;
        }
        current
    }

    /// Xu ly click chuot vao mot o vat pham (Left-click button = 0, Right-click button = 1)
    pub fn click_slot(&mut self, slot_idx: usize, button: u8, mode: u8) -> (i16, i32) {
        let action = self.next_action_number();

        if slot_idx < self.slots.len() && mode == 0 {
            // Mode 0: Click binh thuong
            if button == 0 {
                // Chuot trai: Trac doi vat pham voi chuot
                let temp = self.slots[slot_idx].clone();
                self.slots[slot_idx] = self.cursor_item.clone();
                self.cursor_item = temp;
            } else if button == 1 {
                // Chuot phai: Dat 1 nua hoac dat 1 vat pham xuong
                if self.cursor_item.is_empty() {
                    let count = self.slots[slot_idx].count;
                    let half = (count + 1) / 2;
                    let remaining = count - half;
                    self.cursor_item = self.slots[slot_idx].clone();
                    self.cursor_item.count = half;
                    self.slots[slot_idx].count = remaining;
                    if remaining == 0 {
                        self.slots[slot_idx] = ItemStack::empty();
                    }
                } else if self.slots[slot_idx].is_empty() {
                    self.slots[slot_idx] = self.cursor_item.clone();
                    self.slots[slot_idx].count = 1;
                    self.cursor_item.count = self.cursor_item.count.saturating_sub(1);
                    if self.cursor_item.count == 0 {
                        self.cursor_item = ItemStack::empty();
                    }
                }
            }
        }

        (action, self.state_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_container_click_and_action_numbers() {
        let mut window = ContainerWindow::new(1, "Menu Server", 54);
        let diamond_sword = ItemStack::new(276, 1, 0);
        window.set_slot(13, diamond_sword.clone());

        assert_eq!(window.get_slot(13), Some(&diamond_sword));
        assert!(window.cursor_item.is_empty());

        // Bấm chuột trái vào ô 13 để nhặt kiếm lên con trỏ
        let (action, _) = window.click_slot(13, 0, 0);
        assert_eq!(action, 1);
        assert_eq!(window.cursor_item, diamond_sword);
        assert!(window.get_slot(13).unwrap().is_empty());

        // Bấm chuột trái vào ô 14 để đặt kiếm xuống
        let (action2, _) = window.click_slot(14, 0, 0);
        assert_eq!(action2, 2);
        assert!(window.cursor_item.is_empty());
        assert_eq!(window.get_slot(14).unwrap(), &diamond_sword);
    }
}
