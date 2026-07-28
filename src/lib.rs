#![no_std]

use core::panic::PanicInfo;

const PALETTE: *mut [u32; 4] = 0x04 as _;
const GAMEPAD1: *const u8 = 0x16 as _;
const SYSTEM_FLAGS: *mut u8 = 0x1f as _;
const FRAMEBUFFER: *mut [u8; 6400] = 0xa0 as _;

const BUTTON_LEFT: u8 = 16;
const BUTTON_RIGHT: u8 = 32;
const BUTTON_UP: u8 = 64;
const BUTTON_DOWN: u8 = 128;
const BUTTON_1: u8 = 1;
const BUTTON_2: u8 = 2;
const PRESERVE_FRAMEBUFFER: u8 = 1;
const HIDE_GAMEPAD_OVERLAY: u8 = 2;

const W: i32 = 160;
const H: i32 = 160;
const EPS: f32 = 0.008;
const FAR: f32 = 28.0;
const TRIANGLE_COUNT: usize = 8;
const ACTIVE_TRIANGLE_COUNT: usize = 8;

const BAYER: [u8; 16] = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5];

#[derive(Clone, Copy, Default)]
struct Vec3 {
    x: f32,
    y: f32,
    z: f32,
}

impl Vec3 {
    const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
    fn dot(self, b: Self) -> f32 {
        self.x * b.x + self.y * b.y + self.z * b.z
    }
    fn cross(self, b: Self) -> Self {
        Self::new(
            self.y * b.z - self.z * b.y,
            self.z * b.x - self.x * b.z,
            self.x * b.y - self.y * b.x,
        )
    }
    fn length(self) -> f32 {
        fsqrt(self.dot(self))
    }
    fn normalized(self) -> Self {
        let l = self.length();
        if l > 0.0001 {
            self * (1.0 / l)
        } else {
            self
        }
    }
    fn clamp01(self) -> Self {
        Self::new(
            clamp(self.x, 0.0, 1.0),
            clamp(self.y, 0.0, 1.0),
            clamp(self.z, 0.0, 1.0),
        )
    }
    fn luminance(self) -> f32 {
        self.x * 0.30 + self.y * 0.48 + self.z * 0.22
    }
}

impl core::ops::Add for Vec3 {
    type Output = Self;
    fn add(self, b: Self) -> Self {
        Self::new(self.x + b.x, self.y + b.y, self.z + b.z)
    }
}
impl core::ops::Sub for Vec3 {
    type Output = Self;
    fn sub(self, b: Self) -> Self {
        Self::new(self.x - b.x, self.y - b.y, self.z - b.z)
    }
}
impl core::ops::Mul<f32> for Vec3 {
    type Output = Self;
    fn mul(self, s: f32) -> Self {
        Self::new(self.x * s, self.y * s, self.z * s)
    }
}
impl core::ops::Mul<Vec3> for Vec3 {
    type Output = Self;
    fn mul(self, b: Vec3) -> Self {
        Self::new(self.x * b.x, self.y * b.y, self.z * b.z)
    }
}

#[derive(Clone, Copy)]
struct Material {
    base: Vec3,
    emission: Vec3,
    emission_strength: f32,
    reflectivity: f32,
    roughness: f32,
}

const MATERIALS: [Material; 7] = [
    Material {
        base: Vec3::new(0.30, 0.30, 0.30),
        emission: Vec3::new(0.0, 0.0, 0.0),
        emission_strength: 0.0,
        reflectivity: 0.0,
        roughness: 0.0,
    },
    Material {
        base: Vec3::new(0.48, 0.48, 0.48),
        emission: Vec3::new(0.0, 0.0, 0.0),
        emission_strength: 0.0,
        reflectivity: 0.72,
        roughness: 0.08,
    },
    Material {
        base: Vec3::new(0.58, 0.58, 0.58),
        emission: Vec3::new(0.0, 0.0, 0.0),
        emission_strength: 0.0,
        reflectivity: 0.0,
        roughness: 0.0,
    },
    Material {
        base: Vec3::new(0.42, 0.42, 0.42),
        emission: Vec3::new(0.0, 0.0, 0.0),
        emission_strength: 0.0,
        reflectivity: 0.52,
        roughness: 0.48,
    },
    Material {
        base: Vec3::new(0.82, 0.82, 0.82),
        emission: Vec3::new(1.0, 1.0, 1.0),
        emission_strength: 1.5,
        reflectivity: 0.0,
        roughness: 0.0,
    },
    Material {
        base: Vec3::new(0.72, 0.72, 0.72),
        emission: Vec3::new(1.0, 1.0, 1.0),
        emission_strength: 0.28,
        reflectivity: 0.0,
        roughness: 0.0,
    },
    Material {
        base: Vec3::new(0.85, 0.85, 0.85),
        emission: Vec3::new(0.0, 0.0, 0.0),
        emission_strength: 0.0,
        reflectivity: 1.0,
        roughness: 0.0,
    },
];

