// The night sky's furniture, shared by the vanilla and the Enhanced sky: the stars (a
// catalogue of the 131 brightest, with their true places and colours, and a few thousand
// fainter ones made up on a grid), the Milky Way, three deep-sky smudges and the moon's face.
//
// The stars are fixed to the celestial sphere and the sphere turns with the clock: the
// direction d (x east, y north, z up) is turned into equatorial coordinates with the local
// sidereal time and the latitude (`camera.night`), the same sum as `omsi-sim/src/night.rs`
// (`local_to_equatorial`, tested there). The pole star therefore stands a latitude high and
// stays put, Orion rises in the east in winter, and the sky of one night is the sky of that
// night at that place.
//
// Brightness is in the picture's own units: a first-magnitude star peaks at about 1, the
// faintest star there is at 0.06; the caller scales it to whatever its target holds (the
// Enhanced sky divides by the exposure) and says how dark the sky is behind (`lim`, the
// faintest magnitude that shows: 6.8 in the country, 3 under a city's glow, none at dusk).

// camera.night: x local sidereal time (rad), y latitude (rad), z night-sky strength (the
// setting; 0 = no stars, no moon), w how far into the night the sky is (0 day .. 1 night)
// camera.moon:  xyz direction to the moon, w its angular radius (rad)

