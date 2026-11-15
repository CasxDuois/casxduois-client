//! Indirect draw buffer manager and multi-section draw call batching.

use bytemuck::{Pod, Zeroable};

/// Standard GPU `DrawIndexedIndirectArgs` layout (20 bytes).
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

/// Indirect draw command batch collector for chunk sections
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

    /// Clears the batch buffer before collecting sections for a new frame
    pub fn clear(&mut self) {
        self.commands.clear();
        self.total_indices = 0;
        self.total_quads = 0;
    }

    /// Adds a chunk section into the indirect batch
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

    /// Returns raw byte slice to upload directly to `wgpu::Buffer` with `BufferUsages::INDIRECT`
    pub fn as_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(&self.commands)
    }

    /// Total number of batched draw commands
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
        // Section 3 empty: 0 quads (must skip)
        batch.add_section(0);

        assert_eq!(batch.command_count(), 2);
        assert_eq!(batch.total_quads, 35);
        assert_eq!(batch.total_indices, 35 * 6);

        // Verify command 1
        assert_eq!(batch.commands[0].index_count, 60);
        assert_eq!(batch.commands[0].first_index, 0);
        assert_eq!(batch.commands[0].base_vertex, 0);

        // Verify command 2
        assert_eq!(batch.commands[1].index_count, 150);
        assert_eq!(batch.commands[1].first_index, 60);
        assert_eq!(batch.commands[1].base_vertex, 40);

        // Verify GPU upload buffer byte length
        let bytes = batch.as_bytes();
        assert_eq!(bytes.len(), 40); // 2 commands * 20 bytes
    }
}