#[derive(Clone, Copy)]
struct Triangle {
    a: Vec3,
    e1: Vec3,
    e2: Vec3,
    normal: Vec3,
    material: u8,
    object: u8,
}

const EMPTY_TRIANGLE: Triangle = Triangle {
    a: Vec3::new(0.0, 0.0, 0.0),
    e1: Vec3::new(0.0, 0.0, 0.0),
    e2: Vec3::new(0.0, 0.0, 0.0),
    normal: Vec3::new(0.0, 1.0, 0.0),
    material: 0,
    object: 0,
};

#[derive(Clone, Copy)]
struct Light {
    position: Vec3,
    color: Vec3,
    intensity: f32,
}

const LIGHTS: [Light; 2] = [
    Light {
        position: Vec3::new(-2.0, 2.7, 7.2),
        color: Vec3::new(1.0, 1.0, 1.0),
        intensity: 7.8,
    },
    Light {
        position: Vec3::new(2.0, 2.5, 8.5),
        color: Vec3::new(1.0, 1.0, 1.0),
        intensity: 4.4,
    },
];

#[derive(Clone, Copy)]
struct Sphere {
    center: Vec3,
    radius: f32,
    material: u8,
    object: u8,
}

const SPHERES: [Sphere; 3] = [
    // Perfect mirror: exactly 100% reflectivity, zero roughness (material 6).
    Sphere {
        center: Vec3::new(0.0, 1.05, 4.35),
        radius: 1.05,
        material: 6,
        object: 20,
    },
    Sphere {
        center: Vec3::new(-2.0, 2.7, 7.2),
        radius: 0.72,
        material: 4,
        object: 21,
    },
    Sphere {
        center: Vec3::new(2.0, 2.5, 8.5),
        radius: 0.68,
        material: 5,
        object: 22,
    },
];

#[derive(Clone, Copy)]
struct Box3 {
    min: Vec3,
    max: Vec3,
    material: u8,
    object: u8,
}

const BOXES: [Box3; 4] = [
    Box3 {
        min: Vec3::new(-3.25, 0.0, 5.25),
        max: Vec3::new(-1.75, 1.85, 6.25),
        material: 1,
        object: 10,
    },
    Box3 {
        min: Vec3::new(-0.75, 0.0, 6.65),
        max: Vec3::new(0.35, 1.45, 7.55),
        material: 2,
        object: 11,
    },
    Box3 {
        min: Vec3::new(1.65, 0.0, 5.65),
        max: Vec3::new(3.05, 1.35, 6.75),
        material: 3,
        object: 12,
    },
    Box3 {
        min: Vec3::new(-0.55, 2.35, 8.15),
        max: Vec3::new(0.55, 3.35, 9.25),
        material: 2,
        object: 13,
    },
];

#[derive(Clone, Copy)]
struct Cylinder {
    center: Vec3,
    radius: f32,
    y_min: f32,
    y_max: f32,
    material: u8,
    object: u8,
}

const CYLINDERS: [Cylinder; 1] = [Cylinder {
    center: Vec3::new(3.65, 0.0, 8.0),
    radius: 0.68,
    y_min: 0.0,
    y_max: 2.55,
    material: 1,
    object: 40,
}];

#[derive(Clone, Copy)]
struct Ray {
    origin: Vec3,
    direction: Vec3,
}

#[derive(Clone, Copy)]
struct Hit {
    distance: f32,
    position: Vec3,
    normal: Vec3,
    material: u8,
    object: u8,
}