const STAR_COUNT: i32 = 131;
var<private> STAR_POS: array<vec4<f32>, 131> = array<vec4<f32>, 131>(
    vec4<f32>(-0.18725, 0.93924, -0.28769, -1.46), // Sirius
    vec4<f32>(-0.06334, 0.60267, -0.79547, -0.74), // Canopus
    vec4<f32>(-0.78371, -0.52713, 0.32854, -0.05), // Arcturus
    vec4<f32>(-0.37380, -0.31255, -0.87326, -0.01), // RigilKent
    vec4<f32>(0.12497, -0.76947, 0.62633, 0.03), // Vega
    vec4<f32>(0.13049, 0.68242, 0.71922, 0.08), // Capella
    vec4<f32>(0.19521, 0.97033, -0.14263, 0.13), // Rigel
    vec4<f32>(-0.41811, 0.90383, 0.09098, 0.34), // Procyon
    vec4<f32>(0.49269, 0.22375, -0.84094, 0.46), // Achernar
    vec4<f32>(0.02077, 0.99143, 0.12897, 0.50), // Betelgeuse
    vec4<f32>(-0.42400, -0.25426, -0.86924, 0.61), // Hadar
    vec4<f32>(0.45928, -0.87480, 0.15419, 0.76), // Altair
    vec4<f32>(-0.44939, -0.05239, -0.89180, 0.76), // Acrux
    vec4<f32>(0.34398, 0.89494, 0.28418, 0.85), // Aldebaran
    vec4<f32>(-0.34485, -0.82641, -0.44510, 0.96), // Antares
    vec4<f32>(-0.91407, -0.35638, -0.19355, 0.97), // Spica
    vec4<f32>(-0.39144, 0.79116, 0.46993, 1.14), // Pollux
    vec4<f32>(0.83740, -0.23341, -0.49425, 1.16), // Fomalhaut
    vec4<f32>(0.45558, -0.53625, 0.71055, 1.25), // Deneb
    vec4<f32>(-0.49379, -0.10428, -0.86331, 1.25), // Mimosa
    vec4<f32>(-0.86455, 0.45776, 0.20740, 1.35), // Regulus
    vec4<f32>(-0.22127, 0.84643, -0.48435, 1.50), // Adhara
    vec4<f32>(-0.34060, 0.77775, 0.52829, 1.58), // Castor
    vec4<f32>(-0.09167, -0.79230, -0.60321, 1.62), // Shaula
    vec4<f32>(-0.53800, -0.07370, -0.83971, 1.64), // Gacrux
    vec4<f32>(0.15076, 0.98236, 0.11060, 1.64), // Bellatrix
    vec4<f32>(0.12863, 0.86843, 0.47885, 1.65), // Elnath
    vec4<f32>(-0.25879, 0.23057, -0.93801, 1.67), // Miaplacidus
    vec4<f32>(0.10364, 0.99439, -0.02094, 1.69), // Alnilam
    vec4<f32>(0.60290, -0.31989, -0.73088, 1.74), // Alnair
    vec4<f32>(0.08363, 0.99592, -0.03385, 1.77), // Alnitak
    vec4<f32>(-0.54431, -0.13068, 0.82865, 1.77), // Alioth
    vec4<f32>(-0.36278, 0.57221, -0.73551, 1.78), // Regor
    vec4<f32>(-0.45911, 0.11511, 0.88089, 1.79), // Dubhe
    vec4<f32>(0.40504, 0.50152, 0.76447, 1.80), // Mirfak
    vec4<f32>(-0.26340, 0.85619, -0.44448, 1.83), // Wezen
    vec4<f32>(0.08698, -0.82071, -0.56468, 1.85), // KausAustralis
    vec4<f32>(-0.29554, 0.41243, -0.86172, 1.86), // Avior
    vec4<f32>(-0.58155, -0.29472, 0.75825, 1.86), // Alkaid
    vec4<f32>(-0.07233, -0.72789, -0.68187, 1.87), // Sargas
    vec4<f32>(0.00154, 0.70772, 0.70649, 1.90), // Menkalinan
    vec4<f32>(-0.10955, -0.34070, -0.93377, 1.92), // Atria
    vec4<f32>(-0.15709, 0.94636, 0.28234, 1.93), // Alhena
    vec4<f32>(0.32545, -0.44144, -0.83619, 1.94), // Peacock
    vec4<f32>(-0.38035, 0.43485, -0.81624, 1.96), // Alsephina
    vec4<f32>(-0.09407, 0.94661, -0.30835, 1.98), // Mirzam
    vec4<f32>(-0.77796, 0.61000, -0.15057, 1.98), // Alphard
    vec4<f32>(0.01018, 0.00794, 0.99992, 1.98), // Polaris
    vec4<f32>(0.77964, 0.48340, 0.39811, 2.00), // Hamal
    vec4<f32>(-0.85251, 0.39753, 0.33939, 2.01), // Algieba
    vec4<f32>(0.93395, 0.17985, -0.30885, 2.04), // Diphda
    vec4<f32>(0.21422, -0.87052, -0.44307, 2.05), // Nunki
    vec4<f32>(0.77559, 0.24343, 0.58241, 2.05), // Mirach
    vec4<f32>(-0.68526, -0.42281, -0.59300, 2.06), // Menkent
    vec4<f32>(0.87327, 0.03202, 0.48618, 2.06), // Alpheratz
    vec4<f32>(0.05245, 0.98440, -0.16797, 2.06), // Saiph
    vec4<f32>(-0.10668, -0.97022, 0.21746, 2.07), // Rasalhague
    vec4<f32>(-0.20068, -0.18502, 0.96203, 2.08), // Kochab
    vec4<f32>(0.63385, 0.38048, 0.67340, 2.10), // Almach
    vec4<f32>(0.51454, 0.55274, 0.65553, 2.12), // Algol
    vec4<f32>(-0.96675, 0.04601, 0.25156, 2.14), // Denebola
    vec4<f32>(-0.53112, 0.49528, -0.68747, 2.21), // Suhail
    vec4<f32>(-0.53641, -0.20591, 0.81845, 2.23), // Mizar
    vec4<f32>(0.44369, -0.62088, 0.64626, 2.23), // Sadr
    vec4<f32>(0.12187, 0.99253, -0.00524, 2.23), // Mintaka
    vec4<f32>(-0.00924, -0.62258, 0.78250, 2.23), // Eltanin
    vec4<f32>(-0.52884, -0.71993, 0.44947, 2.23), // Alphecca
    vec4<f32>(0.54277, 0.09693, 0.83427, 2.24), // Schedar
    vec4<f32>(-0.38714, 0.33329, -0.85967, 2.25), // Aspidiske
    vec4<f32>(0.51238, 0.02058, 0.85851, 2.28), // Caph
    vec4<f32>(-0.46049, -0.80001, -0.38462, 2.29), // Dschubba
    vec4<f32>(-0.53592, 0.13910, 0.83273, 2.37), // Merak
    vec4<f32>(-0.66948, -0.58712, 0.45508, 2.37), // Izar
    vec4<f32>(0.81722, -0.55019, 0.17159, 2.39), // Enif
    vec4<f32>(0.85590, -0.21419, 0.47070, 2.42), // Scheat
    vec4<f32>(-0.59194, 0.01602, 0.80582, 2.44), // Phecda
    vec4<f32>(0.35084, -0.29806, 0.88774, 2.45), // Alderamin
    vec4<f32>(-0.31288, 0.81401, -0.48938, 2.45), // Aludra
    vec4<f32>(0.55007, -0.62065, 0.55876, 2.46), // Gienah
    vec4<f32>(0.47419, 0.11977, 0.87224, 2.47), // GammaCas
    vec4<f32>(0.93712, -0.23018, 0.26236, 2.49), // Markab
    vec4<f32>(0.69819, 0.71235, 0.07132, 2.53), // Menkar
    vec4<f32>(-0.91783, 0.18632, 0.35053, 2.56), // Zosma
    vec4<f32>(0.23390, -0.83493, -0.49819, 2.60), // Ascella
    vec4<f32>(-0.64403, -0.74744, -0.16298, 2.61), // Zubeneschamali
    vec4<f32>(-0.45108, -0.82563, -0.33890, 2.62), // Graffias
    vec4<f32>(0.82032, 0.44818, 0.35527, 2.64), // Sheratan
    vec4<f32>(0.46199, 0.18152, 0.86811, 2.68), // Ruchbah
    vec4<f32>(-0.83250, -0.45531, 0.31565, 2.68), // Muphrid
    vec4<f32>(-0.10245, -0.78885, -0.60599, 2.69), // Lesath
    vec4<f32>(0.07938, -0.86387, -0.49743, 2.70), // KausMedia
    vec4<f32>(0.43972, -0.87906, 0.18412, 2.72), // Tarazed
    vec4<f32>(-0.98318, -0.18089, -0.02530, 2.74), // Porrima
    vec4<f32>(0.10656, 0.98896, -0.10297, 2.75), // Hatysa
    vec4<f32>(-0.70602, -0.65207, -0.27631, 2.75), // Zubenelgenubi
    vec4<f32>(-0.35533, -0.85996, 0.36634, 2.77), // Kornephoros
    vec4<f32>(-0.07876, -0.60643, 0.79122, 2.79), // Rastaban
    vec4<f32>(-0.51765, -0.03416, -0.85491, 2.80), // DeltaCru
    vec4<f32>(0.11007, -0.89645, -0.42925, 2.81), // KausBorealis
    vec4<f32>(-0.28676, -0.80200, 0.52399, 2.81), // ZetaHer
    vec4<f32>(0.96351, 0.05556, 0.26185, 2.83), // Algenib
    vec4<f32>(-0.94582, -0.26319, 0.19012, 2.83), // Vindemiatrix
    vec4<f32>(0.49880, 0.76442, 0.40849, 2.87), // Alcyone
    vec4<f32>(0.31203, -0.63274, 0.70871, 2.87), // DeltaCyg
    vec4<f32>(-0.09215, 0.91920, 0.38284, 2.88), // Tejat
    vec4<f32>(-0.76293, 0.50545, 0.40307, 2.97), // Algenubi
    vec4<f32>(0.02182, -0.86206, -0.50633, 2.99), // Alnasl
    vec4<f32>(-0.19971, -0.23949, 0.95014, 3.00), // Pherkad
    vec4<f32>(0.18065, 0.69854, 0.69240, 3.00), // Almaaz
    vec4<f32>(0.09102, 0.92825, 0.36065, 3.00), // ZetaTau
    vec4<f32>(-0.07656, 0.86211, -0.50091, 3.02), // Furud
    vec4<f32>(-0.61812, -0.48336, 0.61992, 3.03), // Seginus
    vec4<f32>(-0.17236, 0.88879, 0.42467, 3.06), // Mebsuta
    vec4<f32>(0.11888, -0.36103, 0.92494, 3.07), // Altais
    vec4<f32>(0.34051, -0.81500, 0.46886, 3.18), // Albireo
    vec4<f32>(0.17397, 0.73167, 0.65908, 3.18), // Haedus
    vec4<f32>(0.21335, -0.01932, 0.97678, 3.21), // Errai
    vec4<f32>(0.21392, -0.81396, 0.54009, 3.25), // Sulafat
    vec4<f32>(-0.54297, -0.03654, 0.83896, 3.31), // Megrez
    vec4<f32>(-0.94477, 0.19136, 0.26606, 3.33), // Chertan
    vec4<f32>(0.38942, 0.21232, 0.89625, 3.37), // Segin
    vec4<f32>(0.10681, 0.97921, 0.17244, 3.39), // Meissa
    vec4<f32>(0.18113, -0.81535, 0.54990, 3.52), // Sheliak
    vec4<f32>(0.36676, 0.87037, 0.32854, 3.53), // Ain
    vec4<f32>(0.49334, 0.76846, 0.40753, 3.63), // Atlas
    vec4<f32>(0.50743, 0.75871, 0.40849, 3.70), // Electra
    vec4<f32>(0.47912, -0.87062, 0.11164, 3.71), // Alshain
    vec4<f32>(0.50342, 0.75915, 0.41263, 3.87), // Maia
    vec4<f32>(0.50342, 0.76275, 0.40594, 4.18), // Merope
    vec4<f32>(0.50567, 0.75679, 0.41422, 4.30), // Taygeta
    vec4<f32>(-0.00720, -0.05904, 0.99823, 4.35), // Yildun
);
var<private> STAR_BV: array<f32, 131> = array<f32, 131>(
    0.00, 0.15, 1.23, 0.71, 0.00, 0.80, -0.03, 0.42, -0.16, 1.85, -0.23, 0.22, -0.24, 1.54, 1.83, -0.23, 1.00, 0.09, 0.09, -0.23, -0.11, -0.21, 0.03, -0.22, 1.59, -0.22, -0.13, 0.00, -0.18, -0.13, -0.21, -0.02, -0.26, 1.07, 0.48, 0.67, -0.03, 1.20, -0.19, 0.40, 0.03, 1.44, 0.00, -0.20, 0.04, -0.23, 1.44, 0.60, 1.15, 1.13, 1.02, -0.13, 1.58, 1.01, -0.11, -0.17, 0.16, 1.47, 1.37, -0.05, 0.09, 1.66, 0.02, 0.67, -0.22, 1.52, 0.03, 1.17, 0.18, 0.34, -0.12, -0.02, 0.97, 1.52, 1.67, 0.00, 0.22, -0.08, 1.03, -0.15, -0.04, 1.63, 0.12, 0.08, -0.11, -0.07, 0.13, 0.13, 0.58, -0.22, 1.38, 1.52, 0.36, -0.21, 0.15, 0.94, 1.17, -0.25, 1.04, 0.65, -0.19, 0.94, -0.09, -0.03, 1.64, 0.81, 1.00, 0.05, 0.54, -0.19, -0.19, 0.19, 1.40, 0.98, 1.13, 1.50, 1.03, -0.05, 0.08, 0.00, -0.15, -0.18, 0.00, 1.01, -0.08, -0.11, 0.86, -0.07, -0.06, -0.10, 0.11
);

