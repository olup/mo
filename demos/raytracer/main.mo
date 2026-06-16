import * as float from "std/float"
import * as fs from "std/fs"
import * as io from "std/io"
import * as String from "std/string"

struct Vec3 {
    x: Float64
    y: Float64
    z: Float64
}

struct Sphere {
    center: Vec3
    radius: Float64
    red: Float64
    green: Float64
    blue: Float64
}

struct Hit {
    found: Bool
    t: Float64
    red: Float64
    green: Float64
    blue: Float64
}

fn vec3(x: Float64, y: Float64, z: Float64) -> Vec3 {
    return Vec3 { x: x, y: y, z: z }
}

fn add(a: &Vec3, b: &Vec3) -> Vec3 {
    return vec3(a.x + b.x, a.y + b.y, a.z + b.z)
}

fn sub(a: &Vec3, b: &Vec3) -> Vec3 {
    return vec3(a.x - b.x, a.y - b.y, a.z - b.z)
}

fn scale(a: &Vec3, value: Float64) -> Vec3 {
    return vec3(a.x * value, a.y * value, a.z * value)
}

fn dot(a: &Vec3, b: &Vec3) -> Float64 {
    return a.x * b.x + a.y * b.y + a.z * b.z
}

fn length(a: &Vec3) -> Float64 {
    return sqrt(dot(a, a))
}

fn normalize(a: &Vec3) -> Vec3 {
    let len = length(a)
    return scale(a, 1.0 / len)
}

fn sqrt(value: Float64) -> Float64 {
    if value <= 0.0 {
        return 0.0
    }
    let mut guess = value
    let mut index = 0
    while index < 8 {
        guess = (guess + value / guess) / 2.0
        index += 1
    }
    return guess
}

fn hit_sphere(origin: &Vec3, direction: &Vec3, sphere: &Sphere) -> Hit {
    let oc = sub(origin, sphere.center)
    let a = dot(direction, direction)
    let half_b = dot(oc, direction)
    let c = dot(oc, oc) - sphere.radius * sphere.radius
    let discriminant = half_b * half_b - a * c
    if discriminant < 0.0 {
        return Hit { found: false, t: 0.0, red: 0.0, green: 0.0, blue: 0.0 }
    }

    let root = (0.0 - half_b - sqrt(discriminant)) / a
    if root <= 0.001 {
        return Hit { found: false, t: 0.0, red: 0.0, green: 0.0, blue: 0.0 }
    }

    let point = add(origin, scale(direction, root))
    let normal = normalize(sub(point, sphere.center))
    let light = normalize(vec3(0.7, 1.0, 0.5))
    let diffuse = float.clamp(dot(normal, light), 0.0, 1.0)
    let ambient = 0.18
    let shade = ambient + diffuse * 0.82

    return Hit {
        found: true,
        t: root,
        red: sphere.red * shade,
        green: sphere.green * shade,
        blue: sphere.blue * shade
    }
}

fn trace(origin: &Vec3, direction: &Vec3) -> Hit {
    let left = Sphere {
        center: vec3(-0.55, -0.05, -2.4),
        radius: 0.58,
        red: 0.95,
        green: 0.25,
        blue: 0.18
    }
    let right = Sphere {
        center: vec3(0.55, 0.0, -2.9),
        radius: 0.72,
        red: 0.15,
        green: 0.55,
        blue: 0.95
    }
    let floor = Sphere {
        center: vec3(0.0, -100.85, -2.8),
        radius: 100.0,
        red: 0.72,
        green: 0.72,
        blue: 0.66
    }

    let hit_left = hit_sphere(origin, direction, left)
    let hit_right = hit_sphere(origin, direction, right)
    let hit_floor = hit_sphere(origin, direction, floor)

    if hit_left.found && (!hit_right.found || hit_left.t < hit_right.t) && (!hit_floor.found || hit_left.t < hit_floor.t) {
        return hit_left
    }
    if hit_right.found && (!hit_floor.found || hit_right.t < hit_floor.t) {
        return hit_right
    }
    if hit_floor.found {
        return hit_floor
    }

    let sky = 0.5 * (direction.y + 1.0)
    return Hit {
        found: true,
        t: 99999.0,
        red: 0.25 * (1.0 - sky) + 0.65 * sky,
        green: 0.35 * (1.0 - sky) + 0.78 * sky,
        blue: 0.55 * (1.0 - sky) + 1.0 * sky
    }
}

fn channel(value: Float64) -> Int {
    return float.to_int(float.clamp(value, 0.0, 1.0) * 255.0)
}

fn write_int(fd: Int, value: Int) -> Int {
    let text = String.from_int(value)
    return io.write_fd(fd, text)
}

fn write_pixel(fd: Int, red: Int, green: Int, blue: Int) -> Int {
    let mut written = 0
    written += write_int(fd, red)
    written += io.write_fd(fd, " ")
    written += write_int(fd, green)
    written += io.write_fd(fd, " ")
    written += write_int(fd, blue)
    written += io.write_fd(fd, "\n")
    return written
}

fn render(path: &Str) -> Int {
    let width = 120
    let height = 72
    let origin = vec3(0.0, 0.0, 0.0)
    let fd = fs.open_write_truncate(path)
    if fd <= 0 {
        return fd
    }
    let mut written = 0

    written += io.write_fd(fd, "P3\n")
    written += write_int(fd, width)
    written += io.write_fd(fd, " ")
    written += write_int(fd, height)
    written += io.write_fd(fd, "\n255\n")

    let mut y = height - 1
    while y >= 0 {
        let mut x = 0
        while x < width {
            let u = ((x + x - width) * 1.0) / width
            let v = ((y + y - height) * 1.0) / height
            let direction = normalize(vec3(u, v, -1.35))
            let hit = trace(origin, direction)
            written += write_pixel(fd, channel(hit.red), channel(hit.green), channel(hit.blue))
            x += 1
        }
        y -= 1
    }

    io.close_fd(fd)
    return written
}

fn main() -> Int {
    let written = render("/tmp/mo_raytracer.ppm")
    if written > 1000 {
        return 42
    }
    return 1
}
