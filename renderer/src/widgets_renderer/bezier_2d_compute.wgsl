// ====== 設定 / Constants ======
const MAX_ANCHORS: u32 = 16u; // 必要に応じて増やす（ホスト側で anchors.len() <= MAX_ANCHORS を保証）

// Rust 側:
// #[repr(C)]
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
    _padding: u32, // アライメント維持（mat4 push_constants との混同回避用、未使用）
};

// Bindings
@group(0) @binding(0) var<uniform> info: BezierInfo;
@group(0) @binding(1) var<storage, read> anchors: array<vec2<f32>>;
@group(0) @binding(2) var<storage, read_write> vertices: array<vec2<f32>>;
// draw_command_storage (間接描画引数バッファ). 現状この compute では書き込まないが
// BindGroupLayout と整合させるために宣言のみ保持。必要であればここで書く実装に変更可能。
// layout: [ vertex_count, instance_count, first_vertex, first_instance ]
@group(0) @binding(3) var<storage, read_write> draw_command: array<u32>;

// ユーティリティ
fn lerp(a: vec2<f32>, b: vec2<f32>, t: f32) -> vec2<f32> {
    return a + (b - a) * t;
}

fn safe_length(v: vec2<f32>) -> f32 {
    return sqrt(v.x * v.x + v.y * v.y);
}

fn safe_normalize(v: vec2<f32>) -> vec2<f32> {
    let l = safe_length(v);
    if (l > 1e-6) {
        return v / l;
    }
    return vec2<f32>(0.0, 1.0); // フォールバック
}

// de Casteljau による点評価（任意次数：num_anchors <= MAX_ANCHORS を仮定）
fn de_casteljau_point(num_anchors: u32, t: f32) -> vec2<f32> {
    var tmp: array<vec2<f32>, MAX_ANCHORS>;
    // load anchors
    for (var i: u32 = 0u; i < num_anchors; i = i + 1u) {
        tmp[i] = anchors[i];
    }
    // 補間反復
    var k: u32 = 1u;
    loop {
        if (k > num_anchors - 1u) { break; }
        let last = num_anchors - k - 1u;
        for (var i: u32 = 0u; i <= last; i = i + 1u) {
            tmp[i] = lerp(tmp[i], tmp[i + 1u], t);
        }
        k = k + 1u;
    }
    return tmp[0];
}

// de Casteljau を差分制御点に適用して接ベクトルを求める
fn bezier_tangent(num_anchors: u32, t: f32) -> vec2<f32> {
    if (num_anchors <= 1u) { return vec2<f32>(0.0, 0.0); }
    var diff: array<vec2<f32>, MAX_ANCHORS>;
    let deg: f32 = f32(num_anchors - 1u);

    for (var i: u32 = 0u; i < num_anchors - 1u; i = i + 1u) {
        diff[i] = anchors[i + 1u] - anchors[i];
    }

    var tmp: array<vec2<f32>, MAX_ANCHORS>;
    for (var i: u32 = 0u; i < num_anchors - 1u; i = i + 1u) {
        tmp[i] = diff[i];
    }

    var k: u32 = 1u;
    loop {
        if (k > (num_anchors - 2u)) { break; }
        let last = (num_anchors - 1u) - k - 1u;
        for (var i: u32 = 0u; i <= last; i = i + 1u) {
            tmp[i] = lerp(tmp[i], tmp[i + 1u], t);
        }
        k = k + 1u;
    }

    return tmp[0] * deg;
}

// 90度左回転で法線
fn left_normal_from_tangent(t: vec2<f32>) -> vec2<f32> {
    let n = safe_normalize(t);
    return vec2<f32>(-n.y, n.x);
}

// 各 invocation が 1 サンプル (0..div) を担当
// 現実装: 交互に左右へオフセットした単一列(L,R,L,R...)を生成
//  - vertices[0] / vertices[div+1] は別途 (拡張/キャップ用途) のためにリザーブ
//  - write index = 1 + sampleIndex
// 将来的に TriangleStrip 用に (L0,R0,L1,R1,...) ペア展開へ変えたい場合は
// ホスト側バッファ確保 / draw_command 生成方法も合わせて変更すること。
@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let div = info.div;
    if (div == 0u) { return; }

    let sampleIndex = global_id.x;
    if (sampleIndex > div) { return; }

    let na = info.num_anchors;
    if (na < 2u || na > MAX_ANCHORS) {
        return;
    }

    let t = f32(sampleIndex) / f32(div);

    let p = de_casteljau_point(na, t);
    let tangent = bezier_tangent(na, t);

    var n = left_normal_from_tangent(tangent);
    if (n.x == 0.0 && n.y == 0.0) {
        n = vec2<f32>(0.0, 1.0);
    }
    let halfw = 0.5 * info.width;

    // 偶奇で左右オフセットを切り替える現在方式
    var offsetDir: f32 = 1.0;
    if ((sampleIndex & 1u) == 1u) {
        offsetDir = -1.0;
    }

    let outPos = p + n * (halfw * offsetDir);

    // 書き込み index (両端 1 つずつ予備確保を前提)
    let writeIndex = 1u + sampleIndex;
    vertices[writeIndex] = outPos;

    // 必要ならここで draw_command を書き込む (現状は command pass が担当)
    // 例:
    // if (sampleIndex == 0u) {
    //     draw_command[0] = (div + 1u) + 2u; // 全頂点数 (両端+2を含む)
    //     draw_command[1] = 1u;              // instance_count
    //     draw_command[2] = 0u;              // first_vertex
    //     draw_command[3] = 0u;              // first_instance
    // }
}