// The galactic frame in equatorial coordinates: the north pole N, the centre C, and Y = N x C
// (the direction of galactic longitude 90 degrees).
const GAL_N: vec3<f32> = vec3<f32>(-0.86767, -0.19808, 0.45598);
const GAL_C: vec3<f32> = vec3<f32>(-0.05488, -0.87344, -0.48383);
const GAL_Y: vec3<f32> = vec3<f32>(0.49411, -0.44483, 0.74698);

// Equatorial unit vector of the local direction d (x east, y north, z up).
fn celestial_of(d: vec3<f32>) -> vec3<f32> {
    let lst = camera.night.x;
    let lat = camera.night.y;
    let sl = sin(lat);
    let cl = cos(lat);
    let st = sin(lst);
    let ct = cos(lst);
    // cos(dec) cos(h) and cos(dec) sin(h), h the hour angle
    let a = -d.y * sl + d.z * cl;
    let b = -d.x;
    return vec3<f32>(ct * a + st * b, st * a - ct * b, d.y * cl + d.z * sl);
}

// A hash of a lattice point, from its integer bits (see `hash2`).
fn hash3(c: vec3<i32>) -> f32 {
    var h = (bitcast<u32>(c.x) * 0x8da6b343u) ^ (bitcast<u32>(c.y) * 0xd8163841u) ^ (bitcast<u32>(c.z) * 0xcb1ab31fu);
    h = h ^ (h >> 13u);
    h = h * 0x5bd1e995u;
    h = h ^ (h >> 15u);
    return f32(h & 0xffffu) / 65535.0;
}

fn vnoise3(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let c = vec3<i32>(i);
    let x00 = mix(hash3(c), hash3(c + vec3<i32>(1, 0, 0)), u.x);
    let x10 = mix(hash3(c + vec3<i32>(0, 1, 0)), hash3(c + vec3<i32>(1, 1, 0)), u.x);
    let x01 = mix(hash3(c + vec3<i32>(0, 0, 1)), hash3(c + vec3<i32>(1, 0, 1)), u.x);
    let x11 = mix(hash3(c + vec3<i32>(0, 1, 1)), hash3(c + vec3<i32>(1, 1, 1)), u.x);
    return mix(mix(x00, x10, u.y), mix(x01, x11, u.y), u.z);
}

fn fbm3(p: vec3<f32>) -> f32 {
    var v = 0.0;
    var a = 0.5;
    var q = p;
    for (var i = 0; i < 4; i = i + 1) {
        v = v + a * vnoise3(q);
        q = q * 2.03 + vec3<f32>(11.7, 5.3, 8.1);
        a = a * 0.5;
    }
    return v;
}

// Four hashes of a lattice cell (stars and moon craters).
fn star_hash(c: vec3<i32>) -> vec4<f32> {
    var x = vec4<u32>(bitcast<vec3<u32>>(c), 0x9e3779b9u).xyzw;
    x = x * 1664525u + 1013904223u;
    x.x = x.x + x.y * x.w;
    x.y = x.y + x.z * x.x;
    x.z = x.z + x.x * x.y;
    x.w = x.w + x.y * x.z;
    x = x ^ (x >> vec4<u32>(16u));
    x.x = x.x + x.y * x.w;
    x.y = x.y + x.z * x.x;
    x.z = x.z + x.x * x.y;
    x.w = x.w + x.y * x.z;
    return vec4<f32>(x & vec4<u32>(0xffffffu)) / 16777216.0;
}

// The colour of a star of colour index B-V: the black body of its temperature (Ballesteros'
// relation, then Tanner Helland's fit of the Planckian locus), washed out a quarter towards
// white, because that is how the eye sees a point of light.
fn bv_colour(bv: f32) -> vec3<f32> {
    let t = 4600.0 * (1.0 / (0.92 * bv + 1.7) + 1.0 / (0.92 * bv + 0.62));
    let k = clamp(t, 2500.0, 12000.0) / 100.0;
    var c = vec3<f32>(1.0);
    if (k <= 66.0) {
        c.g = clamp((99.4708025861 * log(k) - 161.1195681661) / 255.0, 0.0, 1.0);
        c.b = clamp((138.5177312231 * log(max(k - 10.0, 1.0)) - 305.0447927307) / 255.0, 0.0, 1.0);
    } else {
        c.r = clamp(329.698727446 * pow(k - 60.0, -0.1332047592) / 255.0, 0.0, 1.0);
        c.g = clamp(288.1221695283 * pow(k - 60.0, -0.0755148492) / 255.0, 0.0, 1.0);
    }
    c = c / max(max(c.r, c.g), max(c.b, 1e-3));
    return mix(vec3<f32>(1.0), c, 0.75);
}

