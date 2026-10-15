// Enhanced graphics: the glare round the sun and the moon, added to the high-range picture
// before the glow (post.wgsl) takes it. Three things the stock picture lacks:
//
// - the veil: bright light scatters in the eye (and in a lens) and lays a haze over everything
//   round its source, whether that is in the picture or just outside it. The CIE's glare
//   equation says how much: the veil's luminance is the source's illuminance E at the eye
//   times 10 / theta^3 (theta, degrees from the source; a small second term widens it). It
//   needs no sprite: it is a function of the angle from the source, per pixel. How much of the
//   source is seen decides its strength: the sun behind a tree is a few bright points, and the
//   veil is that much weaker.
// - how much is seen: `fs_seen` looks at a ring of directions round the sun and the moon
//   (13 each) in the depth prepass (the sky is depth 0): which of them show the sky, whether a
//   cloud covers the source (the sky cube's alpha), smoothed over a few frames so that a leaf
//   passing the sun does not flicker the picture. It draws one pixel.
// - the shafts: from a pixel towards the sun on the screen, how much sky lies in the way. A
//   pixel behind a tree with a gap between it and the sun is lit by what comes through the
//   gap: the streaks of light through a canopy or between buildings (a screen-space estimate,
//   Mitchell's light shafts, with the picture's own depth as the occluders).
//
// Drawn once per window frame, additively, over the resolved picture. The sky itself is not
// touched: the disc and the aureole are drawn by the sky shader, the veil is on top.

struct GlareParams {
    // xyz the camera's forward, w tan(half the vertical field of view)
    fwd: vec4<f32>,
    // xyz right, w the picture's aspect (width / height)
    right: vec4<f32>,
    // xyz up, w the setting's strength
    up: vec4<f32>,
    // xyz towards the sun, w how much of the sun's light the weather lets through (0..1)
    sun: vec4<f32>,
    // rgb the sun's irradiance in the picture's own units (pre-exposed), w the ring's radius (rad)
    sun_e: vec4<f32>,
    // xyz towards the moon, w how strongly it glares (0 under the horizon or by day)
    moon: vec4<f32>,
    // rgb the moon's irradiance in the picture's own units, w how far the smoothing moves this frame
    moon_e: vec4<f32>,
    // x the clock (s), y the shafts' strength
    misc: vec4<f32>,
};
@group(0) @binding(0) var<uniform> g: GlareParams;
@group(0) @binding(1) var t_depth: texture_depth_2d;
@group(0) @binding(2) var t_cube: texture_cube<f32>;
@group(0) @binding(3) var s_cube: sampler;
// the smoothed visibility of the last frame (fs_seen) / of this one (fs_veil): x the sun's, y the
// moon's, z how clear of cloud the sun is, w how clear the moon is
@group(0) @binding(4) var t_seen: texture_2d<f32>;
// the picture so far (only read by fs_seen: the sky, and whatever has been drawn over it)
@group(0) @binding(5) var t_hdr: texture_2d<f32>;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> VsOut {
    let x = f32(i32(i & 1u) * 4 - 1);
    let y = f32(i32(i >> 1u) * 4 - 1);
    var out: VsOut;
    out.clip = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}

