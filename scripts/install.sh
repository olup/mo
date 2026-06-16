#!/usr/bin/env sh
set -eu

repo="${MO_REPO:-olup/mo}"
version="${MO_VERSION:-v0.1.0-pre-alpha}"
prefix="${MO_PREFIX:-$HOME/.local/mo}"
update_profile="${MO_INSTALL_UPDATE_PROFILE:-1}"

platform="$(uname -s)-$(uname -m)"
case "$platform" in
    Darwin-arm64)
        package="mo-aarch64-apple-darwin"
        ;;
    Darwin-x86_64)
        package="mo-x86_64-apple-darwin"
        ;;
    Linux-aarch64 | Linux-arm64)
        package="mo-aarch64-unknown-linux-gnu"
        ;;
    Linux-x86_64)
        package="mo-x86_64-unknown-linux-gnu"
        ;;
    *)
        echo "unsupported platform: $platform" >&2
        exit 1
        ;;
esac

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT INT TERM

url="https://github.com/$repo/releases/download/$version/$package.tar.gz"
archive="$tmp/mo.tar.gz"

if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$url" -o "$archive"
elif command -v wget >/dev/null 2>&1; then
    wget -qO "$archive" "$url"
else
    echo "install requires curl or wget" >&2
    exit 1
fi

rm -rf "$prefix"
mkdir -p "$prefix"
tar -xzf "$archive" -C "$prefix" --strip-components=1

smoke="$tmp/smoke.mo"
cat > "$smoke" <<'MO'
fn main() -> Int {
    return 0
}
MO

"$prefix/mo" check "$smoke" >/dev/null

path_entry="$prefix"
case "$path_entry" in
    "$HOME"/*)
        path_entry="\$HOME/${path_entry#"$HOME"/}"
        ;;
esac
path_line="export PATH=\"$path_entry:\$PATH\""

profile=""
if [ "$update_profile" != "0" ]; then
    if [ "${MO_PROFILE:-}" ]; then
        profile="$MO_PROFILE"
    elif [ -n "${ZSH_VERSION:-}" ]; then
        profile="$HOME/.zshrc"
    elif [ -n "${BASH_VERSION:-}" ]; then
        profile="$HOME/.bashrc"
    elif [ -f "$HOME/.zshrc" ]; then
        profile="$HOME/.zshrc"
    elif [ -f "$HOME/.bashrc" ]; then
        profile="$HOME/.bashrc"
    else
        profile="$HOME/.profile"
    fi

    mkdir -p "$(dirname "$profile")"
    touch "$profile"
    if ! grep -F "$path_line" "$profile" >/dev/null 2>&1; then
        {
            echo ""
            echo "# mo"
            echo "$path_line"
        } >> "$profile"
    fi
fi

echo "mo installed to $prefix"
if [ "$profile" ]; then
    echo "updated $profile"
    echo "restart your shell or run:"
    echo "  . \"$profile\""
else
    echo "add this to your shell profile:"
    echo "  $path_line"
fi
