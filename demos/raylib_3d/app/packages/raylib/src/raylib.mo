extern "C" {
    fn mo_rl_init_window(width: Int32, height: Int32, title: &Str)
    fn mo_rl_close_window()
    fn mo_rl_window_should_close() -> Int32
    fn mo_rl_set_target_fps(fps: Int32)

    fn mo_rl_begin_drawing()
    fn mo_rl_end_drawing()
    fn mo_rl_clear_background(r: Int32, g: Int32, b: Int32, a: Int32)

    fn mo_rl_begin_mode3d(
        pos_x: Float64, pos_y: Float64, pos_z: Float64,
        target_x: Float64, target_y: Float64, target_z: Float64,
        up_x: Float64, up_y: Float64, up_z: Float64,
        fovy: Float64,
        projection: Int32,
    )
    fn mo_rl_end_mode3d()
    fn mo_rl_draw_grid(slices: Int32, spacing: Float64)
    fn mo_rl_draw_cube(
        x: Float64, y: Float64, z: Float64,
        width: Float64, height: Float64, length: Float64,
        r: Int32, g: Int32, b: Int32, a: Int32,
    )
    fn mo_rl_draw_cube_wires(
        x: Float64, y: Float64, z: Float64,
        width: Float64, height: Float64, length: Float64,
        r: Int32, g: Int32, b: Int32, a: Int32,
    )
    fn mo_rl_draw_text(text: &Str, x: Int32, y: Int32, size: Int32, r: Int32, g: Int32, b: Int32, a: Int32)
    fn mo_rl_get_time() -> Float64
}

pub fn init_window(width: Int32, height: Int32, title: &Str) {
    mo_rl_init_window(width, height, title)
}

pub fn close_window() {
    mo_rl_close_window()
}

pub fn window_should_close() -> Bool {
    return mo_rl_window_should_close() != 0
}

pub fn set_target_fps(fps: Int32) {
    mo_rl_set_target_fps(fps)
}

pub fn begin_drawing() {
    mo_rl_begin_drawing()
}

pub fn end_drawing() {
    mo_rl_end_drawing()
}

pub fn clear_background(r: Int32, g: Int32, b: Int32, a: Int32) {
    mo_rl_clear_background(r, g, b, a)
}

pub fn begin_mode3d(
    pos_x: Float64, pos_y: Float64, pos_z: Float64,
    target_x: Float64, target_y: Float64, target_z: Float64,
    up_x: Float64, up_y: Float64, up_z: Float64,
    fovy: Float64,
    projection: Int32,
) {
    mo_rl_begin_mode3d(
        pos_x, pos_y, pos_z,
        target_x, target_y, target_z,
        up_x, up_y, up_z,
        fovy,
        projection,
    )
}

pub fn end_mode3d() {
    mo_rl_end_mode3d()
}

pub fn draw_grid(slices: Int32, spacing: Float64) {
    mo_rl_draw_grid(slices, spacing)
}

pub fn draw_cube(
    x: Float64, y: Float64, z: Float64,
    width: Float64, height: Float64, length: Float64,
    r: Int32, g: Int32, b: Int32, a: Int32,
) {
    mo_rl_draw_cube(x, y, z, width, height, length, r, g, b, a)
}

pub fn draw_cube_wires(
    x: Float64, y: Float64, z: Float64,
    width: Float64, height: Float64, length: Float64,
    r: Int32, g: Int32, b: Int32, a: Int32,
) {
    mo_rl_draw_cube_wires(x, y, z, width, height, length, r, g, b, a)
}

pub fn draw_text(text: &Str, x: Int32, y: Int32, size: Int32, r: Int32, g: Int32, b: Int32, a: Int32) {
    mo_rl_draw_text(text, x, y, size, r, g, b, a)
}

pub fn get_time() -> Float64 {
    return mo_rl_get_time()
}