// The light of a star of magnitude m, 1 for Sirius. The real ratio is 2.512 a magnitude;
// the picture takes a good part of that (a fifth of a decade a magnitude), so that the
// faint ones are still there next to the bright ones, as they are to the dark-adapted eye.
fn star_flux(m: f32) -> f32 {
    return pow(10.0, -0.16 * (m + 1.46));
}

// How much of a star's light the air lets through towards a height z over the horizon:
// the long way through the air near the horizon dims a star and reddens it.
fn star_extinction(z: f32) -> f32 {
    return clamp(exp(-0.3 / (max(z, 0.0) + 0.12)) * 1.3, 0.0, 1.0) * smoothstep(-0.01, 0.04, z);
}

// The twinkle: the air bends the starlight about, more of it near the horizon.
fn twinkle(seed: f32, z: f32, time: f32) -> f32 {
    let amp = 0.06 + 0.25 * (1.0 - clamp(z, 0.0, 1.0)) * (1.0 - clamp(z, 0.0, 1.0));
    return 1.0 + amp * (0.6 * sin(time * 7.3 + seed * 61.0) + 0.4 * sin(time * 13.1 + seed * 23.0));
}

// One star: a round spot about a pixel across (never thinner, or it would flicker in and out
// between the pixels as the view turns), a faint halo round the bright ones.
fn star_spot(ang: f32, flux: f32, pix: f32) -> f32 {
    let sig = max(pix * 0.9, 3e-4);
    var s = exp(-ang * ang / (sig * sig));
    if (flux > 0.3) {
        s = s + 0.18 * flux * exp(-ang * ang / (sig * sig * 20.0));
    }
    return s;
}

const PSTAR_N: f32 = 64.0;

// A point on face `face` (0..5: +x -x +y -y +z -z) of the cube round the sphere, at
// (u, v) on it.
fn face_point(face: i32, uv: vec2<f32>) -> vec3<f32> {
    let s = select(1.0, -1.0, (face & 1) == 1);
    let ax = face >> 1;
    var p = vec3<f32>(s, uv.x, uv.y);
    if (ax == 1) {
        p = vec3<f32>(uv.x, s, uv.y);
    } else if (ax == 2) {
        p = vec3<f32>(uv.x, uv.y, s);
    }
    return normalize(p);
}

// The made-up stars, fainter than the catalogue: on each face of a cube round the sphere a
// grid of cells, one star (or none) in a cell, and the pixel looks at the nine cells round
// it, so that a star near its cell's edge is not cut off. More of them along the Milky Way.
// None within 4% of the cube's edges, so that none is cut there either.
fn made_up_stars(e: vec3<f32>, pix: f32, lim: f32, z: f32, time: f32) -> vec3<f32> {
    var col = vec3<f32>(0.0);
    if (lim < 3.9) {
        return col;
    }
    let a = abs(e);
    var face = 0;
    var along = e.x;
    var uv = vec2<f32>(e.y, e.z) / max(a.x, 1e-4);
    if (a.y > a.x && a.y >= a.z) {
        face = 2;
        along = e.y;
        uv = vec2<f32>(e.x, e.z) / max(a.y, 1e-4);
    } else if (a.z > a.x && a.z > a.y) {
        face = 4;
        along = e.z;
        uv = vec2<f32>(e.x, e.y) / max(a.z, 1e-4);
    }
    if (along < 0.0) {
        face = face + 1;
    }
    let cell = floor((uv * 0.5 + vec2<f32>(0.5)) * PSTAR_N);
    for (var dj = -1; dj <= 1; dj = dj + 1) {
        for (var di = -1; di <= 1; di = di + 1) {
            let cc = cell + vec2<f32>(f32(di), f32(dj));
            if (cc.x < 0.0 || cc.y < 0.0 || cc.x >= PSTAR_N || cc.y >= PSTAR_N) {
                continue;
            }
            let h = star_hash(vec3<i32>(i32(cc.x), i32(cc.y), face + 17));
            let puv = ((cc + vec2<f32>(0.2) + h.yz * 0.6) / PSTAR_N) * 2.0 - vec2<f32>(1.0);
            if (max(abs(puv.x), abs(puv.y)) > 0.96) {
                continue;
            }
            let p = face_point(face, puv);
            let gb = dot(p, GAL_N);
            // (the cells are the smaller on the sphere the nearer the cube's corners: the odds
            // come down by the same measure, so that the stars are as dense everywhere)
            let even = pow(1.0 + dot(puv, puv), -1.5) * 1.9;
            if (h.x > 0.2 * even * (0.5 + 2.0 * exp(-gb * gb * 14.0))) {
                continue;
            }
            // magnitudes 3.9 .. 6.9, many more of the faint than of the bright
            let m = 3.9 + log2(1.0 + h.w * 21.4) * 0.669;
            let vis = clamp(lim - m + 0.5, 0.0, 1.0);
            if (vis <= 0.0) {
                continue;
            }
            let ang = length(cross(e, p));
            if (ang > max(0.012, pix * 6.0)) {
                continue;
            }
            let hh = star_hash(vec3<i32>(i32(cc.x), i32(cc.y), face + 91));
            let tint = bv_colour(-0.2 + 1.9 * pow(hh.x, 1.7));
            col = col + tint * star_flux(m) * star_spot(ang, 0.0, pix) * vis * twinkle(hh.y, z, time);
        }
    }
    return col;
}

// The catalogue stars: the 131 brightest, as seen from here.
fn catalogue_stars(e: vec3<f32>, pix: f32, lim: f32, z: f32, time: f32) -> vec3<f32> {
    var col = vec3<f32>(0.0);
    for (var i = 0; i < STAR_COUNT; i = i + 1) {
        let sp = STAR_POS[i];
        // (a star is a pixel across: not worth a second look from further than a degree)
        let reach = max(0.02, pix * 6.0);
        if (dot(e, sp.xyz) < 1.0 - 0.5 * reach * reach) {
            continue;
        }
        let vis = clamp(lim - sp.w + 0.5, 0.0, 1.0);
        if (vis <= 0.0) {
            continue;
        }
        let ang = length(cross(e, sp.xyz));
        let flux = star_flux(sp.w);
        col = col + bv_colour(STAR_BV[i]) * flux * star_spot(ang, flux, pix) * vis * twinkle(f32(i) * 0.618, z, time);
    }
    return col;
}