static mut TRIANGLES: [Triangle; TRIANGLE_COUNT] = [EMPTY_TRIANGLE; TRIANGLE_COUNT];
static mut CAMERA_X: f32 = 0.0;
static mut CAMERA_Z: f32 = -2.8;
static mut CAMERA_FORWARD_X: f32 = 0.0;
static mut CAMERA_FORWARD_Z: f32 = 1.0;
static mut CAMERA_PITCH: f32 = 0.0;
static mut FRAME: u32 = 0;
static mut STARTED: bool = false;

fn clamp(v: f32, lo: f32, hi: f32) -> f32 {
    if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    }
}

fn fabs(v: f32) -> f32 {
    if v < 0.0 {
        -v
    } else {
        v
    }
}

// Two Newton steps after the classic bit-level estimate are accurate enough
// for ray directions and distances, and keep this no_std cart dependency-free.
fn fsqrt(v: f32) -> f32 {
    if v <= 0.0 {
        return 0.0;
    }
    let half = v * 0.5;
    let mut inv = f32::from_bits(0x5f37_59df - (v.to_bits() >> 1));
    inv *= 1.5 - half * inv * inv;
    inv *= 1.5 - half * inv * inv;
    v * inv
}

fn triangle(a: Vec3, b: Vec3, c: Vec3, material: u8, object: u8) -> Triangle {
    let e1 = b - a;
    let e2 = c - a;
    Triangle {
        a,
        e1,
        e2,
        normal: e1.cross(e2).normalized(),
        material,
        object,
    }
}

unsafe fn init_scene() {
    // Eight faces form one floating octahedron.
    let top = Vec3::new(-3.55, 3.05, 8.15);
    let bottom = Vec3::new(-3.55, 0.55, 8.15);
    let east = Vec3::new(-2.45, 1.8, 8.15);
    let west = Vec3::new(-4.65, 1.8, 8.15);
    let front = Vec3::new(-3.55, 1.8, 7.05);
    let back = Vec3::new(-3.55, 1.8, 9.25);
    TRIANGLES[0] = triangle(top, front, east, 3, 30);
    TRIANGLES[1] = triangle(top, east, back, 3, 30);
    TRIANGLES[2] = triangle(top, back, west, 3, 30);
    TRIANGLES[3] = triangle(top, west, front, 3, 30);
    TRIANGLES[4] = triangle(bottom, east, front, 3, 30);
    TRIANGLES[5] = triangle(bottom, back, east, 3, 30);
    TRIANGLES[6] = triangle(bottom, west, back, 3, 30);
    TRIANGLES[7] = triangle(bottom, front, west, 3, 30);
}

#[no_mangle]
pub fn start() {
    unsafe {
        // WASM-4's classic default palette, in its native light-to-dark order.
        *PALETTE = [0xE0F8CF, 0x86C06C, 0x306850, 0x071821];
        *SYSTEM_FLAGS = PRESERVE_FRAMEBUFFER | HIDE_GAMEPAD_OVERLAY;
        init_scene();
        *FRAMEBUFFER = [0xFF; 6400];
        STARTED = true;
    }
}

