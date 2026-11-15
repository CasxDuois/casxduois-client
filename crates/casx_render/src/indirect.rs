//! Bo dem lenh ve gian tiep (Indirect Draw Buffer) va gom cum Draw Call da Section.
//!
//! Giup giam overhead tu hang tram Draw Call moi frame xuong mot lenh goi duy nhat:
//! `multi_draw_indexed_indirect`.

use bytemuck::{Pod, Zeroable};

/// Cau truc lenh ve gián tiếp `DrawIndexedIndirectArgs` chuan GPU (20 bytes)
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Pod, Zeroable)]
pub struct DrawIndexedIndirectArgs {
    pub index_count: u32,
    pub instance_count: u32,
    pub first_index: u32,
    pub base_vertex: i32,
    pub first_instance: u32,
}

impl DrawIndexedIndirectArgs {
    pub fn new(index_count: u32, first_index: u32, base_vertex: i32) -> Self {
        Self {
            index_count,
            instance_count: 1,
            first_index,
            base_vertex,
            first_instance: 0,
        }
    }
}

/// Bo gom cum lenh ve gian tiep cho cac Section Chunk
#[derive(Debug, Default, Clone)]
pub struct IndirectBatchManager {
    pub commands: Vec<DrawIndexedIndirectArgs>,
    pub total_indices: u32,
    pub total_quads: u32,
}

impl IndirectBatchManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Xoa sach bo dem truoc khi thu thap Section cho khung hinh moi
    pub fn clear(&mut self) {
        self.commands.clear();
        self.total_indices = 0;
        self.total_quads = 0;
    }

    /// Them mot Section vao lo ve gian tiep
    pub fn add_section(&mut self, quad_count: u32) {
        if quad_count == 0 {
            return;
        }

        let index_count = quad_count * 6;
        let first_index = self.total_indices;
        let base_vertex = (self.total_quads * 4) as i32;

        self.commands.push(DrawIndexedIndirectArgs::new(
            index_count,
            first_index,
            base_vertex,
        ));

        self.total_indices += index_count;
        self.total_quads += quad_count;
    }

    /// Lay du lieu byte tho de nap truc tiep vao `wgpu::Buffer` voi `BufferUsages::INDIRECT`
    pub fn as_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(&self.commands)
    }

    /// So luong lenh ve duoc gom
    pub fn command_count(&self) -> usize {
        self.commands.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_indirect_draw_args_size_and_layout() {
        assert_eq!(std::mem::size_of::<DrawIndexedIndirectArgs>(), 20);

        let mut batch = IndirectBatchManager::new();
        // Section 1: 10 quads
        batch.add_section(10);
        // Section 2: 25 quads
        batch.add_section(25);
        // Section 3 trong: 0 quads (phai bo qua)
        batch.add_section(0);

        assert_eq!(batch.command_count(), 2);
        assert_eq!(batch.total_quads, 35);
        assert_eq!(batch.total_indices, 35 * 6);

        // Kiem tra lenh 1
        assert_eq!(batch.commands[0].index_count, 60);
        assert_eq!(batch.commands[0].first_index, 0);
        assert_eq!(batch.commands[0].base_vertex, 0);

        // Kiem tra lenh 2
        assert_eq!(batch.commands[1].index_count, 150);
        assert_eq!(batch.commands[1].first_index, 60);
        assert_eq!(batch.commands[1].base_vertex, 40);

        // Kiem tra kich thuoc mang byte nap xuong GPU
        let bytes = batch.as_bytes();
        assert_eq!(bytes.len(), 40); // 2 commands * 20 bytes
    }
}
