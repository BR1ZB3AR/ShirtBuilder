// Blit the rendered 3D image (premultiplied, sRGB-encoded) into egui's render pass.
@group(0) @binding(0) var blit_tex: texture_2d<f32>;
@group(0) @binding(1) var blit_samp: sampler;

struct BOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_blit(@builtin(vertex_index) i: u32) -> BOut {
    var o: BOut;
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    o.clip = vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
    o.uv = vec2<f32>(p.x, 1.0 - p.y);
    return o;
}

@fragment
fn fs_blit(in: BOut) -> @location(0) vec4<f32> {
    return textureSample(blit_tex, blit_samp, in.uv);
}

@fragment
fn fs_blit_srgb_target(in: BOut) -> @location(0) vec4<f32> {
    // the target encodes to sRGB itself: hand it linear (premultiplied) values
    let c = textureSample(blit_tex, blit_samp, in.uv);
    if c.a <= 0.0 {
        return vec4<f32>(0.0);
    }
    return vec4<f32>(srgb_to_linear(c.rgb / c.a) * c.a, c.a);
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    return select(pow((c + 0.055) / 1.055, vec3<f32>(2.4)), c / 12.92, c <= vec3<f32>(0.04045));
}
