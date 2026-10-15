// Enhanced graphics: the sky - the physical sky table, clouds lit by the sun and the sky,
// the sun's disc and the air - and the reflection probe drawn from it.

// Clouds: a ray-marched volume on the curved Earth, built the way Guerrilla's Horizon Zero
// Dawn clouds are (and bevy-volumetric-clouds and Frostbite after them):
// * the layer is a spherical shell 1400-2800 m over the ground of an Earth of 6371 km, so
//   the clouds sink to the horizon and thin out into the distance as real ones do;
// * the shape map (clouds.rs: Perlin cut by Worley cells, repeating every 13 km) says where
//   the heaps stand and how much cover each needs to appear; its blue channel and a height
//   gradient round their tops and flatten their bases; the weather's cover (Cumulus 1-3,
//   Overcast) and the weather's own cloud picture nudge how much of it appears;
// * the detail volume (3-D Worley billows, repeating every 420 m) eats the edges, most at
//   the thin rims and least in the dense cores;
// * light: the sun through the cloud towards it (six steps growing 1.7 times, 1.4 km in
//   all) with Hillaire's approximation of multiple scattering (three octaves of weaker
//   extinction and a flatter phase), a two-lobe Henyey-Greenstein phase (a silver lining
//   towards the sun, a little back-scatter away from it), the sky's light from above and
//   the ground's from below by height, integrated per step the energy-conserving
//   Frostbite way; far clouds take on the air's colour.
// The sky cube keeps the result in a fixed world frame (lib.rs `SKY_CUBE_SIZE`): each
// redraw starts the steps at another random point and is blended into what is there, so
// the grain of a few dozen steps averages out over the frames.
const CLOUD_BOTTOM: f32 = 1400.0;
const CLOUD_TOP: f32 = 2800.0;
const EARTH_R: f32 = 6371000.0;
const CLOUD_STEPS: i32 = 56;
// Extinction per metre of the densest cloud.
const CLOUD_SIGMA: f32 = 0.035;
const CLOUD_SHAPE_PERIOD: f32 = 13000.0;
const CLOUD_DETAIL_PERIOD: f32 = 420.0;
const CLOUD_DETAIL_STRENGTH: f32 = 0.3;
const CLOUD_EDGE_SOFTNESS: f32 = 0.12;
const CLOUD_BOTTOM_SOFTNESS: f32 = 0.2;
const CLOUD_MAX_DIST: f32 = 60000.0;
const CLOUD_MS_GAIN: f32 = 2.4;

@group(1) @binding(6) var t_cloud_shape: texture_2d<f32>;
@group(1) @binding(7) var t_cloud_detail: texture_3d<f32>;
@group(1) @binding(8) var s_cloud: sampler;

// Where in its first step this pixel's ray starts (0..1; 0.5 for the reflection probe).
var<private> cloud_jitter: f32 = 0.5;

fn linearstep(a: f32, b: f32, v: f32) -> f32 {
    return clamp((v - a) / (b - a), 0.0, 1.0);
}

// Height over the curved ground of the point t metres along d from the camera.
fn cloud_height(d: vec3<f32>, t: f32) -> f32 {
    return camera.cam_pos.z + eye_off.z + d.z * t + t * t * (1.0 - d.z * d.z) / (2.0 * EARTH_R);
}

// Distance along d at which the ray reaches height h over the curved ground (the far root
// when the camera is above h; -1 when it never does).
fn cloud_shell(d: vec3<f32>, h: f32) -> f32 {
    let a = (1.0 - d.z * d.z) / (2.0 * EARTH_R);
    let b = d.z;
    let c = camera.cam_pos.z + eye_off.z - h;
    if (a < 1e-12) {
        return select(-1.0, -c / b, abs(b) > 1e-6 && -c / b > 0.0);
    }
    let disc = b * b - 4.0 * a * c;
    if (disc < 0.0) {
        return -1.0;
    }
    let q = -0.5 * (b + select(-1.0, 1.0, b >= 0.0) * sqrt(disc));
    let r0 = q / a;
    let r1 = c / q;
    let lo = min(r0, r1);
    let hi = max(r0, r1);
    if (lo > 0.0) {
        return lo;
    }
    return select(-1.0, hi, hi > 0.0);
}

// The sun's irradiance at height z (as `cloud_height` gives it) in the cloud layer:
// between the base, middle and top of `enh.cloud_sun`.
fn cloud_sun_at(z: f32) -> vec3<f32> {
    let t = clamp((z - CLOUD_BOTTOM) / (CLOUD_TOP - CLOUD_BOTTOM), 0.0, 1.0) * 2.0;
    return select(mix(enh.cloud_sun[1].rgb, enh.cloud_sun[2].rgb, t - 1.0), mix(enh.cloud_sun[0].rgb, enh.cloud_sun[1].rgb, t), t < 1.0);
}

// How much of the sky the weather covers here: its type (camera.clouds.x) and, over tens
// of kilometres, its own cloud picture.
fn cloud_coverage(p: vec2<f32>) -> f32 {
    let cover = camera.clouds.x;
    let uv = p / CLOUD_FIELD_TILE + camera.clouds.yz * (2500.0 / CLOUD_FIELD_TILE);
    let field = textureSampleLevel(t_clouds, s_repeat, uv * 0.35, 5.0).g;
    return clamp(0.3 + cover * 0.55 + (field - 0.5) * 0.25, 0.0, 1.0);
}

// The heap before its billows (h: 0 at the base, 1 at the top of the layer).
fn cloud_base_shape(p: vec3<f32>, h: f32, lod: f32) -> f32 {
    let drift = camera.clouds.yz * 2500.0;
    let s = textureSampleLevel(t_cloud_shape, s_cloud, (p.xy + drift) / CLOUD_SHAPE_PERIOD, lod);
    let lo = s.g - 1.0;
    // the heap narrows upwards (every heap: with the map's rounding alone, a heap where it
    // was small rose as a column with a flat lid against the top of the layer)
    let n = h * h * (0.7 + s.b) + pow(1.0 - h, 16.0);
    let m = (s.r - n - lo) / (1.0 - lo);
    return m * (linearstep(0.0, 0.1, h) - linearstep(0.6, 1.0, h));
}