// The Milky Way: a band of unresolved stars along the galactic equator, thick and bright
// towards the centre (in Sagittarius and Scorpius), broken by lanes of dust (the Great Rift
// from Cygnus south) and grainy. Display units: the brightest part is about 0.1.
fn milky_way(e: vec3<f32>) -> vec3<f32> {
    let sb = dot(e, GAL_N);
    let l = atan2(dot(e, GAL_Y), dot(e, GAL_C));
    let bulge = exp(-l * l / (2.0 * 0.55 * 0.55));
    let width = mix(0.085, 0.17, bulge);
    let disk = exp(-sb * sb / (2.0 * width * width));
    let patches = fbm3(e * 5.0 + vec3<f32>(3.0, 1.0, 7.0));
    // dust lanes cut along the middle of the band, mostly on the side of the centre
    let lane_zone = smoothstep(-0.5, 0.4, cos(l - 0.5)) ;
    let lane = smoothstep(0.42, 0.62, fbm3(e * 11.0 + vec3<f32>(9.0, 2.0, 4.0))) * exp(-(sb - 0.015) * (sb - 0.015) / (2.0 * 0.045 * 0.045)) * lane_zone;
    let grain = 0.8 + 0.4 * vnoise3(e * 70.0);
    // (faint: a few thousandths of white away from the centre, a few hundredths at most in it)
    let base = 0.004 + 0.007 * bulge;
    let amount = base * disk * (0.55 + 0.9 * patches) * grain * (1.0 - 0.8 * lane);
    // a warm core, bluer arms
    let tint = mix(vec3<f32>(0.78, 0.88, 1.0), vec3<f32>(1.0, 0.86, 0.64), bulge);
    return tint * max(amount, 0.0);
}

// A smudge on the sky: a round glow of angular size `r` (rad) round the direction c.
fn smudge(e: vec3<f32>, c: vec3<f32>, r: f32, level: f32) -> f32 {
    let a = length(cross(e, c));
    return level * exp(-a * a / (r * r));
}

// The deep sky the naked eye can find: the Andromeda galaxy, the Large and the Small
// Magellanic Cloud.
fn deep_sky(e: vec3<f32>) -> vec3<f32> {
    let m31 = smudge(e, vec3<f32>(0.73860, 0.13923, 0.65961), 0.010, 0.030) + smudge(e, vec3<f32>(0.73860, 0.13923, 0.65961), 0.030, 0.008);
    let lmc = smudge(e, vec3<f32>(0.05477, 0.34176, -0.93819), 0.05, 0.010);
    let smc = smudge(e, vec3<f32>(0.28793, 0.06737, -0.95528), 0.02, 0.010);
    return vec3<f32>(0.85, 0.9, 1.0) * m31 + vec3<f32>(0.9, 0.95, 1.0) * (lmc + smc);
}

// ---- [ROLLBACK meteors-1] Shooting stars --------------------------------------------------
// Now and then a meteor crosses the sky: three independent lanes, each cutting the time into
// slots of 47, 76 and 105 seconds, in about two slots in five a meteor is drawn at a random
// place, in a random direction (mostly downward), 10-25 degrees long in half a second to a
// second and a half: a bright head with a fading trail behind it. About one a minute in all.
fn mt_rand(a: f32, b: f32) -> f32 {
    return fract(sin(dot(vec2<f32>(a, b), vec2<f32>(12.9898, 78.233))) * 43758.5453);
}

fn meteor_lane(d: vec3<f32>, pix: f32, time: f32, lane: f32) -> vec3<f32> {
    let period = 47.0 + 29.0 * lane;
    let sl = time / period + lane * 0.37;
    let n = floor(sl);
    let key = n + lane * 131.0;
    if (mt_rand(key, 1.0) > 0.42) {
        return vec3<f32>(0.0);
    }
    let life = 0.5 + 1.0 * mt_rand(key, 2.0);
    let u0 = 0.5 + (period - life - 1.0) * mt_rand(key, 3.0);
    let a = ((sl - n) * period - u0) / life;
    if (a < 0.0 || a > 1.0) {
        return vec3<f32>(0.0);
    }
    let az = 6.2832 * mt_rand(key, 4.0);
    let el = 0.35 + 1.0 * mt_rand(key, 5.0);
    let p0 = vec3<f32>(cos(el) * sin(az), cos(el) * cos(az), sin(el));
    let len = 0.17 + 0.26 * mt_rand(key, 6.0);
    if (dot(d, p0) < cos(len + 0.25)) {
        return vec3<f32>(0.0);
    }
    // the path: along the sky from p0 in a direction of the tangent plane, downward
    let t1 = normalize(cross(vec3<f32>(0.0, 0.0, 1.0), p0));
    let t2 = cross(p0, t1);
    let w = 6.2832 * mt_rand(key, 8.0);
    var v = cos(w) * t1 + sin(w) * t2;
    if (v.z > 0.0) {
        v = -v;
    }
    let across = dot(d, cross(p0, v));
    let along = atan2(dot(d, v), dot(d, p0));
    let head = len * a;
    let back = head - along;
    let trail = 0.35 * len;
    if (back < -2.0 * pix || back > trail) {
        return vec3<f32>(0.0);
    }
    let sig = max(pix * 0.9, 3e-4);
    let prof = exp(-across * across / (sig * sig));
    let tail = pow(clamp(1.0 - back / trail, 0.0, 1.0), 2.0) * select(0.0, 0.7, back > 0.0);
    let flare = exp(-back * back / (4.0 * sig * sig));
    // (it flares and dies, it does not blink on and off)
    let env = pow(sin(3.1416 * a), 0.6);
    let peak = 0.9 + 1.4 * mt_rand(key, 7.0);
    let tint = mix(vec3<f32>(0.80, 0.93, 1.0), vec3<f32>(1.0, 0.88, 0.68), mt_rand(key, 9.0));
    return tint * prof * (flare + tail) * peak * env;
}

fn meteors(d: vec3<f32>, pix: f32, time: f32) -> vec3<f32> {
    return meteor_lane(d, pix, time, 0.0) + meteor_lane(d, pix, time, 1.0) + meteor_lane(d, pix, time, 2.0);
}

