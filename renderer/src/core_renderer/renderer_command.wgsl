struct DrawIndirectCommand {
    vertex_count: u32,
    instance_count: u32,
    first_vertex: u32,
    first_instance: u32,
};

@group(0) @binding(3) var<storage, read_write> visible_instance_count: atomic<u32>;
@group(0) @binding(4) var<storage, read_write> indirect_command: DrawIndirectCommand;

@compute @workgroup_size(1)
fn command_main() {
    let instance_count = atomicLoad(&visible_instance_count);
    indirect_command.vertex_count = 4; // triangle strip with 4 vertices
    indirect_command.instance_count = instance_count;
    indirect_command.first_vertex = 0;
    indirect_command.first_instance = 0;
}