// Extinction (1/m) at p; `detail` false for the light towards the sun far from the point.
fn cloud_sigma(p: vec3<f32>, h: f32, coverage: f32, lod: f32, detail: bool) -> f32 {
    if (h <= 0.0 || h >= 1.0) {
        return 0.0;
    }
    var m = cloud_base_shape(p, h, lod);
    // (the billows only take away)
    if (m + coverage - 1.0 <= 0.0) {
        return 0.0;
    }
    if (detail) {
        let drift = camera.clouds.yz * 2500.0 * 1.3;
        let q = vec3<f32>(p.xy + drift, p.z) / CLOUD_DETAIL_PERIOD;
        let dl = textureSampleLevel(t_cloud_detail, s_cloud, q, max(lod - 2.0, 0.0)).r;
        m = m - dl * smoothstep(1.0, 0.5, m) * CLOUD_DETAIL_STRENGTH;
    }
    m = smoothstep(0.0, CLOUD_EDGE_SOFTNESS, m + coverage - 1.0);
    m = m * min(h / CLOUD_BOTTOM_SOFTNESS, 1.0);
    return m * CLOUD_SIGMA;
}

// The clouds towards d in front of `below` (the sky behind them): rgb the picture, a how
// much the clouds cover.
fn cloud_layer(d: vec3<f32>, below: vec3<f32>, pix: f32) -> vec4<f32> {
    if (camera.clouds.x <= 0.001 || d.z <= -0.01 || camera.cam_pos.z + eye_off.z > CLOUD_BOTTOM) {
        return vec4<f32>(below, 0.0);
    }
    // [ROLLBACK moonclouds-1] by night the moon is the clouds' light: where its irradiance is
    // above what the sun still gives them, the march lights the clouds from the moon (its
    // phase and height are in `moon_disc`), with the same silver lining towards it
    let moon_dir = normalize(enh.moon.xyz + vec3<f32>(0.0, 0.0, -1e-6));
    let moon_up = smoothstep(-0.02, 0.06, moon_dir.z) * select(0.0, 1.0, camera.night.z > 0.0);
    let moon_irr = enh.moon_disc.rgb * moon_up;
    let use_moon = dot(moon_irr, vec3<f32>(0.2126, 0.7152, 0.0722)) > dot(enh.cloud_sun[1].rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    let sd = select(normalize(camera.sun_dir.xyz), moon_dir, use_moon);
    // a closed cover (or a sky that rain or snow falls from) is the grey deck of the table
    let closed = max(smoothstep(0.85, 1.0, camera.clouds.x), enh.weather.w);
    let t0 = cloud_shell(d, CLOUD_BOTTOM);
    if (t0 < 0.0 || t0 > CLOUD_MAX_DIST) {
        return vec4<f32>(below, 0.0);
    }
    let t1 = min(cloud_shell(d, CLOUD_TOP), min(t0 + 12000.0, CLOUD_MAX_DIST + 6000.0));
    let ds = (t1 - t0) / f32(CLOUD_STEPS);
    // how many texels of the shape map a pixel spans where the ray meets the clouds
    let lod = log2(max(t0 * pix * f32(textureDimensions(t_cloud_shape).x) / CLOUD_SHAPE_PERIOD, 1.0));
    // (the sun before the clouds - at their own height, through the air from up there:
    // how much of it the cover lets through, lights.w, is what this march works out itself)
    let sky_top = sh_irradiance(vec3<f32>(0.0, 0.0, 1.0)) / PI;
    let ground = sh_irradiance(vec3<f32>(0.0, 0.0, -1.0)) / PI;
    let cos_sun = dot(d, sd);
    // [ROLLBACK twilight-1] with the sun low, the underside of a cloud is lit by the glowing
    // horizon on the sun's side: a warm light from below, the more the nearer the sun
    let under = cloud_underlight(d, sd, use_moon);
    let coverage = cloud_coverage(cloud_ground(d, t0));
    var trans = 1.0;
    var acc = vec3<f32>(0.0);
    var hit = 0.0;
    var hit_w = 0.0;
    var t = t0 + ds * cloud_jitter;
    for (var i = 0; i < CLOUD_STEPS; i = i + 1) {
        let p = vec3<f32>(cloud_ground(d, t), cloud_height(d, t));
        let h = (p.z - CLOUD_BOTTOM) / (CLOUD_TOP - CLOUD_BOTTOM);
        let sigma = cloud_sigma(p, h, coverage, lod, true);
        if (sigma > 1e-6) {
            // the sunlight reaching p through the cloud towards the sun
            var od = 0.0;
            var ls = 40.0;
            var lt = ls * 0.5;
            for (var k = 0; k < 6; k = k + 1) {
                let q = p + sd * lt;
                let hq = (q.z - CLOUD_BOTTOM) / (CLOUD_TOP - CLOUD_BOTTOM);
                if (hq >= 1.0) {
                    break;
                }
                od = od + cloud_sigma(q, hq, coverage, lod + 1.0, k < 2) * ls;
                ls = ls * 1.7;
                lt = lt + ls;
            }
            // multiple scattering (Hillaire 2016): octaves of weaker extinction, weaker
            // light and a flatter phase
            let sun = select(cloud_sun_at(p.z), moon_irr, use_moon);
            var direct = vec3<f32>(0.0);
            var a = 1.0;
            var b = 1.0;
            var c = 1.0;
            for (var o = 0; o < 3; o = o + 1) {
                let phase = mix(hg_phase(cos_sun, 0.8 * c), hg_phase(cos_sun, -0.2 * c), 0.5);
                direct = direct + sun * a * phase * exp(-od * b);
                a = a * 0.5;
                b = b * 0.4;
                c = c * 0.5;
            }
            // (single scattering and three octaves hold only part of the light a cloud
            // scatters on inside it; a cumulus's sunlit side is about as bright as white
            // paper in the sun, E/π, which this factor brings it to)
            let amb = mix(ground * 0.45 + sky_top * 0.55, sky_top * 1.1, clamp(h * 1.4, 0.0, 1.0)) + under * (1.0 - clamp(h * 1.6, 0.0, 1.0));
            let light = direct * CLOUD_MS_GAIN + amb;
            // Frostbite: the light scattered over the step, dimmed by the cloud before it
            let dt = exp(-sigma * ds);
            acc = acc + trans * light * (1.0 - dt);
            hit = hit + t * trans * (1.0 - dt);
            hit_w = hit_w + trans * (1.0 - dt);
            trans = trans * dt;
            if (trans < 0.01) {
                break;
            }
        }
        t = t + ds;
    }
    // far clouds take on the colour of the air in front of them, and beyond 40 km fade out
    let dist = select(t0, hit / max(hit_w, 1e-4), hit_w > 1e-4);
    let aerial = 1.0 - exp(-dist / 22000.0);
    let fade = 1.0 - smoothstep(40000.0, CLOUD_MAX_DIST, dist);
    let horizon_fade = smoothstep(-0.01, 0.02, d.z);
    let a = (1.0 - trans) * fade * horizon_fade;
    acc = mix(acc, below * (1.0 - trans), aerial) * fade * horizon_fade;
    var col = acc + (1.0 - a) * below;
    let deck = below * (0.75 + 0.35 * cloud_cover_at(cloud_ground(d, max(t0, 1.0)), 4.0).x);
    col = mix(col, deck, closed);
    // the high, thin layer
    let drift = camera.clouds.yz * 2500.0;
    let t_hi = 7000.0 / max(d.z, 0.02);
    let p_hi = cloud_ground(d, t_hi);
    let hi = cloud_fbm((p_hi + drift * 1.7) / 2500.0);
    // (and the veil of the weather, as thick as its optical depth along the view makes it,
    // fibrous where the high layer's texture is)
    let veil = enh.cloud_sun[0].w;
    let veil_cover = (1.0 - exp(-veil / max(d.z, 0.08) * (0.55 + 0.45 * hi))) * 0.8;
    let hi_cover = max(clamp((hi - 0.6 + camera.clouds.x * 0.2) * 2.0, 0.0, 1.0) * 0.35, veil_cover) * clamp(d.z * 8.0, 0.0, 1.0) * (1.0 - a) * (1.0 - closed);
    // (lit by the sun at its own height, through thin ice: forward scattering towards the
    // sun, a dull grey away from it)
    let hi_sun = select(enh.cloud_sun[3].rgb, moon_irr, use_moon) * (0.12 + 0.5 * hg_phase(cos_sun, 0.6) * 4.0 * PI * 0.25);
    col = mix(col, (hi_sun * (1.0 - closed) / PI + sky_top) * 0.9, hi_cover);
    return vec4<f32>(col, max(max(a, hi_cover), closed));
}

// Sky radiance towards d without the sun's disc: the table, the clouds, the air.
fn sky_radiance(d: vec3<f32>, pix: f32) -> vec3<f32> {
    var col = sky_table(d);
    let c = cloud_layer(d, col, pix);
    col = c.rgb;
    // the weather's fog along the whole way to the horizon (the clear air is in the table
    // already: taken again, it whitened the whole sky)
    let h0 = camera.cam_pos.z - enh.fog.z;
    let dist = 30000.0;
    let a = air_of(d, dist, h0, h0 + dist * max(d.z, 0.0), 0.0);
    return col * a.a + a.rgb;
}

// The sky as the sky cube holds it (drawn a face a frame by `fs_sky_cube`, divided by the
// table scale): rgb, and how much cloud covers the direction.
@group(0) @binding(17) var t_sky_cube: texture_cube<f32>;

// The sky's colour towards `in.dir` as the enhanced path draws it, before the pre-exposure's
// clamp: the cube's clouds and air, the sun's disc, the night.
fn enhanced_sky_colour(in: VsOut) -> vec3<f32> {
    let d = normalize(in.dir);
    let pre = enh.exposure.x;
    // the cube is drawn from its own eye (lib.rs Probe::cube_eye): look the clouds' base up
    // from there, so the sky does not slide with a camera that moved since
    var ld = d;
    let tb = cloud_shell(d, CLOUD_BOTTOM);
    if (tb > 0.0 && camera.cam_pos.z < CLOUD_BOTTOM) {
        ld = normalize(d * min(tb, CLOUD_MAX_DIST) - enh.eye.xyz);
    }
    let cube = textureSampleLevel(t_sky_cube, s_lin, vec3<f32>(ld.x, ld.z, ld.y), 0.0);
    var col = cube.rgb * enh.ground.w;
    // [ROLLBACK twilight-1] the earth's shadow, the belt of Venus and the rays opposite the sun
    col = twilight_sky(d, col, cube.a);
    // the sun: a limb-darkened disc as bright as its irradiance spread over its size,
    // behind whatever cloud there is
    let sd = normalize(camera.sun_dir.xyz);
    let r = enh.sun.w;
    let cosang = dot(d, sd);
    if (cosang > cos(r * 1.5) && d.z > -0.02) {
        // (low in the sky it is flattened by the refraction and its rim ripples: `sun_disc_x`)
        let xy = sun_disc_x(d, sd, cosang, r);
        let x = xy.x;
        let disc = 1.0 - smoothstep(0.85, 1.1, x);
        let limb = 1.0 - 0.6 * (1.0 - sqrt(max(1.0 - min(x, 1.0) * min(x, 1.0), 0.0)));
        let cover = cube.a;
        let h0 = camera.cam_pos.z - enh.fog.z;
        let t = air_of(d, 30000.0, h0, h0 + 30000.0 * max(d.z, 0.0), 0.0).a;
        var l = enh.sun_disc.rgb / (PI * r * r) * disc * limb * (1.0 - cover) * enh.lights.w * t;
        l = green_rim(l, sd, xy.y);
        col = col + l;
    }
    // [ROLLBACK twilight-1] through thin high cloud: the halo, and a pillar over a low sun
    col = sun_halo_pillar(d, sd, col, cube.a);
    col = col + night_sky(d, cube.a, fwidth(d));
    col = col + aurora_layer(d, cube.a, in.clip.xy);
    // [ROLLBACK storm-1] lightning: the bolt and the clouds lit by it
    col = col + lightning_layer(d, length(fwidth(d)) * 0.7);
    return col * pre;
}

@fragment
fn fs_enhanced(in: VsOut) -> @location(0) vec4<f32> {
    // the dome is drawn pre-exposed; the disc is kept within what the target and the glow
    // filter handle
    return vec4<f32>(min(enhanced_sky_colour(in), vec3<f32>(4000.0)), 1.0);
}

// [ROLLBACK newsky-1] The same sky for the plain (Vanilla, Vanilla+) picture, which has no
// tone curve and no exposure of its own: the pre-exposed colour goes through a soft curve to
// the display's range, the horizon takes the fog's colour as the old sky's did, and the
// target is the classic one (`finish_output` encodes it when the shading is gamma-blended).
fn sky_display(c: vec3<f32>) -> vec3<f32> {
    let x = max(c, vec3<f32>(0.0));
    let knee = 0.66;
    let range = 1.0 - knee;
    let peak = max(x.r, max(x.g, x.b));
    // (a mild contrast about mid grey, then the shoulder by the brightest channel and by
    // channel: a low sun's glow runs to a warm white)
    let y = 0.18 * pow(x / 0.18 + vec3<f32>(1e-7), vec3<f32>(1.12));
    let yp = max(y.r, max(y.g, y.b));
    if (yp <= knee) {
        return y;
    }
    let np = knee + range * (1.0 - exp(-(yp - knee) / range));
    let by_peak = y * (np / yp);
    let per = select(y, vec3<f32>(knee) + range * (vec3<f32>(1.0) - exp(-(y - vec3<f32>(knee)) / range)), y > vec3<f32>(knee));
    let o = mix(by_peak, per, 0.4);
    let g = 1.0 - 1.0 / (0.3 * (yp - np) + 1.0);
    return mix(o, vec3<f32>(np), g);
}

@fragment
fn fs_enhanced_ldr(in: VsOut) -> @location(0) vec4<f32> {
    let d = normalize(in.dir);
    var col = sky_display(min(enhanced_sky_colour(in), vec3<f32>(4000.0)));
    let elev = asin(clamp(d.z, -1.0, 1.0));
    // the vanilla fog: swallows the horizon, and a thick one the whole sky
    let horizon = clamp(1.0 - elev / 0.12, 0.0, 1.0) * clamp(camera.fog.w * 1500.0, 0.0, 1.0);
    let whole = clamp(camera.fog.w * 150.0 - 0.15, 0.0, 1.0) * clamp(1.0 - elev / 1.2, 0.35, 1.0);
    // (and the last few degrees take the fog's colour in part, so that the far scenery,
    // which is fogged with it, meets the sky without a seam)
    let seam = 0.55 * (1.0 - smoothstep(0.0, 0.07, elev));
    let f = max(max(horizon, whole), seam);
    // (a dither of half a level of the 8-bit target: the slow gradients of a night sky, the
    // aurora's and a flash dying away, went down in visible steps of one level)
    let ign = fract(52.9829189 * fract(dot(in.clip.xy, vec2<f32>(0.06711056, 0.00583715))));
    let dither = vec3<f32>((ign - 0.5) / 255.0);
    var out_c: vec3<f32>;
    if (camera.sky_color.w > 0.5) {
        out_c = finish_output(srgb_decode(mix(srgb_encode(col), camera.fog.xyz, f)));
    } else {
        out_c = finish_output(mix(col, camera.fog.xyz, f));
    }
    // (the 8 bits are the encoded value's: on a linear output it is encoded, dithered, decoded)
    out_c = select(srgb_decode(srgb_encode(max(out_c, vec3<f32>(0.0))) + dither), out_c + dither, ENCODE_OUTPUT);
    return vec4<f32>(out_c, 1.0);
}

// [ROLLBACK aurora-1] The aurora's light towards d (in display units, so divided by the
// exposure the caller multiplies back): behind the clouds, dimmed by the haze, and only
// where the sky is dark - a moonlit or twilight sky washes it out.
fn aurora_layer(d: vec3<f32>, cover: f32, frag: vec2<f32>) -> vec3<f32> {
    if (abs(camera.fx.z) < 0.01 || d.z <= 0.0) {
        return vec3<f32>(0.0);
    }
    let h0 = camera.cam_pos.z - enh.fog.z;
    let through = air_of(d, 30000.0, h0, h0 + 30000.0 * max(d.z, 0.0), 0.0).a * (1.0 - cover);
    if (through < 0.01) {
        return vec3<f32>(0.0);
    }
    let sky_l = dot(sky_table(d), vec3<f32>(0.2126, 0.7152, 0.0722)) * enh.exposure.x;
    let gate = 1.0 - smoothstep(0.08, 0.7, clamp(sky_l * 90.0, 0.0, 1.0));
    if (gate < 0.01) {
        return vec3<f32>(0.0);
    }
    // (the event's seed rides in the thousands of 4096 of the distance, see `CameraUniform::fx`)
    let seed = floor((camera.fx.w + 512.0) / 4096.0);
    let a = aurora_sky(d, camera.post.y, camera.fx.z, camera.fx.w - seed * 4096.0, seed, frag);
    return a * gate * through / max(enh.exposure.x, 1e-6);
}

// ---- [ROLLBACK storm-1] Lightning ---------------------------------------------------------
// The strongest strike of the moment (`camera.bolt`, `camera.bolt2`, from `omsi_sim::storm`):
// its bolt, a jagged channel from the cloud base to the ground with a few forks, drawn in the
// plane tangent to the sky at its middle (so that its length is the cloud base's height seen from
// the strike's distance), and the light it throws into the clouds. In display units, divided
// by the exposure like the aurora, so that a flash is as bright by night as it is white by day.
fn lg_jag(u: f32, seed: f32, k: f32) -> f32 {
    return (vnoise(vec2<f32>(u * 7.0 + seed * 1.37, k * 13.1 + 2.0)) - 0.5) * 0.30
        + (vnoise(vec2<f32>(u * 19.0 + seed * 2.11, k * 7.7 + 5.0)) - 0.5) * 0.14
        + (vnoise(vec2<f32>(u * 47.0 + seed * 3.3, k * 3.3 + 9.0)) - 0.5) * 0.06;
}

fn lightning_bolt(d: vec3<f32>, pix: f32) -> f32 {
    let vis = camera.bolt2.x;
    if (vis < 0.01) {
        return 0.0;
    }
    let m = normalize(camera.bolt.xyz);
    let c = dot(d, m);
    if (c < 0.55) {
        return 0.0;
    }
    let ex = normalize(cross(vec3<f32>(0.0, 0.0, 1.0), m));
    let ey = cross(m, ex);
    let q = vec2<f32>(dot(d, ex), dot(d, ey)) / c;
    // the bolt runs from the ground (elevation 0) to the cloud base (twice the elevation of
    // its middle): in this plane from -h2 to +h2
    let h2 = tan(asin(clamp(m.z, 0.003, 0.95)));
    let u = (q.y + h2) / (2.0 * h2);
    if (u < -0.02 || u > 1.02) {
        return 0.0;
    }
    let seed = camera.bolt2.y;
    let w = pix * 1.1 + 0.0004;
    let hh = 2.0 * h2;
    let dx = q.x - lg_jag(u, seed, 0.0) * hh;
    var line = exp(-dx * dx / (w * w)) + 0.2 * exp(-dx * dx / (30.0 * w * w));
    // the forks: they leave the channel at some height and run down and out, thinner and fading
    for (var k = 0; k < 3; k = k + 1) {
        let kf = f32(k) + 1.0;
        let uk = mix(0.3, 0.92, mt_rand(seed, kf * 3.0));
        let side = select(-1.0, 1.0, mt_rand(seed, kf * 5.0) > 0.5);
        let slope = side * (0.3 + 0.5 * mt_rand(seed, kf * 7.0));
        let len = 0.15 + 0.3 * mt_rand(seed, kf * 11.0);
        let t = uk - u;
        if (t > 0.0 && t < len) {
            let xb = lg_jag(uk, seed, 0.0) * hh + slope * t * hh + lg_jag(u * 1.7 + kf, seed, kf) * hh * 0.7;
            let db = q.x - xb;
            line = max(line, 0.65 * (1.0 - t / len) * exp(-db * db / (0.6 * w * w)));
        }
    }
    let ends = smoothstep(-0.02, 0.0, u) * (1.0 - smoothstep(0.97, 1.02, u));
    return line * ends * vis;
}

// How much cloud there is towards d, worked out here at the picture's own resolution from the
// cloud shape map (two samples through the layer), and not read from the sky cube: the cube is
// refined a face at a time over many frames, so that lighting what it holds showed its faces
// (a box) and its refining (the light fading in steps).
fn lightning_cover(d: vec3<f32>, pix: f32) -> f32 {
    if (camera.clouds.x <= 0.001) {
        return 0.0;
    }
    let closed = max(smoothstep(0.85, 1.0, camera.clouds.x), enh.weather.w);
    let t0 = cloud_shell(d, CLOUD_BOTTOM);
    if (t0 < 0.0 || t0 > CLOUD_MAX_DIST) {
        return closed * 0.9;
    }
    let p = cloud_ground(d, t0);
    let cov = cloud_coverage(p);
    let lod = log2(max(t0 * pix * f32(textureDimensions(t_cloud_shape).x) / CLOUD_SHAPE_PERIOD, 1.0));
    let s = cloud_sigma(vec3<f32>(p, CLOUD_BOTTOM + 0.3 * (CLOUD_TOP - CLOUD_BOTTOM)), 0.3, cov, lod, false)
        + cloud_sigma(vec3<f32>(p, CLOUD_BOTTOM + 0.65 * (CLOUD_TOP - CLOUD_BOTTOM)), 0.65, cov, lod, false);
    return max(clamp(s / (CLOUD_SIGMA * 1.1), 0.0, 1.0), closed * 0.9);
}

fn lightning_glow(d: vec3<f32>, cover: f32) -> vec3<f32> {
    let f = camera.bolt.w;
    if (f < 0.004) {
        return vec3<f32>(0.0);
    }
    let m = normalize(camera.bolt.xyz);
    let n = normalize(m.xy + vec2<f32>(1e-6, 0.0));
    // (the light is in the cloud over the strike, higher than the bolt's middle)
    let el = min(asin(clamp(m.z, 0.0, 1.0)) * 2.3, 1.2);
    let l = vec3<f32>(cos(el) * n.x, cos(el) * n.y, sin(el));
    let ang = acos(clamp(dot(d, l), -1.0, 1.0));
    let g = exp(-ang * ang / 0.4) + 0.35 * exp(-ang * ang / 2.0);
    // the cloud is what is lit; the clear sky takes little of it
    let lit = 0.9 * pow(clamp(cover, 0.0, 1.0), 0.7) + 0.05;
    return vec3<f32>(0.60, 0.70, 1.0) * f * g * lit * 0.9 * smoothstep(-0.02, 0.05, d.z);
}

fn lightning_layer(d: vec3<f32>, pix: f32) -> vec3<f32> {
    if (camera.bolt.w < 0.004 && camera.bolt2.x < 0.01) {
        return vec3<f32>(0.0);
    }
    var cover = 0.0;
    if (camera.bolt.w >= 0.004 && d.z > 0.0) {
        cover = lightning_cover(d, pix);
    }
    let reach = clamp(1.4 / (1.0 + camera.bolt2.z / 3.0), 0.35, 1.2);
    let bolt = vec3<f32>(0.80, 0.88, 1.0) * lightning_bolt(d, pix) * 2.2 * reach * smoothstep(-0.02, 0.02, d.z);
    return (lightning_glow(d, cover) + bolt) / max(enh.exposure.x, 1e-6);
}

// The moon and the stars towards d (radiance), behind what cloud covers it (stars.wgsl).
// The moon is a lit sphere: its lit side faces the sun, its phase is what the sun's
// direction makes of it, its face is the moon's own (seas, ray craters) turned as it is
// seen from here. The stars are the sky of this place and hour; they show only where the
// sky behind them is dark - the haze, a lit city's glow and the moon's own scattered light
// leave the brightest few, a clear moonless country night all of them and the Milky Way.
fn night_sky(d: vec3<f32>, cover: f32, pix3: vec3<f32>) -> vec3<f32> {
    var col = vec3<f32>(0.0);
    let strength = camera.night.z;
    if (d.z < -0.01 || strength <= 0.0) {
        return col;
    }
    let h0 = camera.cam_pos.z - enh.fog.z;
    let through = air_of(d, 30000.0, h0, h0 + 30000.0 * max(d.z, 0.0), 0.0).a * (1.0 - cover);
    if (through < 0.01) {
        return col;
    }
    let pix = length(pix3) * 0.7;
    let m = enh.moon.xyz;
    let r = enh.moon.w;
    let sun = normalize(camera.sun_dir.xyz);
    if (dot(d, m) > cos(r * 1.3) && max(enh.moon_disc.r, enh.moon_disc.g) > 0.0) {
        let ms = moon_surface(d, m, r, sun, pix);
        // (as bright as the lit part's share of the irradiance spread over the disc)
        let lit_share = max(0.5 + 0.5 * dot(-m, sun), 0.03);
        col = col + enh.moon_disc.rgb / (PI * r * r * lit_share) * ms.x * ms.y * through;
    }
    // the faintest star that shows (a magnitude): in the picture's own units the sky behind
    // is sky_l; a clear dark sky shows the 6th magnitude, a hazy one or one with a city's
    // glow the first few, a twilight none
    let haze = clamp(1.0 - enh.moon_disc.w / 0.55, 0.0, 1.0);
    if (d.z > 0.0 && camera.night.w > 0.02) {
        let sky_l = dot(sky_table(d), vec3<f32>(0.2126, 0.7152, 0.0722)) * enh.exposure.x;
        let dark = clamp(sky_l * 90.0 + 0.45 * haze, 0.0, 1.0);
        let lim = 6.9 - 6.0 * dark;
        if (lim > 1.5) {
            let stars = night_stars(d, pix, lim, camera.post.y);
            col = col + stars * min(strength, 1.5) * 1.2 * through / max(enh.exposure.x, 1e-6);
        }
    }
    return col;
}

// --- the reflection probe: a cube map of the sky seen from the camera, drawn now and then
// and blurred into its mip levels for rough surfaces (divided by the table scale, which
// the main pass multiplies back in).
struct ProbeParams {
    // x first face of this pass (0 or 3), y roughness, z face size of level 0, w level
    p: vec4<f32>,
};
@group(0) @binding(15) var<uniform> probe: ProbeParams;
@group(0) @binding(16) var t_probe_src: texture_cube<f32>;

// Direction of a texel of cube face f (u, v in -1..1, v down), in cube coordinates.
fn face_dir(f: i32, u: f32, v: f32) -> vec3<f32> {
    var c = vec3<f32>(-u, -v, -1.0);
    switch f {
        case 0: { c = vec3<f32>(1.0, -v, -u); }
        case 1: { c = vec3<f32>(-1.0, -v, u); }
        case 2: { c = vec3<f32>(u, 1.0, v); }
        case 3: { c = vec3<f32>(u, -1.0, -v); }
        case 4: { c = vec3<f32>(u, -v, 1.0); }
        default: {}
    }
    return normalize(c);
}

fn cube_to_world(c: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(c.x, c.z, c.y);
}

struct ProbeOut {
    @location(0) a: vec4<f32>,
    @location(1) b: vec4<f32>,
    @location(2) c: vec4<f32>,
};

struct FsIn {
    @builtin(position) clip: vec4<f32>,
};

@vertex
fn vs_probe(@builtin(vertex_index) i: u32) -> FsIn {
    let x = f32(i32(i & 1u) * 4 - 1);
    let y = f32(i32(i >> 1u) * 4 - 1);
    return FsIn(vec4<f32>(x, y, 0.0, 1.0));
}

fn probe_sky(f: i32, uv: vec2<f32>) -> vec4<f32> {
    let c = face_dir(f, uv.x, uv.y);
    let d = cube_to_world(c);
    var col: vec3<f32>;
    if (d.z >= 0.0) {
        // the sky cube (lib.rs binds it as level 0's source), already divided by the table
        // scale: marching the clouds again for the probe took 3.4 ms every time it was drawn
        return vec4<f32>(textureSampleLevel(t_probe_src, s_lin, c, 0.0).rgb, 1.0);
    } else {
        // below the horizon: the table's distant ground, with the houses and trees of a
        // town darkening the band just under the horizon
        col = sky_table(d);
        let band = smoothstep(0.02, 0.25, -d.z);
        col = mix(col, enh.ground.rgb * 0.8, 1.0 - band * 0.6);
    }
    return vec4<f32>(col / max(enh.ground.w, 1e-8), 1.0);
}

@fragment
fn fs_probe_sky(in: FsIn) -> ProbeOut {
    let size = probe.p.z;
    let uv = in.clip.xy / size * 2.0 - vec2<f32>(1.0);
    let f0 = i32(probe.p.x);
    return ProbeOut(probe_sky(f0, uv), probe_sky(f0 + 1, uv), probe_sky(f0 + 2, uv));
}

fn radical_inverse(i: u32) -> f32 {
    var b = i;
    b = (b << 16u) | (b >> 16u);
    b = ((b & 0x55555555u) << 1u) | ((b & 0xAAAAAAAAu) >> 1u);
    b = ((b & 0x33333333u) << 2u) | ((b & 0xCCCCCCCCu) >> 2u);
    b = ((b & 0x0F0F0F0Fu) << 4u) | ((b & 0xF0F0F0F0u) >> 4u);
    b = ((b & 0x00FF00FFu) << 8u) | ((b & 0xFF00FF00u) >> 8u);
    return f32(b) * 2.3283064365386963e-10;
}

// GGX-filtered radiance around n (the split-sum assumption: view = normal), sampling the
// sharper levels at a lod that matches each sample's footprint.
fn prefilter(n: vec3<f32>) -> vec3<f32> {
    let rough = probe.p.y;
    let a = rough * rough;
    let up = select(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 0.0, 1.0), abs(n.z) < 0.999);
    let tx = normalize(cross(up, n));
    let ty = cross(n, tx);
    let count = 32u;
    let texel_sa = 4.0 * PI / (6.0 * probe.p.z * probe.p.z);
    var sum = vec3<f32>(0.0);
    var wsum = 0.0;
    for (var i = 0u; i < count; i = i + 1u) {
        let xi = vec2<f32>(f32(i) / f32(count), radical_inverse(i));
        let phi = 2.0 * PI * xi.x;
        let cos_t = sqrt((1.0 - xi.y) / (1.0 + (a * a - 1.0) * xi.y));
        let sin_t = sqrt(1.0 - cos_t * cos_t);
        let h = tx * (cos(phi) * sin_t) + ty * (sin(phi) * sin_t) + n * cos_t;
        let l = 2.0 * dot(n, h) * h - n;
        let nl = dot(n, l);
        if (nl <= 0.0) {
            continue;
        }
        let nh = max(dot(n, h), 0.0);
        let d2 = nh * nh * (a * a - 1.0) + 1.0;
        let dd = a * a / (PI * d2 * d2);
        let pdf = dd * 0.25;
        let sample_sa = 1.0 / (f32(count) * pdf + 1e-4);
        let lod = clamp(0.5 * log2(sample_sa / texel_sa) + 1.0, 0.0, probe.p.w - 1.0);
        sum = sum + textureSampleLevel(t_probe_src, s_lin, l, lod).rgb * nl;
        wsum = wsum + nl;
    }
    return sum / max(wsum, 1e-4);
}

