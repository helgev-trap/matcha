// Bezier 2D Command Shader
// 目的: compute/geometry 生成後に間接描画コマンド (DrawIndirectArgs 相当) を書き込む。
// layout (wgpu::util::DrawIndirectArgs):
//   struct DrawIndirectArgs {
//       vertex_count: u32;
//       instance_count: u32;
//       first_vertex: u32;
//       first_instance: u32;
//   }
//
// 現行ジオメトリ生成方式:
//   compute シェーダで vertices[1 .. div] (交互オフセット) を埋め、
//   vertices[0], vertices[div+1] を予約 (キャップ/拡張用) として +2 余分に確保。
//   → 総頂点数 = (div + 1) + 2
//
// NOTE:
//   将来 (L0,R0,L1,R1,...) ペア展開方式へ変更する場合は vertex_count の式を
//   (div + 1) * 2 に差し替え、compute 側頂点レイアウトも変更すること。
//
// Rust 側 BezierInfo とフィールド整合 (#repr(C)):
// struct BezierInfo {
//     num_anchors: u32,
//     div: u32,
//     width: f32,
//     _padding: u32,
// }
struct BezierInfo {
    num_anchors: u32,
    div: u32,
    width: f32,
    _padding: u32,
};

@group(0) @binding(0) var<uniform> info: BezierInfo;
@group(0) @binding(1) var<storage, read> anchors: array<vec2<f32>>;        // (未使用だが BindGroup 整合のため残す)
@group(0) @binding(2) var<storage, read> vertices: array<vec2<f32>>;       // (頂点数計算を将来ここで検証するなら参照可)
@group(0) @binding(3) var<storage, read_write> draw_command: array<u32>;   // 4 要素: vertex_count, instance_count, first_vertex, first_instance

@compute @workgroup_size(1)
fn main() {
    // 分割数 0 の場合は何も描画しない
    if (info.div == 0u) {
        draw_command[0] = 0u;
        draw_command[1] = 0u;
        draw_command[2] = 0u;
        draw_command[3] = 0u;
        return;
    }

    // 現在の交互オフセット配置方式に合わせた頂点数:
    // sample 点: (div + 1)
    // 先頭/末尾予約: +2
    let vertex_count = (info.div + 1u) + 2u;

    draw_command[0] = vertex_count; // vertex_count
    draw_command[1] = 1u;           // instance_count
    draw_command[2] = 0u;           // first_vertex
    draw_command[3] = 0u;           // first_instance
}