// All the stars towards d. `lim`: the faintest magnitude that shows against the sky behind
// (about 6.8 for a dark one); `pix`: the angle a pixel spans; `time`: the clock for the
// twinkle. Nothing below the horizon.
fn night_stars(d: vec3<f32>, pix: f32, lim: f32, time: f32) -> vec3<f32> {
    if (d.z < -0.02 || lim < 0.5) {
        return vec3<f32>(0.0);
    }
    let e = celestial_of(d);
    let ext = star_extinction(d.z);
    var col = catalogue_stars(e, pix, lim, d.z, time) + made_up_stars(e, pix, lim, d.z, time);
    // [ROLLBACK meteors-1] a shooting star now and then (bright: it shows through a hazier sky than the faint stars)
    if (lim > 2.5) {
        col = col + meteors(d, pix, time) * smoothstep(2.5, 4.5, lim);
    }
    // the diffuse light is only for the darkest skies
    let dark = smoothstep(4.8, 6.4, lim);
    if (dark > 0.0) {
        col = col + (milky_way(e) + deep_sky(e)) * dark;
    }
    // near the horizon the air reddens what it lets through
    return col * ext * mix(vec3<f32>(1.0, 0.72, 0.5), vec3<f32>(1.0), sqrt(ext));
}

// The albedo of the moon's face at q (the disc is the unit circle, +x the right, +y up as
// seen from the northern hemisphere with the moon in the south): the dark seas on the
// bright highlands, the bright ray craters, a grain of regolith. 1 is a highland.
fn moon_blob(q: vec2<f32>, c: vec2<f32>, rx: f32, ry: f32, rot: f32) -> f32 {
    let s = sin(rot);
    let k = cos(rot);
    let p = q - c;
    let x = (p.x * k + p.y * s) / rx;
    let y = (-p.x * s + p.y * k) / ry;
    return 1.0 - smoothstep(0.62, 1.0, sqrt(x * x + y * y));
}

fn moon_albedo(q: vec2<f32>) -> f32 {
    // the edges of the seas are ragged
    let w = vec2<f32>(cloud_fbm(q * 4.0 + vec2<f32>(1.3, 5.1)), cloud_fbm(q * 4.0 + vec2<f32>(8.2, 2.7))) - vec2<f32>(0.47);
    let p = q + w * 0.11;
    var sea = 0.0;
    // (places from the selenographic latitude and longitude: x = cos(lat) sin(lon), y = sin(lat))
    sea = max(sea, moon_blob(p, vec2<f32>(-0.27, 0.50), 0.38, 0.30, 0.25));    // Imbrium
    sea = max(sea, moon_blob(p, vec2<f32>(0.26, 0.47), 0.22, 0.19, -0.2));     // Serenitatis
    sea = max(sea, moon_blob(p, vec2<f32>(0.47, 0.14), 0.31, 0.24, 0.35));     // Tranquillitatis
    sea = max(sea, moon_blob(p, vec2<f32>(0.80, 0.29), 0.09, 0.12, 0.0));      // Crisium
    sea = max(sea, moon_blob(p, vec2<f32>(0.76, -0.13), 0.13, 0.20, -0.2));    // Fecunditatis
    sea = max(sea, moon_blob(p, vec2<f32>(0.55, -0.26), 0.08, 0.10, 0.0));     // Nectaris
    sea = max(sea, moon_blob(p, vec2<f32>(-0.64, 0.10), 0.27, 0.50, 0.15));    // Oceanus Procellarum
    sea = max(sea, moon_blob(p, vec2<f32>(-0.27, -0.36), 0.24, 0.16, 0.1));    // Nubium
    sea = max(sea, moon_blob(p, vec2<f32>(-0.38, -0.17), 0.13, 0.12, 0.0));    // Cognitum
    sea = max(sea, moon_blob(p, vec2<f32>(-0.57, -0.41), 0.10, 0.10, 0.0));    // Humorum
    sea = max(sea, moon_blob(p, vec2<f32>(0.01, 0.83), 0.42, 0.06, 0.0));      // Frigoris
    sea = max(sea, moon_blob(p, vec2<f32>(0.02, 0.23), 0.10, 0.09, 0.0));      // Vaporum
    sea = max(sea, moon_blob(p, vec2<f32>(-0.14, 0.21), 0.10, 0.09, 0.0));     // Aestuum
    // the grain of the regolith and the large craters
    let grain = 0.86 + 0.28 * cloud_fbm(q * 11.0 + vec2<f32>(2.0, 3.0));
    var a = mix(1.0, 0.5, sea) * grain;
    // Tycho, with its rays; Copernicus, Kepler and Aristarchus, bright spots
    let t = q - vec2<f32>(-0.14, -0.68);
    let td = length(t);
    let rays = pow(abs(sin(atan2(t.y, t.x) * 5.0 + 5.0 * cloud_fbm(q * 5.0))), 8.0) * exp(-td * 2.2) * smoothstep(0.03, 0.12, td);
    a = a + 0.26 * rays + 0.6 * exp(-td * td / (0.03 * 0.03));
    a = a + 0.45 * exp(-dot(q - vec2<f32>(-0.34, 0.17), q - vec2<f32>(-0.34, 0.17)) / (0.03 * 0.03));
    a = a + 0.3 * exp(-dot(q - vec2<f32>(-0.61, 0.14), q - vec2<f32>(-0.61, 0.14)) / (0.022 * 0.022));
    a = a + 0.75 * exp(-dot(q - vec2<f32>(-0.67, 0.40), q - vec2<f32>(-0.67, 0.40)) / (0.025 * 0.025));
    // a few dark-floored craters
    a = a - 0.3 * exp(-dot(q - vec2<f32>(-0.10, 0.78), q - vec2<f32>(-0.10, 0.78)) / (0.035 * 0.035));
    return clamp(a, 0.25, 1.6);
}

// The moon's disc towards d, for a moon in direction m of angular radius r and the sun in
// direction s: x is the surface's brightness against the full moon's highlands (1), with
// the sun's light on it as the phase gives it, and the earthshine on the dark side; y is
// how much of the pixel the disc covers; z how much of the surface here is lit by the sun.
// The moon is a sphere: the lit side faces the sun, so the crescent opens the right way
// on the sky. Its face is turned as it is for a viewer here (north up when it stands on
// the meridian, tilted when it rises or sets), and it has no limb darkening: a full moon
// is a flat disc, since the surface scatters back towards the sun.
fn moon_surface(d: vec3<f32>, m: vec3<f32>, r: f32, s: vec3<f32>, pix: f32) -> vec3<f32> {
    let pole = vec3<f32>(0.0, cos(camera.night.y), sin(camera.night.y));
    var up = pole - m * dot(pole, m);
    if (dot(up, up) < 1e-4) {
        up = vec3<f32>(0.0, 0.0, 1.0) - m * m.z;
    }
    up = normalize(up);
    let right = cross(m, up);
    let q = vec2<f32>(dot(d, right), dot(d, up)) / r;
    let rad = length(q);
    let aa = clamp(pix / r, 0.02, 0.5);
    let cover = 1.0 - smoothstep(1.0 - aa, 1.0 + aa, rad);
    if (cover <= 0.0) {
        return vec3<f32>(0.0);
    }
    let qc = q / max(rad, 1.0);
    let nz = sqrt(max(1.0 - dot(qc, qc), 0.0));
    let n = normalize(right * qc.x + up * qc.y - m * nz);
    let ndl = max(dot(n, s), 0.0);
    // Lommel-Seeliger: the brightness of a rough surface lit at an angle, 1 at the full moon
    let ls = 2.0 * ndl / (ndl + nz + 1e-3);
    let lit_share = clamp(0.5 + 0.5 * dot(-m, s), 0.0, 1.0);
    let earth = 0.02 * nz * (1.0 - lit_share);
    let alb = moon_albedo(qc);
    return vec3<f32>((ls + earth) * alb, cover, smoothstep(0.0, 0.4, ls));
}

