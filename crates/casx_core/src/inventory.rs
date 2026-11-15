//! Player inventory and server container window management.

use crate::nbt::NbtTag;

/// Item stack in an inventory slot
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

/// Normalized item stack shared across protocol versions (Canonical ItemStack)
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CanonicalItemStack {
    pub id: u32,
    pub count: u8,
    pub damage: u16,
    pub custom_model_data: Option<i32>,
    pub display_name: Option<String>,
    pub lore: Vec<String>,
}

impl Default for CanonicalItemStack {
    fn default() -> Self {
        Self::empty()
    }
}

impl CanonicalItemStack {
    pub const EMPTY: Self = Self {
        id: 0,
        count: 0,
        damage: 0,
        custom_model_data: None,
        display_name: None,
        lore: Vec::new(),
    };

    pub fn empty() -> Self {
        Self::EMPTY
    }

    pub fn new(id: u32, count: u8, damage: u16) -> Self {
        Self {
            id,
            count,
            damage,
            custom_model_data: None,
            display_name: None,
            lore: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.id == 0 || self.count == 0
    }
}

/// Container window interface (Chests, Crafting Table, NPC Menus)
#[derive(Debug, Clone)]
pub struct ContainerWindow {
    pub window_id: u8,
    pub title: String,
    pub slots: Vec<ItemStack>,
    pub cursor_item: ItemStack,
    pub action_number: i16, // Action sequence number for 1.12.2
    pub state_id: i32,      // State ID for 26.2
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

    /// Handles mouse click on slot (Left-click button = 0, Right-click button = 1)
    pub fn click_slot(&mut self, slot_idx: usize, button: u8, mode: u8) -> (i16, i32) {
        let action = self.next_action_number();

        if slot_idx < self.slots.len() && mode == 0 {
            // Mode 0: Normal click
            if button == 0 {
                // Left-click: Swap item with cursor
                let temp = self.slots[slot_idx].clone();
                self.slots[slot_idx] = self.cursor_item.clone();
                self.cursor_item = temp;
            } else if button == 1 {
                // Right-click: Place half or single item
                if self.cursor_item.is_empty() {
                    let count = self.slots[slot_idx].count;
                    let half = count.div_ceil(2);
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
        let mut window = ContainerWindow::new(1, "Server Menu", 54);
        let diamond_sword = ItemStack::new(276, 1, 0);
        window.set_slot(13, diamond_sword.clone());

        assert_eq!(window.get_slot(13), Some(&diamond_sword));
        assert!(window.cursor_item.is_empty());

        // Left-click slot 13 to pick up sword onto cursor
        let (action, _) = window.click_slot(13, 0, 0);
        assert_eq!(action, 1);
        assert_eq!(window.cursor_item, diamond_sword);
        assert!(window.get_slot(13).unwrap().is_empty());

        // Left-click slot 14 to place sword down
        let (action2, _) = window.click_slot(14, 0, 0);
        assert_eq!(action2, 2);
        assert!(window.cursor_item.is_empty());
        assert_eq!(window.get_slot(14).unwrap(), &diamond_sword);
    }
}
