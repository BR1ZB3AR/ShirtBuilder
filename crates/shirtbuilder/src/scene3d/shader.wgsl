// ShirtBuilder 3D preview — fabric shading modelled on the web app's three.js scene
// (MeshPhysicalMaterial: Lambert diffuse, GGX specular, Charlie sheen; hemisphere + key/fill/rim
// directional lights, a spot light projecting a window-light cookie, a soft room environment,
// PCF soft shadows, Khronos PBR Neutral tone mapping, sRGB output, then the fabric's
// brightness / contrast like the web app's post step).

struct Globals {
    view_proj: mat4x4<f32>,
    key_vp: mat4x4<f32>,
    gobo_vp: mat4x4<f32>,
    cam_pos: vec4<f32>,
    key: vec4<f32>,      // xyz: towards the light, w: intensity
    fill: vec4<f32>,
    rim: vec4<f32>,
    sky: vec4<f32>,      // rgb sky colour × intensity
    ground: vec4<f32>,   // rgb ground colour × intensity
    gobo_pos: vec4<f32>, // xyz position, w intensity
    gobo_dir: vec4<f32>, // xyz light → target, w cos(cone)
    gobo_col: vec4<f32>, // rgb, w cos(inner cone)
    env: vec4<f32>,      // x environment intensity, y exposure, z brightness, w contrast
    flags: vec4<f32>,    // x key shadows, y gobo on, z key shadow texel, w gobo shadow texel
};

struct Material {
    tint_out: vec4<f32>,    // rgb, w: kind (0 cloth both sides, 1 cloth outside, 2 cloth inside, 3 solid)
    tint_in: vec4<f32>,
    emissive_in: vec4<f32>,
    solid: vec4<f32>,       // linear base colour of solid objects
    pbr: vec4<f32>,         // roughness, sheen, sheen roughness, clearcoat
    sheen_col: vec4<f32>,
    nmap: vec4<f32>,        // repeat xy, normal scale outside, inside
    misc: vec4<f32>,        // x use vertex AO, y alpha test, z receives shadows, w knit tiles/cm on the surface (0: by UV)
};

@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var shadow_key: texture_depth_2d;
@group(0) @binding(2) var shadow_gobo: texture_depth_2d;
@group(0) @binding(3) var shadow_samp: sampler_comparison;
@group(0) @binding(4) var cookie: texture_2d<f32>;
@group(0) @binding(5) var lin_clamp: sampler;

@group(1) @binding(0) var<uniform> m: Material;
@group(1) @binding(1) var atlas_out: texture_2d<f32>;
@group(1) @binding(2) var atlas_in: texture_2d<f32>;
@group(1) @binding(3) var atlas_samp: sampler;
@group(1) @binding(4) var nrm_tex: texture_2d<f32>;
@group(1) @binding(5) var nrm_samp: sampler;

struct VIn {
    @location(0) pos: vec3<f32>,
    @location(1) nrm: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) ao: f32,
};

struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) nrm: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) ao: f32,
};

@vertex
fn vs_main(v: VIn) -> VOut {
    var o: VOut;
    o.clip = g.view_proj * vec4<f32>(v.pos, 1.0);
    o.world = v.pos;
    o.nrm = v.nrm;
    o.uv = v.uv;
    o.ao = v.ao;
    return o;
}

const PI: f32 = 3.14159265;

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    return select(pow((c + 0.055) / 1.055, vec3<f32>(2.4)), c / 12.92, c <= vec3<f32>(0.04045));
}
fn linear_to_srgb(c: vec3<f32>) -> vec3<f32> {
    return select(1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055, c * 12.92, c <= vec3<f32>(0.0031308));
}

fn neutral_tonemap(c0: vec3<f32>) -> vec3<f32> {
    let start = 0.8 - 0.04;
    let desat = 0.15;
    var c = c0;
    let x = min(c.r, min(c.g, c.b));
    let off = select(0.04, x - 6.25 * x * x, x < 0.08);
    c = c - off;
    let peak = max(c.r, max(c.g, c.b));
    if peak < start {
        return c;
    }
    let d = 1.0 - start;
    let new_peak = 1.0 - d * d / (peak + d - start);
    c = c * (new_peak / peak);
    let gg = 1.0 - 1.0 / (desat * (peak - new_peak) + 1.0);
    return mix(c, vec3<f32>(new_peak), gg);
}