// The vanilla sky's night: the envir.cfg picture `base` with the stars, the Milky Way and the
// moon in it. (The stock sky is a gradient with nothing on it.) Stars show as the sky
// behind them is dark: none by day, none in the twilight, all of them in a clear night, fewer
// round a bright moon; a thick fog hides them later, in `fs_main`. The moon is there by day
// too, its lit side a pale disc in the blue.
fn vanilla_night(d: vec3<f32>, base: vec3<f32>, pix: f32) -> vec3<f32> {
    let strength = camera.night.z;
    if (strength <= 0.0 || d.z < -0.02) {
        return base;
    }
    var col = base;
    let time = camera.post.y;
    let night = camera.night.w;
    let sun = normalize(camera.sun_dir.xyz);
    let m = normalize(camera.moon.xyz + vec3<f32>(0.0, 0.0, -1e-6));
    let lit_share = clamp(0.5 + 0.5 * dot(-m, sun), 0.0, 1.0);
    let moon_up = smoothstep(-0.05, 0.15, m.z);
    let ang_m = acos(clamp(dot(d, m), -1.0, 1.0));
    // the faintest star that shows: 6.8 in a dark night, less as the twilight's light and
    // the moon's glare fill the sky
    let dusk = smoothstep(0.55, 1.0, camera.sky.w);
    let glare = moon_up * (2.2 * pow(lit_share, 1.6) + 3.0 * pow(lit_share, 1.3) * exp(-ang_m * 5.0));
    let lim = 6.8 * dusk - glare * dusk;
    if (lim > 0.5) {
        col = col + night_stars(d, pix, lim, time) * min(strength, 1.5);
    }
    // the moon's glow, then the disc itself (opaque: it hides the stars behind it)
    if (moon_up > 0.0) {
        // (the disc only where the moon is: not at the opposite point of the sky, where the
        // face's coordinates are zero as well)
        let tint = vec3<f32>(1.0, 0.96, 0.88);
        let halo = (0.09 * exp(-ang_m / 0.012) + 0.022 * exp(-ang_m / 0.09)) * pow(lit_share, 1.5) * moon_up;
        col = col + tint * halo * mix(0.35, 1.0, night);
        var ms = vec3<f32>(0.0);
        if (dot(d, m) > cos(camera.moon.w * 1.5)) {
            ms = moon_surface(d, m, camera.moon.w, sun, pix);
        }
        if (ms.y > 0.0) {
            let face = tint * min(ms.x, 1.35) * 0.92;
            // by night the dark side blots out the sky; by day it is the sky
            let dim = mix(1.0, 0.55, night);
            let on = face + col * (1.0 - ms.z) * dim;
            col = mix(col, on, ms.y * moon_up);
        }
    }
    return col;
}

// ---- [ROLLBACK aurora-1] The aurora ------------------------------------------------------
// Curtains of light 90-330 km up over the polar horizon (the kilometres are real: a ray is
// followed up through the layer, so the curtains have their parallax and their rays stand
// along the leaning field lines, converging on the magnetic zenith). The arc lies `dist` km
// towards the pole of the observer, its middle nearest. [ROLLBACK aurora-3] Each event has a
// look of its own, drawn from its seed (`au_event`): the direction of its arcs, one to three
// of them, thick or thin, calm or lively, with their own rhythm of break-ups (a quick
// brightening that runs along the arc and dies away) and a slow drift. Colours by height:
// the green of atomic oxygen (557.7 nm, the bulk, 100-150 km) alone up to a strength of 100 %;
// above it the red of its upper levels (630 nm, over 150 km) and the violet-pink lower edge
// (nitrogen, under 100 km) come in. `amp_s` is the strength (negative: the southern sky),
// `dist` the arc's distance in km. The result is in display units (the caller divides by the
// exposure).
struct AuEvent {
    ang: f32,
    w1: f32,
    w2: f32,
    g1: f32,
    g2: f32,
    wid: f32,
    act: f32,
    period: f32,
    phase: f32,
    redness: f32,
    pink: f32,
    ph: f32,
}

fn au_rand(q: f32, salt: f32) -> f32 {
    return fract(sin(dot(vec2<f32>(q + 0.5, salt * 7.31 + 1.7), vec2<f32>(12.9898, 78.233))) * 43758.5453);
}

fn au_event(q: f32) -> AuEvent {
    var e: AuEvent;
    // the arc's direction: up to 30 degrees off the pole, either way
    e.ang = (au_rand(q, 1.0) - 0.5) * 1.1;
    // one arc in about a third of the events, two in another, three in the rest
    let n = au_rand(q, 2.0);
    e.w1 = select(0.12, 0.6, n > 0.3);
    e.w2 = select(0.0, 0.42, n > 0.72);
    e.g1 = 80.0 + 90.0 * au_rand(q, 3.0);
    e.g2 = e.g1 + 70.0 + 80.0 * au_rand(q, 4.0);
    // thin and sharp .. thick and soft; calm .. lively
    e.wid = 0.7 + 0.9 * au_rand(q, 5.0);
    e.act = 0.65 + 0.8 * au_rand(q, 6.0);
    // a break-up every 75-165 s
    e.period = 75.0 + 90.0 * au_rand(q, 7.0);
    e.phase = e.period * au_rand(q, 8.0);
    e.redness = 0.6 + 0.6 * au_rand(q, 9.0);
    e.pink = au_rand(q, 10.0);
    e.ph = 6.2832 * au_rand(q, 11.0);
    return e;
}

// The break-up of the moment: x how strong (0 .. 1, a quick rise and a fall over a quarter
// of a minute; only in the strong displays), y the seconds since it began, z where along the
// arc it began (km).
fn au_burst(e: AuEvent, time: f32, amp: f32) -> vec3<f32> {
    let c = (time + e.phase) / e.period;
    let n = floor(c);
    let bt = (c - n) * e.period;
    let gain = smoothstep(0.45, 1.0, amp);
    let b = smoothstep(0.0, 2.0, bt) * exp(-bt / 16.0) * gain;
    return vec3<f32>(b, bt, (au_rand(n + e.ph, 12.0) - 0.5) * 500.0);
}

