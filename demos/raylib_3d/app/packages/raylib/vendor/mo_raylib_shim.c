#include "raylib.h"

void mo_rl_init_window(int width, int height, const char *title) {
    InitWindow(width, height, title);
}

void mo_rl_close_window(void) {
    CloseWindow();
}

int mo_rl_window_should_close(void) {
    return WindowShouldClose();
}

void mo_rl_set_target_fps(int fps) {
    SetTargetFPS(fps);
}

void mo_rl_begin_drawing(void) {
    BeginDrawing();
}

void mo_rl_end_drawing(void) {
    EndDrawing();
}

void mo_rl_clear_background(int r, int g, int b, int a) {
    ClearBackground((Color){ (unsigned char)r, (unsigned char)g, (unsigned char)b, (unsigned char)a });
}

void mo_rl_begin_mode3d(
    double pos_x, double pos_y, double pos_z,
    double target_x, double target_y, double target_z,
    double up_x, double up_y, double up_z,
    double fovy,
    int projection
) {
    Camera3D camera = { 0 };
    camera.position = (Vector3){ (float)pos_x, (float)pos_y, (float)pos_z };
    camera.target = (Vector3){ (float)target_x, (float)target_y, (float)target_z };
    camera.up = (Vector3){ (float)up_x, (float)up_y, (float)up_z };
    camera.fovy = (float)fovy;
    camera.projection = projection;
    BeginMode3D(camera);
}

void mo_rl_end_mode3d(void) {
    EndMode3D();
}

void mo_rl_draw_grid(int slices, double spacing) {
    DrawGrid(slices, (float)spacing);
}

void mo_rl_draw_cube(
    double x, double y, double z,
    double width, double height, double length,
    int r, int g, int b, int a
) {
    DrawCube(
        (Vector3){ (float)x, (float)y, (float)z },
        (float)width,
        (float)height,
        (float)length,
        (Color){ (unsigned char)r, (unsigned char)g, (unsigned char)b, (unsigned char)a }
    );
}

void mo_rl_draw_cube_wires(
    double x, double y, double z,
    double width, double height, double length,
    int r, int g, int b, int a
) {
    DrawCubeWires(
        (Vector3){ (float)x, (float)y, (float)z },
        (float)width,
        (float)height,
        (float)length,
        (Color){ (unsigned char)r, (unsigned char)g, (unsigned char)b, (unsigned char)a }
    );
}

void mo_rl_draw_text(const char *text, int x, int y, int size, int r, int g, int b, int a) {
    DrawText(text, x, y, size, (Color){ (unsigned char)r, (unsigned char)g, (unsigned char)b, (unsigned char)a });
}

double mo_rl_get_time(void) {
    return GetTime();
}