fn d_ggx(alpha: f32, nh: f32) -> f32 {
    let a2 = alpha * alpha;
    let den = nh * nh * (a2 - 1.0) + 1.0;
    return a2 / (PI * den * den);
}
fn v_smith(alpha: f32, nl: f32, nv: f32) -> f32 {
    let a2 = alpha * alpha;
    let gv = nl * sqrt(a2 + (1.0 - a2) * nv * nv);
    let gl = nv * sqrt(a2 + (1.0 - a2) * nl * nl);
    return 0.5 / max(gv + gl, 1e-6);
}
fn f_schlick(f0: vec3<f32>, vh: f32) -> vec3<f32> {
    let k = exp2((-5.55473 * vh - 6.98316) * vh);
    return f0 * (1.0 - k) + vec3<f32>(k);
}
fn d_charlie(rough: f32, nh: f32) -> f32 {
    let alpha = rough * rough;
    let inv = 1.0 / alpha;
    let sin2 = max(1.0 - nh * nh, 0.0078125);
    return (2.0 + inv) * pow(sin2, inv * 0.5) / (2.0 * PI);
}
fn v_neubelt(nv: f32, nl: f32) -> f32 {
    return clamp(1.0 / (4.0 * (nl + nv - nl * nv)), 0.0, 1.0);
}
fn ibl_sheen(nv: f32, r: f32) -> f32 {
    let r2 = r * r;
    let a = select(-8.48 * r2 + 14.3 * r - 9.95, -339.2 * r2 + 161.4 * r - 25.9, r < 0.25);
    let b = select(1.97 * r2 - 3.27 * r + 0.72, 44.0 * r2 - 23.7 * r + 3.26, r < 0.25);
    let dg = exp(a * nv + b) + select(0.1 * (r - 0.25), 0.0, r < 0.25);
    return clamp(dg / PI, 0.0, 1.0);
}
fn dfg(nv: f32, r: f32) -> vec2<f32> {
    let c0 = vec4<f32>(-1.0, -0.0275, -0.572, 0.022);
    let c1 = vec4<f32>(1.0, 0.0425, 1.04, -0.04);
    let rr = r * c0 + c1;
    let a004 = min(rr.x * rr.x, exp2(-9.28 * nv)) * rr.x + rr.y;
    return vec2<f32>(-1.04, 1.04) * a004 + rr.zw;
}

// a soft studio room (stands in for three.js' RoomEnvironment): bright ceiling panels, grey walls
fn env_radiance(d: vec3<f32>) -> vec3<f32> {
    let up = clamp(d.y * 0.5 + 0.5, 0.0, 1.0);
    let side = 0.12 * max(0.0, d.x) + 0.08 * max(0.0, -d.z);
    return vec3<f32>(0.42 + 0.55 * up * up + side);
}

fn pcf(tex: texture_depth_2d, coord: vec3<f32>, texel: f32) -> f32 {
    if coord.x < 0.0 || coord.x > 1.0 || coord.y < 0.0 || coord.y > 1.0 || coord.z > 1.0 || coord.z < 0.0 {
        return 1.0;
    }
    var s = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let o = vec2<f32>(f32(x), f32(y)) * texel * 1.5;
            s += textureSampleCompareLevel(tex, shadow_samp, coord.xy + o, coord.z);
        }
    }
    return s / 9.0;
}

fn shadow_coord(vp: mat4x4<f32>, p: vec3<f32>, bias: f32) -> vec3<f32> {
    let c = vp * vec4<f32>(p, 1.0);
    let n = c.xyz / c.w;
    return vec3<f32>(n.x * 0.5 + 0.5, 0.5 - n.y * 0.5, n.z + bias);
}

struct Lobe {
    diffuse: vec3<f32>,
    spec: vec3<f32>,
    sheen: vec3<f32>,
};

fn direct(l: vec3<f32>, radiance: vec3<f32>, n: vec3<f32>, v: vec3<f32>, albedo: vec3<f32>, rough: f32, sheen_col: vec3<f32>, sheen_rough: f32, cc: f32) -> Lobe {
    var o: Lobe;
    let nl = clamp(dot(n, l), 0.0, 1.0);
    let irr = radiance * nl;
    let h = normalize(l + v);
    let nh = clamp(dot(n, h), 0.0, 1.0);
    let nv = clamp(dot(n, v), 1e-4, 1.0);
    let vh = clamp(dot(v, h), 0.0, 1.0);
    let alpha = max(rough * rough, 0.0025);
    o.diffuse = irr * albedo / PI;
    o.spec = irr * f_schlick(vec3<f32>(0.04), vh) * (v_smith(alpha, nl, nv) * d_ggx(alpha, nh));
    if cc > 0.0 {
        let ca = 0.3 * 0.3;
        o.spec += irr * cc * f_schlick(vec3<f32>(0.04), vh) * (v_smith(ca, nl, nv) * d_ggx(ca, nh));
    }
    o.sheen = irr * sheen_col * d_charlie(max(sheen_rough, 0.07), nh) * v_neubelt(nv, nl);
    return o;
}