fn luma(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

// A direction on the screen: xy in -1..1 (y up), z how far in front of the camera it lies.
fn project(dir: vec3<f32>) -> vec3<f32> {
    let z = dot(dir, g.fwd.xyz);
    let w = max(z, 1e-3) * g.fwd.w;
    return vec3<f32>(dot(dir, g.right.xyz) / (w * g.right.w), dot(dir, g.up.xyz) / w, z);
}

// 1 where the direction shows the sky, 0 where the depth prepass has something in front of
// it; -1 for a direction off the screen, where nothing is known (and the glare of a source
// just outside the picture is the reason to ask: see `edge_sky`).
fn sky_at(dir: vec3<f32>) -> f32 {
    let p = project(dir);
    if (p.z <= 0.02 || abs(p.x) >= 1.0 || abs(p.y) >= 1.0) {
        return -1.0;
    }
    let size = vec2<f32>(textureDimensions(t_depth));
    let px = vec2<i32>(vec2<f32>((p.x * 0.5 + 0.5) * size.x, (0.5 - p.y * 0.5) * size.y));
    let d = textureLoad(t_depth, clamp(px, vec2<i32>(0), vec2<i32>(size) - vec2<i32>(1)), 0);
    return select(0.0, 1.0, d <= 1e-7);
}

// Two unit vectors across the direction s.
fn across(s: vec3<f32>) -> mat2x3<f32> {
    let a = normalize(cross(s, select(vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(1.0, 0.0, 0.0), abs(s.z) > 0.95)));
    return mat2x3<f32>(a, cross(s, a));
}

// [ROLLBACK sunglare-3] What a source just off the picture is seen through: the sky at the
// picture's edge nearest to it (seven depth reads along the border). A window shows sky there
// (the glass is not in the depth), the roof and the walls of the cab do not - the sun beside
// the picture then does not shine through them. 1 for a direction on the screen; 0 behind the
// camera (nothing of it reaches the picture but a faint, far veil).
fn edge_sky(s: vec3<f32>) -> f32 {
    let p = project(s);
    if (p.z <= 0.02) {
        return 0.0;
    }
    if (abs(p.x) < 1.0 && abs(p.y) < 1.0) {
        return 1.0;
    }
    let m = max(abs(p.x), abs(p.y));
    let q = p.xy / m;
    let tang = select(vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), abs(p.y) > abs(p.x));
    let size = vec2<f32>(textureDimensions(t_depth));
    var sum = 0.0;
    for (var i = 0; i < 7; i = i + 1) {
        let a = (f32(i) - 3.0) / 3.0;
        let c = clamp(q * 0.96 + tang * a * 0.25, vec2<f32>(-0.99), vec2<f32>(0.99));
        let px = vec2<i32>(vec2<f32>((c.x * 0.5 + 0.5) * size.x, (0.5 - c.y * 0.5) * size.y));
        let d = textureLoad(t_depth, clamp(px, vec2<i32>(0), vec2<i32>(size) - vec2<i32>(1)), 0);
        sum = sum + select(0.0, 1.0, d <= 1e-7);
    }
    return sum / 7.0;
}

// The share of 13 directions in a hexagonal ring (the centre, six at half the radius, six at
// the full one) round s, `r` radians out, that show the sky; the ones off the screen count
// as much as `unknown`.
fn ring_seen(s: vec3<f32>, r: f32, unknown: f32) -> f32 {
    let ab = across(s);
    var v = sky_at(s);
    var sum = select(v, unknown, v < 0.0);
    for (var k = 0; k < 6; k = k + 1) {
        let a = f32(k) * 1.0471976;
        let c = cos(a);
        let sn = sin(a);
        let inner = normalize(s + (ab[0] * c + ab[1] * sn) * (0.5 * r));
        let outer = normalize(s + (ab[0] * cos(a + 0.5235988) + ab[1] * sin(a + 0.5235988)) * r);
        v = sky_at(inner);
        sum = sum + select(v, unknown, v < 0.0);
        v = sky_at(outer);
        sum = sum + select(v, unknown, v < 0.0);
    }
    return sum / 13.0;
}

// How much of the sun's disc the picture itself shows at a direction: the picture's luminance
// there against what the disc alone would have (`ref_l`), 0 where something covers it - a
// building, a tree, a sun blind, the bus's roof - and in between where something thin does
// (a leaf's edge, a blind's weave, a cloud). The picture knows what the depth prepass does not:
// a blended sun blind or a window's tint is not in the depth, and light does not come through
// them as it does through the sky. -1 for a direction off the screen, where nothing is known.
fn hdr_seen(dir: vec3<f32>, ref_l: f32) -> f32 {
    let p = project(dir);
    if (p.z <= 0.02 || abs(p.x) >= 1.0 || abs(p.y) >= 1.0) {
        return -1.0;
    }
    let size = vec2<f32>(textureDimensions(t_hdr));
    let px = vec2<i32>(vec2<f32>((p.x * 0.5 + 0.5) * size.x, (0.5 - p.y * 0.5) * size.y));
    var c = textureLoad(t_hdr, clamp(px, vec2<i32>(0), vec2<i32>(size) - vec2<i32>(1)), 0).rgb;
    c = select(c, vec3<f32>(0.0), c != c);
    return smoothstep(0.02, 0.2, luma(min(c, vec3<f32>(65000.0))) / ref_l);
}

