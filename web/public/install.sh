#!/bin/sh
# Envbyte installer for macOS and Linux.
#
#   curl -fsSL https://envbyte.trackedge.in/install.sh | sh
#
# Downloads the release archive for this machine from GitHub, checks it against
# the SHA-256 published with the release, and installs `envbyte` into
# ~/.envbyte/bin. Nothing is run as root.
#
# Settings (environment variables):
#   ENVBYTE_VERSION=0.4.0       install that version instead of the latest
#   ENVBYTE_INSTALL_DIR=<dir>   install somewhere other than ~/.envbyte/bin
#   ENVBYTE_NO_MODIFY_PATH=1    never edit shell startup files
#
# Everything lives in functions and runs from the last line, so a download cut
# off halfway can never execute a partial script.

set -eu

REPO="NYLONXD/EnvByte_CLI"

main() {
    need uname
    need tar
    need mkdir
    need mktemp

    target=$(detect_target)
    version="${ENVBYTE_VERSION:-}"
    version="${version#v}"
    if [ -n "$version" ]; then
        base="https://github.com/$REPO/releases/download/v$version"
    else
        base="https://github.com/$REPO/releases/latest/download"
    fi
    install_dir="${ENVBYTE_INSTALL_DIR:-$HOME/.envbyte/bin}"
    archive="envbyte-$target.tar.gz"

    scratch=$(mktemp -d)
    trap 'rm -rf "$scratch"' EXIT
    trap 'exit 1' INT TERM

    say "Downloading envbyte ${version:-(latest)} for $target"
    download "$base/$archive" "$scratch/$archive"
    download "$base/$archive.sha256" "$scratch/$archive.sha256"
    verify_checksum "$scratch/$archive" "$scratch/$archive.sha256"

    tar -xzf "$scratch/$archive" -C "$scratch"
    binary="$scratch/envbyte-$target/envbyte"
    [ -f "$binary" ] || fail "the archive does not contain envbyte-$target/envbyte"

    mkdir -p "$install_dir"
    # Copy beside the destination, then rename over it: the swap is atomic, and
    # a copy of envbyte that is running right now keeps working.
    cp "$binary" "$install_dir/.envbyte.new"
    chmod 755 "$install_dir/.envbyte.new"
    mv -f "$install_dir/.envbyte.new" "$install_dir/envbyte"

    installed=$("$install_dir/envbyte" --version 2>/dev/null || true)
    [ -n "$installed" ] || fail "installed $install_dir/envbyte, but it did not run"
    success "Installed $installed to $install_dir/envbyte"

    ensure_on_path "$install_dir"

    printf '\n'
    say "Get started:"
    say "  envbyte register     create an account"
    say "  envbyte create app   start a project in this directory"
    say "Docs: https://envbyte.trackedge.in"
}

detect_target() {
    os=$(uname -s)
    arch=$(uname -m)
    case "$arch" in
        x86_64 | amd64) arch=x86_64 ;;
        arm64 | aarch64) arch=aarch64 ;;
        *) fail "unsupported CPU architecture: $arch. Install from source with: cargo install envbyte" ;;
    esac

    case "$os" in
        Darwin)
            # A shell running under Rosetta reports x86_64 on Apple Silicon;
            # the native build is the right one there.
            if [ "$arch" = x86_64 ] && [ "$(sysctl -n sysctl.proc_translated 2>/dev/null || echo 0)" = 1 ]; then
                arch=aarch64
            fi
            echo "$arch-apple-darwin"
            ;;
        Linux)
            if [ "$arch" = x86_64 ]; then
                # Statically linked, so it runs on glibc and musl (Alpine) alike.
                echo "x86_64-unknown-linux-musl"
            elif is_musl; then
                fail "there is no prebuilt envbyte for ARM64 musl (e.g. Alpine). Install from source with: cargo install envbyte"
            else
                echo "aarch64-unknown-linux-gnu"
            fi
            ;;
        MINGW* | MSYS* | CYGWIN* | Windows_NT)
            fail "on Windows, run this instead: powershell -c \"irm https://envbyte.trackedge.in/install.ps1 | iex\""
            ;;
        *)
            fail "unsupported operating system: $os. Install from source with: cargo install envbyte"
            ;;
    esac
}