fn au_shell(dz: f32, h: f32) -> f32 {
    let b = 6371.0 * dz;
    return -b + sqrt(b * b + 12742.0 * h + h * h);
}

fn au_height_colour(h: f32, hot: f32, e: AuEvent) -> vec3<f32> {
    let green = smoothstep(86.0, 102.0, h) * exp(-max(h - 108.0, 0.0) / 38.0);
    let red = smoothstep(150.0, 235.0, h) * exp(-max(h - 235.0, 0.0) / 90.0) * (1.0 - smoothstep(240.0, 320.0, h)) * 0.5 * hot * e.redness;
    // (the lower edge's band is wide enough for two or three of the 26 steps: no speckle)
    let edge = smoothstep(84.0, 92.0, h) * (1.0 - smoothstep(96.0, 118.0, h)) * 0.8 * hot;
    let fringe = mix(vec3<f32>(0.85, 0.12, 0.70), vec3<f32>(1.0, 0.28, 0.52), e.pink);
    return vec3<f32>(0.10, 1.0, 0.30) * green + vec3<f32>(1.0, 0.07, 0.10) * red + fringe * edge;
}

fn au_sheet(x: f32, y: f32, dist: f32, time: f32, k: f32, off: f32, e: AuEvent, bu: vec3<f32>) -> f32 {
    let tf = time * e.act;
    // the middle line: the oval's curve (the arc bends away over the east and west), a slow
    // drift of the whole and the folds that travel along it (livelier in a break-up)
    let centre = dist + off + x * x / 5600.0 + 30.0 * sin(time * 0.0042 + e.ph + k);
    let fold = (46.0 * sin(x / 190.0 + tf * 0.05 + 1.3 + k * 2.1)
        + 22.0 * sin(x / 65.0 - tf * 0.11 + k)
        + 11.0 * sin(x / 27.0 + tf * 0.2)) * (1.0 + 0.6 * bu.x);
    let w = (62.0 + 18.0 * sin(x / 310.0 + k)) * e.wid;
    let q = (y - centre - fold) / w;
    // the rays: upright strands along the sheet, flickering (fast in a break-up)
    let r1 = vnoise(vec2<f32>(x / 7.0 + tf * 0.25 + k * 9.0 + 3.0 * bu.x * sin(time * 2.3 + x / 40.0), y / 45.0 + 3.1));
    let r2 = vnoise(vec2<f32>(x / 2.2 - tf * 0.4, 7.7 + k * 3.0));
    let rays = 0.3 + (1.0 + 0.8 * bu.x) * smoothstep(0.28, 0.85, r1) * (0.6 + 0.4 * r2);
    // the break-up's front runs out along the arc to both sides
    let df = abs(x - bu.z) - 28.0 * bu.y;
    let surge = 1.0 + bu.x * (0.5 + 3.0 * exp(-(df * df) / 45000.0));
    // a broad faint glow round the arc
    let glow = 0.03 * exp(-((y - centre) * (y - centre)) / (2.0 * 170.0 * 170.0));
    return exp(-q * q) * rays * surge + glow;
}

fn aurora_sky(d: vec3<f32>, time: f32, amp_s: f32, dist: f32, seed: f32, frag: vec2<f32>) -> vec3<f32> {
    let amp = min(abs(amp_s), 1.6);
    if (amp < 0.01 || d.z <= 0.0) {
        return vec3<f32>(0.0);
    }
    let e = au_event(seed);
    let hemi = select(1.0, -1.0, amp_s < 0.0);
    // the arc's frame: x along it (east), y towards its middle; turned from the pole by the
    // event's angle, which drifts a little
    let ang = e.ang + 0.05 * sin(time * 0.0031 + e.ph);
    let ca = cos(ang);
    let sa = sin(ang);
    let dx = ca * d.x - sa * d.y * hemi;
    let dy = sa * d.x + ca * d.y * hemi;
    let dz = max(d.z, 0.0);
    // where the ray meets the lowest and the highest level, towards the arc: when it never
    // gets near it there is nothing to sum
    let t_lo = au_shell(dz, 86.0);
    let t_hi = au_shell(dz, 330.0);
    // (900 km short of the arc the glow and the sheets are both far under what the eye or an 8-bit
    // picture can tell from nothing: no edge shows where the sum is left out; the field lines
    // lean up to 60 km towards it)
    if (max(dy * t_lo, dy * t_hi) + 60.0 < dist - 900.0) {
        return vec3<f32>(0.0);
    }
    // 100 %: green; above it the red and the violet join
    let hot = smoothstep(1.0, 1.45, amp);
    let bu = au_burst(e, time, amp);
    let c1 = e.w1 * 0.55;
    let c2 = e.w2 * 0.45;
    let steps = 26;
    let jitter = fract(52.9829189 * fract(dot(frag, vec2<f32>(0.06711056, 0.00583715))));
    let dh = 244.0 / f32(steps);
    var acc = vec3<f32>(0.0);
    for (var i = 0; i < steps; i = i + 1) {
        let h = 86.0 + (f32(i) + jitter) * dh;
        let t = au_shell(dz, h);
        let x = dx * t;
        // (the field lines lean away from the pole going up: the label of the line a point is
        // on is its distance plus a quarter of its height over 110 km)
        let y = dy * t + 0.25 * (h - 110.0);
        var s = au_sheet(x, y, dist, time, 0.0, 0.0, e, bu) + c1 * au_sheet(x, y, dist, time, 1.0, e.g1, e, bu);
        if (c2 > 0.0) {
            s = s + c2 * au_sheet(x, y, dist, time, 2.0, e.g2, e, bu);
        }
        acc = acc + au_height_colour(h, hot, e) * s;
    }
    // the path through the layer is longer towards the horizon (the sheets are seen edge on
    // and look brighter); the sum was taken in height steps
    let along = dh / max(d.z, 0.16);
    let pulse = (0.80 + 0.20 * sin(time * 0.37 + d.x * 5.0)) * (0.88 + 0.12 * sin(time * 0.023));
    // (the air dims the last degree over the horizon)
    let low = smoothstep(0.0, 0.05, d.z);
    let norm = 1.3 / (1.0 + c1 + c2);
    // (above 100 % the strength adds colour more than light: a flood of it would wash the sky)
    let level = select(1.0 + 0.4 * (amp - 1.0), amp, amp <= 1.0);
    // (0.0012: the first pass, 0.0021, was too bright for a night sky; the cap goes with it)
    return min(acc * along * 0.0012 * level * pulse * low * norm, vec3<f32>(0.35));
}