// The share of the 13 directions of the ring round the sun (inside its disc) that show it;
// the ones off the screen count as much as the cloud over the sun lets through.
fn sun_seen(s: vec3<f32>, r: f32, unknown: f32) -> f32 {
    // the disc's radiance in the picture (the sky shader clamps it at 4000), half of it for
    // the limb's darkening and the air in the way
    let disc_r = r / 0.8;
    let ref_l = max(min(luma(g.sun_e.rgb) / (3.14159265 * disc_r * disc_r), 4000.0) * 0.5, 1e-3);
    let ab = across(s);
    var sum = 0.0;
    var v = hdr_seen(s, ref_l);
    sum = sum + select(v, unknown, v < 0.0);
    for (var k = 0; k < 6; k = k + 1) {
        let a = f32(k) * 1.0471976;
        let inner = normalize(s + (ab[0] * cos(a) + ab[1] * sin(a)) * (0.5 * r));
        let outer = normalize(s + (ab[0] * cos(a + 0.5235988) + ab[1] * sin(a + 0.5235988)) * r);
        v = hdr_seen(inner, ref_l);
        sum = sum + select(v, unknown, v < 0.0);
        v = hdr_seen(outer, ref_l);
        sum = sum + select(v, unknown, v < 0.0);
    }
    return sum / 13.0;
}

// How clear of cloud the direction is (the sky cube's alpha is the cover).
fn cloud_clear(s: vec3<f32>) -> f32 {
    let cover = textureSampleLevel(t_cube, s_cube, vec3<f32>(s.x, s.z, s.y), 0.0).a;
    return 1.0 - smoothstep(0.15, 0.9, cover);
}

@fragment
fn fs_seen(in: VsOut) -> @location(0) vec4<f32> {
    let prev = textureLoad(t_seen, vec2<i32>(0, 0), 0);
    // (a direction of zero length is read as straight down: no NaN may get into the
    // smoothed value, which would stay there)
    let sun = normalize(g.sun.xyz + vec3<f32>(0.0, 0.0, -1e-6));
    let moon = normalize(g.moon.xyz + vec3<f32>(0.0, 0.0, -1e-6));
    let r = g.sun_e.w;
    // (the sun: what the picture shows of its disc, which includes the cloud, so z is 1; the
    // moon: the depth prepass and the cloud's cover)
    // (z: how clear of cloud the sun is, for the shafts; the veil takes the cloud from the
    // picture, in x)
    var now = vec4<f32>(
        sun_seen(sun, r, cloud_clear(sun) * edge_sky(sun)),
        ring_seen(moon, r, edge_sky(moon)),
        cloud_clear(sun),
        cloud_clear(moon),
    );
    now = select(now, vec4<f32>(0.0), now != now);
    return mix(prev, now, g.moon_e.w);
}

// The CIE's glare equation, theta in degrees: the veil's luminance over the illuminance.
fn veil_of(theta_deg: f32) -> f32 {
    let t = max(theta_deg, 0.35);
    return 10.0 / (t * t * t) + 0.35 / (t * t * (1.0 + pow(t / 62.5, 4.0)));
}

// The glare of one source towards dir.
fn glare_of(dir: vec3<f32>, s: vec3<f32>, e: vec3<f32>, seen: f32) -> vec3<f32> {
    if (seen <= 0.0) {
        return vec3<f32>(0.0);
    }
    let theta = degrees(atan2(length(cross(dir, s)), dot(dir, s)));
    return e * seen * veil_of(theta);
}

// Interleaved gradient noise (Jimenez): a fine, even jitter for the shafts' steps.
fn ign(p: vec2<f32>) -> f32 {
    return fract(52.9829189 * fract(dot(p, vec2<f32>(0.06711056, 0.00583715))));
}

// The shafts: the light that reaches the pixel through the gaps between it and the sun on
// the screen. `suv` is the sun in picture coordinates (0..1, y down); the pixel marches 24
// steps towards it over (at most) 40% of the picture, taking in the sky it passes, the
// nearest steps most.
fn shafts(uv: vec2<f32>, suv: vec2<f32>, jitter: f32, aspect: f32, outside: f32) -> f32 {
    let to = suv - uv;
    let len = length(to * vec2<f32>(aspect, 1.0));
    if (len < 1e-4) {
        return 1.0;
    }
    let reach = min(1.0, 0.4 / len);
    let size = vec2<f32>(textureDimensions(t_depth));
    var acc = 0.0;
    var wsum = 0.0;
    for (var i = 0; i < 24; i = i + 1) {
        let t = (f32(i) + jitter) / 24.0 * reach;
        let q = uv + to * t;
        if (q.x < 0.0 || q.y < 0.0 || q.x >= 1.0 || q.y >= 1.0) {
            // off the picture on the way: nothing is known but what the picture's edge shows
            // there (`edge_sky`: a roof or a wall at the edge keeps the light out)
            acc = acc + outside * exp(-t * 2.2);
            wsum = wsum + exp(-t * 2.2);
            continue;
        }
        let d = textureLoad(t_depth, vec2<i32>(q * size), 0);
        let w = exp(-t * 2.2);
        acc = acc + select(0.0, w, d <= 1e-7);
        wsum = wsum + w;
    }
    return acc / max(wsum, 1e-4);
}