is_musl() {
    if command -v ldd >/dev/null 2>&1 && ldd --version 2>&1 | grep -qi musl; then
        return 0
    fi
    ls /lib/ld-musl-* >/dev/null 2>&1
}

download() {
    if command -v curl >/dev/null 2>&1; then
        curl --proto '=https' --tlsv1.2 -fsSL --retry 3 -o "$2" "$1" ||
            fail "could not download $1"
    elif command -v wget >/dev/null 2>&1; then
        wget --https-only -q -O "$2" "$1" || fail "could not download $1"
    else
        fail "curl or wget is required"
    fi
}

# A secrets tool must not install a binary it could not verify, so a missing
# hashing tool is an error rather than a skipped check.
verify_checksum() {
    expected=$(cut -d ' ' -f 1 <"$2")
    if command -v sha256sum >/dev/null 2>&1; then
        actual=$(sha256sum "$1" | cut -d ' ' -f 1)
    elif command -v shasum >/dev/null 2>&1; then
        actual=$(shasum -a 256 "$1" | cut -d ' ' -f 1)
    else
        fail "sha256sum or shasum is required to verify the download"
    fi
    if [ -z "$expected" ] || [ "$expected" != "$actual" ]; then
        fail "checksum mismatch for $(basename "$1"): expected $expected, got $actual"
    fi
    say "Checksum verified"
}

ensure_on_path() {
    case ":$PATH:" in
        *":$1:"*) return 0 ;;
    esac

    if [ "$1" = "$HOME/.envbyte/bin" ]; then
        # Written literally, so the profile expands $HOME itself at login.
        # shellcheck disable=SC2016
        dir_expr='$HOME/.envbyte/bin'
    else
        dir_expr=$1
    fi

    if [ "${ENVBYTE_NO_MODIFY_PATH:-}" = 1 ]; then
        say "Add envbyte to your PATH:  export PATH=\"$dir_expr:\$PATH\""
        return 0
    fi

    case "$(basename "${SHELL:-sh}")" in
        zsh) add_line "${ZDOTDIR:-$HOME}/.zshrc" "export PATH=\"$dir_expr:\$PATH\"" ;;
        bash)
            add_line "$HOME/.bashrc" "export PATH=\"$dir_expr:\$PATH\""
            # macOS Terminal starts login shells, which read .bash_profile.
            if [ "$(uname -s)" = Darwin ]; then
                add_line "$HOME/.bash_profile" "export PATH=\"$dir_expr:\$PATH\""
            fi
            ;;
        fish)
            mkdir -p "$HOME/.config/fish/conf.d"
            add_line "$HOME/.config/fish/conf.d/envbyte.fish" "fish_add_path \"$dir_expr\""
            ;;
        *) add_line "$HOME/.profile" "export PATH=\"$dir_expr:\$PATH\"" ;;
    esac
    say "Open a new terminal (or run: export PATH=\"$dir_expr:\$PATH\") to use envbyte."
}

add_line() {
    if [ -f "$1" ] && grep -qF "$2" "$1"; then
        return 0
    fi
    printf '\n# envbyte\n%s\n' "$2" >>"$1"
    say "Added envbyte to PATH in $1"
}

need() {
    command -v "$1" >/dev/null 2>&1 || fail "$1 is required"
}

if [ -t 1 ]; then
    bold=$(printf '\033[1m')
    green=$(printf '\033[32m')
    red=$(printf '\033[31m')
    reset=$(printf '\033[0m')
else
    bold=""
    green=""
    red=""
    reset=""
fi

say() {
    printf '%s\n' "$1"
}

success() {
    printf '%s%s%s\n' "$green$bold" "$1" "$reset"
}

fail() {
    printf '%serror:%s %s\n' "$red$bold" "$reset" "$1" >&2
    exit 1
}

main "$@"
