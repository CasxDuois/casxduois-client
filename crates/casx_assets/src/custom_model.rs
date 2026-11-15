//! Bo phan tich CustomModelData va anh xa Texture/Model tuy bien cho Server Items.
//!
//! Cho phep cac may chu MMO/RPG (nhu Hypixel, Aemine) hien thi vu khi, giap va vat pham tuy bien qua the NBT `CustomModelData`.

use std::collections::HashMap;

/// Quy tac override mo hinh dua theo gia tri CustomModelData
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelOverride {
    pub custom_model_data: i32,
    pub model_identifier: String,
    pub texture_id: u16,
}

/// Dinh nghia mo hinh vat pham va danh sach cac override
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemModelDefinition {
    pub item_id: i32,
    pub default_texture_id: u16,
    pub overrides: Vec<ModelOverride>,
}

impl ItemModelDefinition {
    pub fn new(item_id: i32, default_texture_id: u16) -> Self {
        Self {
            item_id,
            default_texture_id,
            overrides: Vec::new(),
        }
    }

    pub fn add_override(&mut self, custom_model_data: i32, identifier: &str, texture_id: u16) {
        self.overrides.push(ModelOverride {
            custom_model_data,
            model_identifier: identifier.to_string(),
            texture_id,
        });
    }

    /// Tim texture phu hop dua vao the CustomModelData
    pub fn resolve_texture(&self, custom_model_data: Option<i32>) -> u16 {
        if let Some(cmd) = custom_model_data {
            for ovr in &self.overrides {
                if ovr.custom_model_data == cmd {
                    return ovr.texture_id;
                }
            }
        }
        self.default_texture_id
    }
}

/// So dang ky tap trung cac mo hinh CustomModelData
#[derive(Debug, Default, Clone)]
pub struct CustomModelRegistry {
    pub models: HashMap<i32, ItemModelDefinition>,
}

impl CustomModelRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Dang ky mot mo hinh vat pham
    pub fn register(&mut self, model: ItemModelDefinition) {
        self.models.insert(model.item_id, model);
    }

    /// Phan giai texture cho mot vat pham
    pub fn resolve_item_texture(&self, item_id: i32, custom_model_data: Option<i32>) -> u16 {
        if let Some(def) = self.models.get(&item_id) {
            def.resolve_texture(custom_model_data)
        } else {
            0 // TEX_MISSING
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_custom_model_data_resolution() {
        let mut registry = CustomModelRegistry::new();

        // Vat pham 276 (Diamond Sword trong 1.12.2) voi texture mac dinh = 50
        let mut sword = ItemModelDefinition::new(276, 50);
        // Dang ky CustomModelData: 1001 -> Dragon Blade (Texture ID 2001)
        sword.add_override(1001, "item/dragon_blade", 2001);
        // Dang ky CustomModelData: 1002 -> Excalibur (Texture ID 2002)
        sword.add_override(1002, "item/excalibur", 2002);

        registry.register(sword);

        // Khong co CustomModelData -> tra ve texture mac dinh
        assert_eq!(registry.resolve_item_texture(276, None), 50);

        // CustomModelData 1001 -> tra ve 2001
        assert_eq!(registry.resolve_item_texture(276, Some(1001)), 2001);

        // CustomModelData 1002 -> tra ve 2002
        assert_eq!(registry.resolve_item_texture(276, Some(1002)), 2002);

        // CustomModelData khong khop -> fallback texture mac dinh
        assert_eq!(registry.resolve_item_texture(276, Some(9999)), 50);

        // Item ID chua dang ky -> tra ve 0
        assert_eq!(registry.resolve_item_texture(999, None), 0);
    }
}