@fragment
fn fs_veil(in: VsOut) -> @location(0) vec4<f32> {
    let ndc = vec2<f32>(in.uv.x * 2.0 - 1.0, 1.0 - in.uv.y * 2.0);
    let dir = normalize(g.fwd.xyz + g.right.xyz * (ndc.x * g.fwd.w * g.right.w) + g.up.xyz * (ndc.y * g.fwd.w));
    let seen = textureLoad(t_seen, vec2<i32>(0, 0), 0);
    let sun = normalize(g.sun.xyz + vec3<f32>(0.0, 0.0, -1e-6));
    let moon = normalize(g.moon.xyz + vec3<f32>(0.0, 0.0, -1e-6));
    // the sun's light through the weather and above the horizon (the veil takes how much of it
    // shows from the picture, x; the shafts need no sight of the disc, only of the sky)
    let sun_up = g.sun.w * smoothstep(-0.03, 0.06, sun.z);
    var col = glare_of(dir, sun, g.sun_e.rgb, seen.x * sun_up);
    // the moon glares less than its light says: the eye, adapted to the night, has the veil
    // but the picture's exposure has caught up with it (a full moon blazed: a fifth of it)
    col = col + glare_of(dir, moon, g.moon_e.rgb, seen.y * seen.w * g.moon.w) * 0.2;
    let tint = g.sun_e.rgb / max(luma(g.sun_e.rgb), 1e-4);
    let sun_clear = seen.z * sun_up;
    // the shafts, from the sun's place on the screen when it is in front of the camera
    let ps = project(sun);
    if (g.misc.y > 0.0 && ps.z > 0.05 && sun_clear > 0.02) {
        let suv = vec2<f32>(ps.x * 0.5 + 0.5, 0.5 - ps.y * 0.5);
        // (the sun may be off the picture: the shafts then run in from its side)
        let lit = shafts(in.uv, suv, ign(in.clip.xy), g.right.w, edge_sky(sun));
        let off = length((in.uv - suv) * vec2<f32>(g.right.w, 1.0));
        let reach = 1.0 / (1.0 + (off / 0.35) * (off / 0.35));
        // a streak is bright where the pixel itself is in the shade of something near, so that
        // the light shows (on open sky it only brightens the sky, which the veil does)
        let shade = 1.0 - max(sky_at(dir), 0.0);
        // (as strong as the sun's light is: dim at dusk with the veil)
        // (the rays stay when the sun itself is hidden behind a tree or a building: they are the
        // light that gets round it; they are a little weaker then)
        col = col + tint * g.misc.y * lit * reach * (0.25 + 0.75 * shade) * sun_clear * 0.3 * min(luma(g.sun_e.rgb), 1.0) * (0.7 + 0.3 * smoothstep(0.0, 0.25, seen.x));
    }
    // [ROLLBACK moonrays-1] and the same from the moon at night, fainter (a moon's rays are
    // what a bright moon through a canopy or between buildings gives)
    let pm = project(moon);
    let moon_clear = seen.w * g.moon.w;
    if (g.misc.y > 0.0 && pm.z > 0.05 && moon_clear > 0.02) {
        let muv = vec2<f32>(pm.x * 0.5 + 0.5, 0.5 - pm.y * 0.5);
        let lit = shafts(in.uv, muv, ign(in.clip.xy + vec2<f32>(17.0, 31.0)), g.right.w, edge_sky(moon));
        let off = length((in.uv - muv) * vec2<f32>(g.right.w, 1.0));
        let reach = 1.0 / (1.0 + (off / 0.3) * (off / 0.3));
        let mtint = g.moon_e.rgb / max(luma(g.moon_e.rgb), 1e-4);
        let shade = 1.0 - max(sky_at(dir), 0.0);
        col = col + mtint * g.misc.y * lit * reach * (0.25 + 0.75 * shade) * moon_clear * 0.3 * min(luma(g.moon_e.rgb), 1.0) * 0.1 * (0.7 + 0.3 * smoothstep(0.0, 0.25, seen.y));
    }
    return vec4<f32>(min(col * g.up.w, vec3<f32>(3000.0)), 0.0);
}