@fragment
fn fs_main(in: VOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    let kind = i32(m.tint_out.w + 0.5);
    // every texture sample up front (uniform control flow)
    let tuv = vec2<f32>(in.uv.x, 1.0 - in.uv.y); // three.js canvas textures are flipped
    let t_out = textureSample(atlas_out, atlas_samp, tuv);
    let t_in = textureSample(atlas_in, atlas_samp, tuv);
    let nuv = in.uv * m.nmap.xy;
    let nt = textureSample(nrm_tex, nrm_samp, nuv).xyz * 2.0 - 1.0;
    // the knit laid straight onto the surface from three sides (realistic model, misc.w > 0)
    let tk = m.misc.w;
    let tpx = textureSample(nrm_tex, nrm_samp, in.world.zy * tk).xyz * 2.0 - 1.0;
    let tpy = textureSample(nrm_tex, nrm_samp, in.world.xz * tk).xyz * 2.0 - 1.0;
    let tpz = textureSample(nrm_tex, nrm_samp, in.world.xy * tk).xyz * 2.0 - 1.0;
    let dpx = dpdx(in.world);
    let dpy = dpdy(in.world);
    let duvx = dpdx(nuv);
    let duvy = dpdy(nuv);
    let cookie_uv = shadow_coord(g.gobo_vp, in.world, 0.0);
    let ck = textureSampleLevel(cookie, lin_clamp, cookie_uv.xy, 0.0).rgb;

    let inside = kind == 2 || (kind == 0 && !front);
    var base: vec3<f32>;
    var alpha = 1.0;
    if kind == 3 {
        base = m.solid.rgb;
    } else {
        let t = select(t_out, t_in, inside);
        base = srgb_to_linear(t.rgb) * select(m.tint_out.rgb, m.tint_in.rgb, inside); // atlas stored sRGB-encoded (UNORM)
        if m.misc.y > 0.5 {
            // alpha test at 0.5, softened over a pixel for alpha-to-coverage
            let w = max(fwidth(t.a), 1e-4);
            alpha = smoothstep(0.5, 0.5 + w, t.a);
            if alpha <= 0.0 {
                discard;
            }
        }
    }
    if m.misc.x > 0.5 {
        base *= in.ao;
    }

    // normals: back faces turn round; fabric weave from the tiling normal map
    let face = select(-1.0, 1.0, front);
    var n = normalize(in.nrm) * face;
    let ns = select(m.nmap.z, m.nmap.w, inside);
    if kind != 3 && ns > 0.0 && tk > 0.0 {
        // triplanar, whiteout blend: each projection's bump added in its own plane, weighted by
        // how squarely the surface faces it
        var bw = pow(abs(n), vec3<f32>(4.0));
        bw = bw / max(bw.x + bw.y + bw.z, 1e-5);
        let ax = vec3<f32>(tpx.xy * ns + n.zy, abs(tpx.z) * n.x);
        let ay = vec3<f32>(tpy.xy * ns + n.xz, abs(tpy.z) * n.y);
        let az = vec3<f32>(tpz.xy * ns + n.xy, abs(tpz.z) * n.z);
        n = normalize(ax.zyx * bw.x + ay.xzy * bw.y + az.xyz * bw.z);
    } else if kind != 3 && ns > 0.0 {
        let q1p = cross(dpy, n);
        let q0p = cross(n, dpx);
        let tt = q1p * duvx.x + q0p * duvy.x;
        let bb = q1p * duvx.y + q0p * duvy.y;
        let det = max(dot(tt, tt), dot(bb, bb));
        let sc = select(face * inverseSqrt(det), 0.0, det == 0.0);
        let mn = vec3<f32>(nt.xy * ns, nt.z);
        n = normalize(tt * (mn.x * sc) + bb * (mn.y * sc) + n * mn.z);
    }
    let v = normalize(g.cam_pos.xyz - in.world);
    let rough = m.pbr.x;
    let sheen_col = m.sheen_col.rgb * m.pbr.y;
    let sheen_rough = m.pbr.z;
    let cc = m.pbr.w;

    // shadows (offset along the geometric normal like three.js' normalBias)
    let gn = normalize(in.nrm) * face;
    var sh_key = 1.0;
    if g.flags.x > 0.5 && m.misc.z > 0.5 {
        sh_key = pcf(shadow_key, shadow_coord(g.key_vp, in.world + gn * 0.35, -0.0004), g.flags.z);
    }
    var lo: Lobe;
    lo.diffuse = vec3<f32>(0.0);
    lo.spec = vec3<f32>(0.0);
    lo.sheen = vec3<f32>(0.0);
    for (var i = 0; i < 3; i++) {
        var dir = g.key;
        var s = sh_key;
        if i == 1 {
            dir = g.fill;
            s = 1.0;
        } else if i == 2 {
            dir = g.rim;
            s = 1.0;
        }
        let r = direct(normalize(dir.xyz), vec3<f32>(dir.w * s), n, v, base, rough, sheen_col, sheen_rough, cc);
        lo.diffuse += r.diffuse;
        lo.spec += r.spec;
        lo.sheen += r.sheen;
    }
    // window-light spot with its cookie
    if g.flags.y > 0.5 {
        let to_l = g.gobo_pos.xyz - in.world;
        let l = normalize(to_l);
        let ang = dot(-l, normalize(g.gobo_dir.xyz));
        let cone = smoothstep(g.gobo_dir.w, g.gobo_col.w, ang);
        var s = 1.0;
        if m.misc.z > 0.5 {
            s = pcf(shadow_gobo, shadow_coord(g.gobo_vp, in.world + gn * 0.35, -0.0004), g.flags.w);
        }
        let rad = g.gobo_col.rgb * g.gobo_pos.w * cone * s * srgb_to_linear(ck);
        let r = direct(l, rad, n, v, base, rough, sheen_col, sheen_rough, cc);
        lo.diffuse += r.diffuse;
        lo.spec += r.spec;
        lo.sheen += r.sheen;
    }
    // hemisphere light + environment
    let nv = clamp(dot(n, v), 1e-4, 1.0);
    let hemi = mix(g.ground.rgb, g.sky.rgb, 0.5 * n.y + 0.5);
    let env_irr = PI * env_radiance(n) * g.env.x;
    let indirect_diffuse = (hemi + env_irr) * base / PI;
    let ab = dfg(nv, rough);
    let refl = reflect(-v, n);
    let indirect_spec = env_radiance(refl) * g.env.x * (vec3<f32>(0.04) * ab.x + vec3<f32>(ab.y));
    let sheen_ind = env_irr / PI * sheen_col * ibl_sheen(nv, sheen_rough);
    let sheen_comp = 1.0 - 0.157 * max(sheen_col.r, max(sheen_col.g, sheen_col.b));
    var col = (lo.diffuse + lo.spec + indirect_diffuse + indirect_spec) * sheen_comp + lo.sheen + sheen_ind;
    if inside {
        col += m.emissive_in.rgb;
    }
    // tone mapping, sRGB, then brightness / contrast (as the web app does after three's output)
    var out = linear_to_srgb(clamp(neutral_tonemap(col * g.env.y), vec3<f32>(0.0), vec3<f32>(1.0)));
    out = clamp((out - 0.5) * g.env.w + 0.5, vec3<f32>(0.0), vec3<f32>(1.0)) * g.env.z;
    return vec4<f32>(out * alpha, alpha);
}

// ---------------------------------------------------------------- shadow maps
struct ShadowU {
    vp: mat4x4<f32>,
};
@group(2) @binding(0) var<uniform> su: ShadowU; // group 0 stays unbound in the shadow pass

struct SOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_shadow(v: VIn) -> SOut {
    var o: SOut;
    o.clip = su.vp * vec4<f32>(v.pos, 1.0);
    o.uv = v.uv;
    return o;
}

@fragment
fn fs_shadow(in: SOut) {
    let kind = i32(m.tint_out.w + 0.5);
    let a = textureSample(atlas_out, atlas_samp, vec2<f32>(in.uv.x, 1.0 - in.uv.y)).a;
    if kind != 3 && m.misc.y > 0.5 && a < 0.5 {
        discard;
    }
}

