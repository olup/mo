#!/usr/bin/env sh
set -eu

version="${RAYLIB_VERSION:-6.0}"
arch="${RAYLIB_ARCH:-arm64}"
root_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
package_dir="$root_dir/app/packages/raylib"
vendor_dir="$package_dir/vendor"
build_root="$vendor_dir/build"
archive="$vendor_dir/raylib-$version.tar.gz"
source_dir="$build_root/raylib-$version"
cmake_dir="$build_root/cmake"

mkdir -p "$vendor_dir" "$build_root"

if [ ! -f "$archive" ]; then
    curl -fsSL "https://github.com/raysan5/raylib/archive/refs/tags/$version.tar.gz" -o "$archive"
fi

if [ ! -d "$source_dir" ]; then
    tar -xzf "$archive" -C "$build_root"
fi

cmake -S "$source_dir" -B "$cmake_dir" \
    -DBUILD_SHARED_LIBS=OFF \
    -DBUILD_EXAMPLES=OFF \
    -DCMAKE_OSX_ARCHITECTURES="$arch" \
    -DCMAKE_BUILD_TYPE=Release
cmake --build "$cmake_dir" --target raylib --config Release

raylib_archive="$(find "$cmake_dir" -name libraylib.a -print -quit)"
if [ -z "$raylib_archive" ]; then
    echo "prepare-raylib.sh: could not find built libraylib.a" >&2
    exit 1
fi

cp "$raylib_archive" "$vendor_dir/libraylib.a"

cc -arch "$arch" -I "$source_dir/src" -c "$vendor_dir/mo_raylib_shim.c" -o "$vendor_dir/mo_raylib_shim.o"
ar rcs "$vendor_dir/libraylib_mo.a" "$vendor_dir/mo_raylib_shim.o"

echo "Built:"
echo "  $vendor_dir/libraylib.a"
echo "  $vendor_dir/libraylib_mo.a"