fn intersect_triangle(ray: Ray, tri: Triangle, max_distance: f32) -> Option<(f32, Vec3)> {
    let p = ray.direction.cross(tri.e2);
    let det = tri.e1.dot(p);
    if det > -0.0001 && det < 0.0001 {
        return None;
    }
    let inv = 1.0 / det;
    let tvec = ray.origin - tri.a;
    let u = tvec.dot(p) * inv;
    if u < 0.0 || u > 1.0 {
        return None;
    }
    let q = tvec.cross(tri.e1);
    let v = ray.direction.dot(q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = tri.e2.dot(q) * inv;
    if t > EPS && t < max_distance {
        let n = if tri.normal.dot(ray.direction) > 0.0 {
            tri.normal * -1.0
        } else {
            tri.normal
        };
        Some((t, n))
    } else {
        None
    }
}

fn intersect_sphere(ray: Ray, sphere: Sphere, max_distance: f32) -> Option<(f32, Vec3)> {
    let oc = ray.origin - sphere.center;
    let b = oc.dot(ray.direction);
    let c = oc.dot(oc) - sphere.radius * sphere.radius;
    let discriminant = b * b - c;
    if discriminant < 0.0 {
        return None;
    }
    let root = fsqrt(discriminant);
    let mut t = -b - root;
    if t <= EPS {
        t = -b + root;
    }
    if t > EPS && t < max_distance {
        let position = ray.origin + ray.direction * t;
        Some((t, (position - sphere.center) * (1.0 / sphere.radius)))
    } else {
        None
    }
}

fn update_slab(
    origin: f32,
    direction: f32,
    min: f32,
    max: f32,
    negative_normal: Vec3,
    positive_normal: Vec3,
    near: &mut f32,
    far: &mut f32,
    normal: &mut Vec3,
) -> bool {
    if direction > -0.0001 && direction < 0.0001 {
        return origin >= min && origin <= max;
    }
    let inverse = 1.0 / direction;
    let mut t1 = (min - origin) * inverse;
    let mut t2 = (max - origin) * inverse;
    let mut entering_normal = negative_normal;
    if t1 > t2 {
        core::mem::swap(&mut t1, &mut t2);
        entering_normal = positive_normal;
    }
    if t1 > *near {
        *near = t1;
        *normal = entering_normal;
    }
    if t2 < *far {
        *far = t2;
    }
    *near <= *far
}

fn intersect_box(ray: Ray, body: Box3, max_distance: f32) -> Option<(f32, Vec3)> {
    let mut near = EPS;
    let mut far = max_distance;
    let mut normal = Vec3::new(0.0, 1.0, 0.0);
    if !update_slab(
        ray.origin.x,
        ray.direction.x,
        body.min.x,
        body.max.x,
        Vec3::new(-1.0, 0.0, 0.0),
        Vec3::new(1.0, 0.0, 0.0),
        &mut near,
        &mut far,
        &mut normal,
    ) {
        return None;
    }
    if !update_slab(
        ray.origin.y,
        ray.direction.y,
        body.min.y,
        body.max.y,
        Vec3::new(0.0, -1.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
        &mut near,
        &mut far,
        &mut normal,
    ) {
        return None;
    }
    if !update_slab(
        ray.origin.z,
        ray.direction.z,
        body.min.z,
        body.max.z,
        Vec3::new(0.0, 0.0, -1.0),
        Vec3::new(0.0, 0.0, 1.0),
        &mut near,
        &mut far,
        &mut normal,
    ) {
        return None;
    }
    if near > EPS && near < max_distance {
        Some((near, normal))
    } else {
        None
    }
}

fn intersect_cylinder(ray: Ray, body: Cylinder, max_distance: f32) -> Option<(f32, Vec3)> {
    let ox = ray.origin.x - body.center.x;
    let oz = ray.origin.z - body.center.z;
    let a = ray.direction.x * ray.direction.x + ray.direction.z * ray.direction.z;
    let mut best = max_distance;
    let mut normal = Vec3::new(0.0, 1.0, 0.0);

    if a > 0.0001 {
        let b = ox * ray.direction.x + oz * ray.direction.z;
        let c = ox * ox + oz * oz - body.radius * body.radius;
        let discriminant = b * b - a * c;
        if discriminant >= 0.0 {
            let root = fsqrt(discriminant);
            let mut t = (-b - root) / a;
            let mut y = ray.origin.y + ray.direction.y * t;
            if t <= EPS || y < body.y_min || y > body.y_max {
                t = (-b + root) / a;
                y = ray.origin.y + ray.direction.y * t;
            }
            if t > EPS && t < best && y >= body.y_min && y <= body.y_max {
                best = t;
                let position = ray.origin + ray.direction * t;
                normal = Vec3::new(
                    (position.x - body.center.x) / body.radius,
                    0.0,
                    (position.z - body.center.z) / body.radius,
                );
            }
        }
    }

    if ray.direction.y > 0.0001 || ray.direction.y < -0.0001 {
        let cap_y = if ray.direction.y > 0.0 {
            body.y_min
        } else {
            body.y_max
        };
        let t = (cap_y - ray.origin.y) / ray.direction.y;
        if t > EPS && t < best {
            let position = ray.origin + ray.direction * t;
            let dx = position.x - body.center.x;
            let dz = position.z - body.center.z;
            if dx * dx + dz * dz <= body.radius * body.radius {
                best = t;
                normal = if ray.direction.y > 0.0 {
                    Vec3::new(0.0, -1.0, 0.0)
                } else {
                    Vec3::new(0.0, 1.0, 0.0)
                };
            }
        }
    }

    if best < max_distance {
        Some((best, normal))
    } else {
        None
    }
}

fn trace_closest(ray: Ray, max_distance: f32) -> Option<Hit> {
    let mut best = max_distance;
    let mut hit: Option<Hit> = None;

    if ray.direction.y < -0.0001 {
        let t = -ray.origin.y / ray.direction.y;
        if t > EPS && t < best {
            best = t;
            hit = Some(Hit {
                distance: t,
                position: ray.origin + ray.direction * t,
                normal: Vec3::new(0.0, 1.0, 0.0),
                material: 0,
                object: 0,
            });
        }
    }

    unsafe {
        let mut i = 0;
        while i < ACTIVE_TRIANGLE_COUNT {
            let tri = TRIANGLES[i];
            if let Some((t, normal)) = intersect_triangle(ray, tri, best) {
                best = t;
                hit = Some(Hit {
                    distance: t,
                    position: ray.origin + ray.direction * t,
                    normal,
                    material: tri.material,
                    object: tri.object,
                });
            }
            i += 1;
        }
    }

    let mut i = 0;
    while i < BOXES.len() {
        let body = BOXES[i];
        if let Some((t, normal)) = intersect_box(ray, body, best) {
            best = t;
            hit = Some(Hit {
                distance: t,
                position: ray.origin + ray.direction * t,
                normal,
                material: body.material,
                object: body.object,
            });
        }
        i += 1;
    }
    i = 0;
    while i < SPHERES.len() {
        let sphere = SPHERES[i];
        if let Some((t, normal)) = intersect_sphere(ray, sphere, best) {
            best = t;
            hit = Some(Hit {
                distance: t,
                position: ray.origin + ray.direction * t,
                normal,
                material: sphere.material,
                object: sphere.object,
            });
        }
        i += 1;
    }
    i = 0;
    while i < CYLINDERS.len() {
        let body = CYLINDERS[i];
        if let Some((t, normal)) = intersect_cylinder(ray, body, best) {
            best = t;
            hit = Some(Hit {
                distance: t,
                position: ray.origin + ray.direction * t,
                normal,
                material: body.material,
                object: body.object,
            });
        }
        i += 1;
    }
    hit
}

fn trace_any(ray: Ray, max_distance: f32, ignore_object: u8) -> bool {
    unsafe {
        let mut i = 0;
        while i < ACTIVE_TRIANGLE_COUNT {
            let tri = TRIANGLES[i];
            if tri.object != ignore_object && intersect_triangle(ray, tri, max_distance).is_some() {
                return true;
            }
            i += 1;
        }
    }
    let mut i = 0;
    while i < BOXES.len() {
        let body = BOXES[i];
        if body.object != ignore_object && intersect_box(ray, body, max_distance).is_some() {
            return true;
        }
        i += 1;
    }
    i = 0;
    while i < SPHERES.len() {
        let sphere = SPHERES[i];
        if sphere.object != ignore_object
            && MATERIALS[sphere.material as usize].emission_strength == 0.0
            && intersect_sphere(ray, sphere, max_distance).is_some()
        {
            return true;
        }
        i += 1;
    }
    i = 0;
    while i < CYLINDERS.len() {
        let body = CYLINDERS[i];
        if body.object != ignore_object && intersect_cylinder(ray, body, max_distance).is_some() {
            return true;
        }
        i += 1;
    }
    false
}

fn sky(direction: Vec3) -> Vec3 {
    let h = clamp(direction.y * 0.65 + 0.35, 0.0, 1.0);
    Vec3::new(0.025, 0.025, 0.025) + Vec3::new(0.065, 0.065, 0.065) * h
}

fn simple_reflection_color(ray: Ray) -> Vec3 {
    if let Some(hit) = trace_closest(ray, FAR) {
        simple_hit_color(hit)
    } else {
        sky(ray.direction)
    }
}

fn simple_hit_color(hit: Hit) -> Vec3 {
    let mat = MATERIALS[hit.material as usize];
    let mut color = if mat.emission_strength > 0.0 {
        mat.emission * mat.emission_strength
    } else if hit.object == 0 {
        mat.base * 0.78
    } else {
        mat.base * 0.46
    };
    let fog = clamp((hit.distance - 5.0) / 17.0, 0.0, 1.0);
    color = color * (1.0 - fog) + Vec3::new(0.055, 0.055, 0.055) * fog;
    color
}

fn reflected_color(ray: Ray, allow_second_bounce: bool, px: i32, py: i32) -> Vec3 {
    if let Some(hit) = trace_closest(ray, FAR) {
        let mat = MATERIALS[hit.material as usize];
        let mut color = simple_hit_color(hit);

        // The second bounce is deliberately simple: no lights, shadows, or third bounce.
        if allow_second_bounce && mat.reflectivity >= 0.5 && mat.emission_strength == 0.0 {
            let reflected = ray.direction - hit.normal * (2.0 * ray.direction.dot(hit.normal));
            let pattern = BAYER[(((px + 1) & 3) + (((py + 2) & 3) << 2)) as usize] as f32;
            let noise = pattern / 15.0 - 0.5;
            let rough = Vec3::new(-noise * 0.6, noise, noise * 0.35) * (mat.roughness * 0.18);
            let second_ray = Ray {
                origin: hit.position + hit.normal * EPS,
                direction: (reflected + rough).normalized(),
            };
            let second = simple_reflection_color(second_ray);
            color = color * (1.0 - mat.reflectivity) + second * mat.reflectivity;
        }
        color
    } else {
        sky(ray.direction)
    }
}

fn contact_shadow(position: Vec3) -> f32 {
    let centers = [
        (Vec3::new(-2.0, 0.0, 7.2), 1.05, 0.42),
        (Vec3::new(2.0, 0.0, 8.5), 1.0, 0.38),
        (Vec3::new(0.0, 0.0, 8.7), 0.9, 0.34),
        (Vec3::new(0.0, 0.0, 4.35), 0.9, 0.30),
        (Vec3::new(-3.55, 0.0, 8.15), 1.0, 0.32),
    ];
    let mut shade = 1.0;
    let mut i = 0;
    while i < centers.len() {
        let dx = position.x - centers[i].0.x;
        let dz = position.z - centers[i].0.z;
        let d = fsqrt(dx * dx + dz * dz) / centers[i].1;
        if d < 1.0 {
            shade *= 1.0 - centers[i].2 * (1.0 - d);
        }
        i += 1;
    }
    shade
}

fn shade_primary(ray: Ray, hit: Hit, px: i32, py: i32, allow_second_bounce: bool) -> Vec3 {
    let mat = MATERIALS[hit.material as usize];
    if mat.emission_strength > 0.0 {
        return mat.emission * mat.emission_strength + Vec3::new(0.18, 0.18, 0.18);
    }

    let mut base = mat.base;
    if hit.object == 0 {
        // World-locked, restrained floor grid variation.
        let gx = (hit.position.x * 1.35) as i32;
        let gz = (hit.position.z * 1.35) as i32;
        let grid = if ((gx ^ gz) & 1) == 0 { 1.08 } else { 0.91 };
        base = base * grid;
    } else if hit.object >= 10 && hit.object <= 13 {
        // Stable per-face contrast makes the analytic boxes read as volumes.
        let face_tone = if hit.normal.y != 0.0 {
            1.18
        } else if hit.normal.x != 0.0 {
            0.72
        } else {
            0.94
        };
        base = base * face_tone;
    }

    let mut color = base * 0.22;
    let mut i = 0;
    while i < LIGHTS.len() {
        let light = LIGHTS[i];
        let to_light = light.position - hit.position;
        let dist2 = to_light.dot(to_light);
        if dist2 < 95.0 {
            let dist = fsqrt(dist2);
            let ldir = to_light * (1.0 / dist);
            let ndotl = clamp(hit.normal.dot(ldir), 0.0, 1.0);
            let attenuation = light.intensity / (1.0 + dist2 * 0.34);
            let shadow_ray = Ray {
                origin: hit.position + hit.normal * EPS,
                direction: ldir,
            };
            let blocked = trace_any(shadow_ray, dist - EPS * 2.0, hit.object);
            if !blocked {
                color = color + (base * light.color) * (ndotl * attenuation);
                color = color + light.color * (0.045 * attenuation);
            }
        }
        i += 1;
    }

    if hit.object == 0 {
        color = color * contact_shadow(hit.position);
    }

    let view = ray.direction * -1.0;
    let mut rim = 1.0 - clamp(view.dot(hit.normal), 0.0, 1.0);
    rim = rim * rim * 0.28;
    color = color + Vec3::new(0.62, 0.62, 0.62) * rim;

    if mat.reflectivity > 0.01 {
        let reflected = ray.direction - hit.normal * (2.0 * ray.direction.dot(hit.normal));
        let n = (BAYER[((px & 3) + ((py & 3) << 2)) as usize] as f32 / 15.0) - 0.5;
        let rough = Vec3::new(n, -n * 0.55, n * 0.37) * (mat.roughness * 0.22);
        let rr = Ray {
            origin: hit.position + hit.normal * EPS,
            direction: (reflected + rough).normalized(),
        };
        let rc = reflected_color(rr, allow_second_bounce, px, py);
        color = color * (1.0 - mat.reflectivity) + rc * mat.reflectivity;
    }

    let fog = clamp((hit.distance - 6.0) / 15.0, 0.0, 1.0);
    color * (1.0 - fog) + Vec3::new(0.05, 0.05, 0.05) * fog
}

fn add_screen_glow(
    mut color: Vec3,
    px: i32,
    py: i32,
    camera: Vec3,
    forward: Vec3,
    right: Vec3,
    up: Vec3,
) -> Vec3 {
    let mut i = 0;
    while i < LIGHTS.len() {
        let l = LIGHTS[i];
        let delta = l.position - camera;
        let depth = delta.dot(forward);
        if depth > 0.15 {
            let sx = 80.0 + delta.dot(right) / depth * 106.67;
            let sy = 80.0 - delta.dot(up) / depth * 106.67;
            let dx = px as f32 + 0.5 - sx;
            let dy = py as f32 + 0.5 - sy;
            let radius = clamp(300.0 / depth, 16.0, 40.0);
            let d2 = dx * dx + dy * dy;
            if d2 < radius * radius {
                let glow_strength = if i == 0 { 0.34 } else { 0.17 };
                let glow = (1.0 - fsqrt(d2) / radius) * glow_strength;
                color = color + l.color * glow;
            }
        }
        i += 1;
    }
    color
}

fn quantize(color: Vec3, x: i32, y: i32) -> u8 {
    let lum = color.clamp01().luminance();
    let scaled = clamp(lum * 1.35, 0.0, 1.0) * 3.0;
    let level = scaled as u8;
    let fraction = scaled - level as f32;
    let threshold = (BAYER[((x & 3) + ((y & 3) << 2)) as usize] as f32 + 0.5) / 16.0;
    let selected = if level < 3 && fraction > threshold {
        level + 1
    } else {
        level
    };
    3 - selected
}

fn put_pixel_2bpp(x: i32, y: i32, color: u8) {
    unsafe {
        let fb = &mut *FRAMEBUFFER;
        let index = (y * 160 + x) as usize;
        let byte = index >> 2;
        let shift = ((x & 3) << 1) as u8;
        fb[byte] = (fb[byte] & !(0x3 << shift)) | ((color & 0x3) << shift);
    }
}

fn collides(x: f32, z: f32) -> bool {
    const CAMERA_Y: f32 = 1.48;
    const CAMERA_RADIUS: f32 = 0.27;

    let mut i = 0;
    while i < BOXES.len() {
        let body = BOXES[i];
        let vertical_overlap =
            CAMERA_Y + CAMERA_RADIUS > body.min.y && CAMERA_Y - CAMERA_RADIUS < body.max.y;
        let nearest_x = clamp(x, body.min.x, body.max.x);
        let nearest_z = clamp(z, body.min.z, body.max.z);
        let dx = x - nearest_x;
        let dz = z - nearest_z;
        if vertical_overlap && dx * dx + dz * dz < CAMERA_RADIUS * CAMERA_RADIUS {
            return true;
        }
        i += 1;
    }

    i = 0;
    while i < SPHERES.len() {
        let sphere = SPHERES[i];
        let dx = x - sphere.center.x;
        let dy = CAMERA_Y - sphere.center.y;
        let dz = z - sphere.center.z;
        let radius = sphere.radius + CAMERA_RADIUS;
        if dx * dx + dy * dy + dz * dz < radius * radius {
            return true;
        }
        i += 1;
    }

    i = 0;
    while i < CYLINDERS.len() {
        let body = CYLINDERS[i];
        let vertical_overlap =
            CAMERA_Y + CAMERA_RADIUS > body.y_min && CAMERA_Y - CAMERA_RADIUS < body.y_max;
        let dx = x - body.center.x;
        let dz = z - body.center.z;
        let radius = body.radius + CAMERA_RADIUS;
        if vertical_overlap && dx * dx + dz * dz < radius * radius {
            return true;
        }
        i += 1;
    }

    // Exact octahedron shape in normalized L1 distance, with a small camera margin.
    let octa_distance =
        fabs(x + 3.55) / 1.10 + fabs(CAMERA_Y - 1.80) / 1.25 + fabs(z - 8.15) / 1.10;
    if octa_distance < 1.16 {
        return true;
    }

    false
}

#[no_mangle]
pub fn update() {
    unsafe {
        if !STARTED {
            start();
        }
        let gamepad = *GAMEPAD1;
        let moving = gamepad
            & (BUTTON_1 | BUTTON_2 | BUTTON_LEFT | BUTTON_RIGHT | BUTTON_UP | BUTTON_DOWN)
            != 0;

        // Rotate the horizontal forward vector incrementally, avoiding trig in no_std.
        let mut fx = CAMERA_FORWARD_X;
        let mut fz = CAMERA_FORWARD_Z;
        let turn_sin = 0.030;
        let turn_cos = 0.99955;
        if gamepad & BUTTON_LEFT != 0 {
            let old_fx = fx;
            fx = old_fx * turn_cos - fz * turn_sin;
            fz = old_fx * turn_sin + fz * turn_cos;
        }
        if gamepad & BUTTON_RIGHT != 0 {
            let old_fx = fx;
            fx = old_fx * turn_cos + fz * turn_sin;
            fz = -old_fx * turn_sin + fz * turn_cos;
        }
        let inv = 1.0 / fsqrt(fx * fx + fz * fz);
        fx *= inv;
        fz *= inv;
        CAMERA_FORWARD_X = fx;
        CAMERA_FORWARD_Z = fz;

        // WASM-4 maps Z/X to button 2/button 1 respectively.
        if gamepad & BUTTON_2 != 0 {
            CAMERA_PITCH += 0.018;
        }
        if gamepad & BUTTON_1 != 0 {
            CAMERA_PITCH -= 0.018;
        }
        CAMERA_PITCH = clamp(CAMERA_PITCH, -0.45, 0.45);

        let mut nx = CAMERA_X;
        let mut nz = CAMERA_Z;
        let speed = 0.105;
        if gamepad & BUTTON_UP != 0 {
            nx += fx * speed;
            nz += fz * speed;
        }
        if gamepad & BUTTON_DOWN != 0 {
            nx -= fx * speed;
            nz -= fz * speed;
        }
        nx = clamp(nx, -10.0, 10.0);
        nz = clamp(nz, -7.0, 18.0);

        // Resolve axes separately so glancing contact preserves tangential motion.
        if !collides(nx, CAMERA_Z) {
            CAMERA_X = nx;
        }
        if !collides(CAMERA_X, nz) {
            CAMERA_Z = nz;
        }

        let camera = Vec3::new(CAMERA_X, 1.48, CAMERA_Z);
        let forward = Vec3::new(fx, CAMERA_PITCH, fz).normalized();
        let right = Vec3::new(fz, 0.0, -fx);
        let camera_up = forward.cross(right).normalized();
        let parity = (FRAME & 1) as i32;
        let quarter = (FRAME & 3) as i32;
        let mut y = 0;
        while y < H {
            let mut x = 0;
            while x < W {
                let render_pixel = if moving {
                    ((x & 1) | ((y & 1) << 1)) == quarter
                } else {
                    ((x + y) & 1) == parity
                };
                if render_pixel {
                    let sx = ((x as f32 + 0.5) / W as f32 - 0.5) * 1.5;
                    let sy = (0.5 - (y as f32 + 0.5) / H as f32) * 1.5;
                    let direction = (forward + right * sx + camera_up * sy).normalized();
                    let ray = Ray {
                        origin: camera,
                        direction,
                    };
                    let color = if let Some(hit) = trace_closest(ray, FAR) {
                        shade_primary(ray, hit, x, y, !moving)
                    } else {
                        sky(direction)
                    };
                    let color = add_screen_glow(color, x, y, camera, forward, right, camera_up);
                    put_pixel_2bpp(x, y, quantize(color, x, y));
                }
                x += 1;
            }
            y += 1;
        }
        FRAME = FRAME.wrapping_add(1);
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