@fragment
fn fs_probe_filter(in: FsIn) -> ProbeOut {
    let size = probe.p.z / pow(2.0, probe.p.w);
    let uv = in.clip.xy / size * 2.0 - vec2<f32>(1.0);
    let f0 = i32(probe.p.x);
    return ProbeOut(
        vec4<f32>(prefilter(face_dir(f0, uv.x, uv.y)), 1.0),
        vec4<f32>(prefilter(face_dir(f0 + 1, uv.x, uv.y)), 1.0),
        vec4<f32>(prefilter(face_dir(f0 + 2, uv.x, uv.y)), 1.0),
    );
}

// One face of the sky cube: the sky radiance (the table, the clouds, the air) divided by the
// table scale, and the cloud cover in alpha (the sun's disc is hidden by it).
@fragment
fn fs_sky_cube(in: FsIn) -> @location(0) vec4<f32> {
    sky_pass = true;
    let size = probe.p.z;
    let uv = in.clip.xy / size * 2.0 - vec2<f32>(1.0);
    let d = cube_to_world(face_dir(i32(probe.p.x), uv.x, uv.y));
    let pix = 1.4 / size;
    // a new start point for the steps each redraw (p.y counts the redraws): interleaved
    // gradient noise over the face, shifted by the golden ratio
    let ign = fract(52.9829189 * fract(dot(in.clip.xy, vec2<f32>(0.06711056, 0.00583715))));
    cloud_jitter = fract(ign + probe.p.y * 0.61803399);
    eye_off = enh.eye.xyz;
    var col = sky_table(d);
    let c = cloud_layer(d, col, pix);
    col = c.rgb;
    let h0 = camera.cam_pos.z + eye_off.z - enh.fog.z;
    let dist = 30000.0;
    let a = air_of(d, dist, h0, h0 + dist * max(d.z, 0.0), 0.0);
    col = col * a.a + a.rgb;
    return vec4<f32>(col / max(enh.ground.w, 1e-8), c.a);
}

