# Raylib 3D demo

This demo shows a Mo package that distributes a precompiled C library with its
own `mo.toml`. The application imports the Mo `raylib` package only; the native
static libraries and macOS frameworks are declared by the package.

The package uses a small C shim so Mo calls a simple ABI made of integers,
`Float64`, and strings instead of passing raylib structs by value.

Build raylib and the shim:

```sh
cd demos/raylib_3d
mo exec prepare
```

Then build and run the Mo app:

```sh
mo exec build
/tmp/mo_raylib_3d_demo
```

Or compile and run directly:

```sh
mo exec run
```

The demo opens a native raylib window and renders a small 3D scene.
