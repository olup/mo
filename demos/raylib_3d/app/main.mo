import * as rl from "raylib/raylib"

fn draw_scene() {
    rl.clear_background(18, 20, 28, 255)
    rl.begin_mode3d(
        5.0, 4.0, 7.0,
        0.0, 0.75, 0.0,
        0.0, 1.0, 0.0,
        45.0,
        0,
    )
    rl.draw_grid(20, 1.0)
    rl.draw_cube(0.0, 1.0, 0.0, 2.0, 2.0, 2.0, 64, 160, 255, 255)
    rl.draw_cube_wires(0.0, 1.0, 0.0, 2.05, 2.05, 2.05, 245, 245, 245, 255)
    rl.draw_cube(-2.75, 0.5, -1.25, 1.0, 1.0, 1.0, 245, 120, 70, 255)
    rl.draw_cube(2.75, 0.75, 1.25, 1.0, 1.5, 1.0, 120, 220, 150, 255)
    rl.end_mode3d()
    rl.draw_text("Mo + raylib: native 3D rendering", 24, 24, 24, 245, 245, 245, 255)
    rl.draw_text("Static C library linked from package mo.toml", 24, 56, 18, 180, 190, 205, 255)
}

fn main() -> Int {
    rl.init_window(960, 540, "Mo raylib 3D demo")
    rl.set_target_fps(60)

    while !rl.window_should_close() {
        rl.begin_drawing()
        draw_scene()
        rl.end_drawing()
    }

    rl.close_window()
    return 42
}