// ---- [ROLLBACK twilight-1] Sunrise and sunset ---------------------------------------------
// What the sky model leaves faint at the low sun, drawn over it by angle: the earth's shadow
// rising in the east at dusk (and in the west at dawn) with the pink belt of Venus above it,
// the rays that fan out from the point opposite the sun, the flattened, rippling disc of the
// sun on the horizon (and, on a clean day, the green rim of its last light), the halo and the
// pillar of a sun seen through thin high cloud, and the glow under the clouds. Each strength is
// a number here, 1 as designed, 0 off.
const TW_SHADOW: f32 = 1.0;
const TW_BELT: f32 = 1.0;
const TW_RAYS: f32 = 1.0;
const TW_DISC: f32 = 1.0;
const TW_HALO: f32 = 1.0;
const TW_UNDER: f32 = 1.0;

fn tw_hash(x: f32) -> f32 {
    return fract(sin(x * 127.1 + 311.7) * 43758.5453);
}

fn tw_noise(x: f32) -> f32 {
    let i = floor(x);
    let f = fract(x);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(tw_hash(i), tw_hash(i + 1.0), u);
}

fn tw_lum(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

// The sky `col` towards d at the twilight. `cover` is how much cloud is in the way.
fn twilight_sky(d: vec3<f32>, col: vec3<f32>, cover: f32) -> vec3<f32> {
    let sd = normalize(camera.sun_dir.xyz);
    // how far below the horizon the sun is (rad; negative above it)
    let dep = -asin(clamp(sd.z, -1.0, 1.0));
    let hs = length(sd.xy);
    if (hs < 1e-3 || d.z < -0.02 || dep < -0.18 || dep > 0.2) {
        return col;
    }
    let ah = -sd.xy / hs;
    let dl = length(d.xy);
    var cd = 0.0;
    if (dl > 1e-4) {
        cd = dot(d.xy / dl, ah);
    }
    let el = asin(clamp(d.z, -1.0, 1.0));
    let clear = (1.0 - 0.85 * cover) * (1.0 - clamp(enh.fog.x * 1500.0, 0.0, 1.0));
    var c = col;
    // the shadow of the earth and the belt over it: only while the sun is just below the
    // horizon (from a degree above it to six or so below, then it is night's blue)
    let gate = smoothstep(-0.03, 0.012, dep) * (1.0 - smoothstep(0.07, 0.16, dep));
    if (gate > 0.003) {
        // its top: about one and a half times the sun's depression and a degree over the point
        // opposite the sun, lower to either side (it meets the horizon at ninety degrees off)
        let top = max(cd, 0.0) * (1.35 * max(dep + 0.012, 0.0) + 0.010);
        let soft = 0.006 + 0.30 * top;
        let shadow = (1.0 - smoothstep(top - soft, top + soft, el)) * smoothstep(0.0, 0.55, cd) * clear * gate * TW_SHADOW;
        c = mix(c, c * vec3<f32>(0.53, 0.62, 0.80), shadow * 0.85);
        let wb = 0.022 + 0.45 * top;
        let q = (el - (top + 0.9 * wb)) / wb;
        let belt = exp(-q * q) * smoothstep(-0.1, 0.6, cd) * clear * gate * TW_BELT;
        let pink = vec3<f32>(1.22, 0.80, 0.86) * tw_lum(c);
        c = mix(c, pink, clamp(belt * 0.5, 0.0, 0.6));
    }
    // the rays: from the point opposite the sun, where clouds scattered through the sky cast
    // their shadows far across it (and only then)
    let rays_gate = smoothstep(-0.17, -0.05, dep) * (1.0 - smoothstep(0.02, 0.09, dep));
    let patchy = smoothstep(0.1, 0.3, camera.clouds.x) * (1.0 - smoothstep(0.75, 0.95, camera.clouds.x));
    if (rays_gate * patchy > 0.003) {
        let anti = -sd;
        let e1 = normalize(cross(anti, vec3<f32>(0.0, 0.0, 1.0)) + vec3<f32>(1e-5, 0.0, 0.0));
        let e2 = cross(e1, anti);
        let w = dot(d, anti);
        if (w > 0.3) {
            let ang = atan2(dot(d, e1), dot(d, e2));
            // (a few bright rays between wider dim gaps, not an even fan)
            let n = tw_noise(ang * 4.0 + 3.7) * 0.65 + tw_noise(ang * 11.0 + 9.1) * 0.35;
            let ray = smoothstep(0.5, 0.9, n) - 0.5 * smoothstep(0.45, 0.15, n);
            let reach = smoothstep(0.3, 0.8, w) * (1.0 - smoothstep(0.35, 0.8, el));
            c = c * (1.0 + 0.07 * ray * reach * rays_gate * patchy * (1.0 - 0.5 * cover) * TW_RAYS);
        }
    }
    return c;
}

// How far from the middle of the sun's disc d is, as a share of its radius (x), and how high
// up its rim (y: -1 the lowest, +1 the highest point of the rim). Low in the sky the refraction
// flattens the disc by up to an eighth, and its rim ripples a little.
fn sun_disc_x(d: vec3<f32>, sd: vec3<f32>, cosang: f32, r: f32) -> vec2<f32> {
    let alt = asin(clamp(sd.z, -1.0, 1.0));
    let low = (1.0 - smoothstep(0.0, 0.12, alt)) * TW_DISC;
    let plain = clamp(acos(clamp(cosang, -1.0, 1.0)) / r, 0.0, 1.5);
    if (low < 0.01) {
        return vec2<f32>(plain, 0.0);
    }
    let upv = normalize(vec3<f32>(0.0, 0.0, 1.0) - sd * sd.z + vec3<f32>(1e-6, 0.0, 0.0));
    let sdh = cross(sd, upv);
    let dl = d - sd * cosang;
    let dv = dot(dl, upv);
    let dh = dot(dl, sdh);
    let flat_k = 1.0 - 0.13 * low;
    let x = length(vec2<f32>(dh, dv / flat_k)) / r;
    let ripple = 1.0 + 0.035 * low * low * sin(dv / r * 11.0 + camera.post.y * 2.3 + dh / r * 1.7);
    return vec2<f32>(clamp(x * ripple, 0.0, 1.5), dv / (r * flat_k));
}

// The last light of a setting (or the first of a rising) sun on a clean day: the green rim.
fn green_rim(l: vec3<f32>, sd: vec3<f32>, rim: f32) -> vec3<f32> {
    if (enh.twilight.x < 0.01) {
        return l;
    }
    let alt = asin(clamp(sd.z, -1.0, 1.0));
    let window = smoothstep(-0.017, -0.012, alt) * (1.0 - smoothstep(-0.008, -0.003, alt));
    let g = enh.twilight.x * window * smoothstep(0.45, 0.8, rim) * TW_DISC;
    return mix(l, vec3<f32>(0.05, 1.0, 0.55) * tw_lum(l) * 2.2, clamp(g, 0.0, 1.0));
}

// The 22 degree halo and the sun pillar of a sun behind thin high cloud (`enh.cloud_sun[0].w`,
// the veil's optical depth): the halo a faint ring, reddish inside, whitish-blue outside; the
// pillar a slender column of light over a low sun.
fn sun_halo_pillar(d: vec3<f32>, sd: vec3<f32>, col: vec3<f32>, cover: f32) -> vec3<f32> {
    let veil = enh.cloud_sun[0].w;
    if (veil < 0.03 || sd.z < -0.15) {
        return col;
    }
    let cosang = dot(d, sd);
    let thin = smoothstep(0.04, 0.2, veil) * (1.0 - smoothstep(0.8, 1.6, veil)) * (1.0 - cover) * TW_HALO;
    var c = col;
    if (thin > 0.003 && cosang > 0.8 && cosang < 0.96) {
        let ang = acos(clamp(cosang, -1.0, 1.0));
        let q = (ang - 0.384) / 0.016;
        let ring = exp(-q * q);
        let tint = mix(vec3<f32>(1.0, 0.72, 0.62), vec3<f32>(0.72, 0.84, 1.0), smoothstep(0.37, 0.40, ang));
        c = c + tint * tw_lum(c) * (0.26 * ring * thin);
    }
    // the pillar: a sun within eight degrees of the horizon, a few tenths of a degree wide
    let alt = asin(clamp(sd.z, -1.0, 1.0));
    let pg = smoothstep(0.17, 0.05, alt) * smoothstep(-0.12, -0.02, alt) * thin;
    if (pg > 0.003 && cosang > 0.9) {
        let upv = normalize(vec3<f32>(0.0, 0.0, 1.0) - sd * sd.z + vec3<f32>(1e-6, 0.0, 0.0));
        let sdh = cross(sd, upv);
        let dl = d - sd * cosang;
        let dv = dot(dl, upv);
        let dh = dot(dl, sdh);
        let along = select(exp(-(-dv) / 0.03), exp(-dv / 0.09), dv > 0.0);
        let pillar = exp(-(dh / 0.007) * (dh / 0.007)) * along * smoothstep(0.004, 0.02, abs(dv) + abs(dh));
        c = c + vec3<f32>(1.25, 0.95, 0.7) * tw_lum(c) * (0.5 * pillar * pg);
    }
    return c;
}

// The warm light from below that the horizon on the sun's side throws on the clouds' undersides.
fn cloud_underlight(d: vec3<f32>, sd: vec3<f32>, use_moon: bool) -> vec3<f32> {
    let low = (1.0 - smoothstep(0.02, 0.22, sd.z)) * smoothstep(-0.14, -0.02, sd.z) * select(1.0, 0.0, use_moon) * TW_UNDER;
    if (low < 0.01) {
        return vec3<f32>(0.0);
    }
    let hs = length(sd.xy);
    var hd = vec2<f32>(1.0, 0.0);
    if (hs > 1e-4) {
        hd = sd.xy / hs;
    }
    let glow = sky_table(normalize(vec3<f32>(hd.x, hd.y, 0.06)));
    var facing = 0.5;
    let dl = length(d.xy);
    if (dl > 1e-4) {
        facing = smoothstep(-0.4, 0.9, dot(d.xy / dl, hd));
    }
    return glow * (0.16 * low * facing);
}
