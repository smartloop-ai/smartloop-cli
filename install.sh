#!/bin/sh
# Smartloop CLI installer.
#
#   curl -fsSL https://raw.githubusercontent.com/smartloop-ai/smartloop-cli/main/install.sh | sh
#
# Environment:
#   SMARTLOOP_CLI_VERSION      Version to install (default: latest release)
#   SMARTLOOP_CLI_INSTALL_DIR  Install directory (default: $HOME/.smartloop/bin)
set -eu

REPO="smartloop-ai/smartloop-cli"
INSTALL_DIR="${SMARTLOOP_CLI_INSTALL_DIR:-$HOME/.smartloop/bin}"

# Colors, only when stdout is a terminal.
if [ -t 1 ]; then
    GREEN='\033[1;32m'; RED='\033[0;31m'; BOLD='\033[1m'; NC='\033[0m'
else
    GREEN=''; RED=''; BOLD=''; NC=''
fi

info() { printf '%s\n' "$1"; }
error() { printf "${RED}Error:${NC} %s\n" "$1" >&2; exit 1; }

need() {
    command -v "$1" >/dev/null 2>&1 || error "$1 is required but not installed"
}

# Map uname output onto the Rust target triple the release is built for.
detect_target() {
    os="$(uname -s)"
    arch="$(uname -m)"

    case "$os" in
        Linux)  os_part="unknown-linux-musl" ;;
        Darwin) os_part="apple-darwin" ;;
        MINGW*|MSYS*|CYGWIN*) os_part="pc-windows-msvc" ;;
        *) error "Unsupported OS: $os" ;;
    esac

    case "$arch" in
        x86_64|amd64)  arch_part="x86_64" ;;
        arm64|aarch64) arch_part="aarch64" ;;
        *) error "Unsupported architecture: $arch" ;;
    esac

    if [ "$os_part" = "pc-windows-msvc" ] && [ "$arch_part" != "x86_64" ]; then
        error "Windows builds are published for x86_64 only"
    fi

    TARGET="${arch_part}-${os_part}"
    if [ "$os_part" = "pc-windows-msvc" ]; then
        ARCHIVE_EXT="zip"
        BIN_NAME="smartloop.exe"
    else
        ARCHIVE_EXT="tar.gz"
        BIN_NAME="smartloop"
    fi
}

# Latest release tag, read straight off the redirect so no JSON parser and no
# API rate limit are involved.  A repo with no releases redirects to
# /releases instead of /releases/tag/vX.Y.Z, so require the /tag/ segment
# rather than handing the caller back a URL as if it were a version.
latest_version() {
    url="$(curl -fsSLI -o /dev/null -w '%{url_effective}' \
        "https://github.com/${REPO}/releases/latest")" || return 1

    case "$url" in
        */tag/v*) printf '%s\n' "${url##*/tag/v}" ;;
        *) return 1 ;;
    esac
}

verify_checksum() {
    archive="$1"
    sums="$2"

    if command -v sha256sum >/dev/null 2>&1; then
        actual="$(sha256sum "$archive" | cut -d' ' -f1)"
    elif command -v shasum >/dev/null 2>&1; then
        actual="$(shasum -a 256 "$archive" | cut -d' ' -f1)"
    else
        info "No sha256 tool found; skipping checksum verification"
        return 0
    fi

    expected="$(grep " $(basename "$archive")\$" "$sums" | cut -d' ' -f1)"
    [ -n "$expected" ] || error "No checksum published for $(basename "$archive")"
    [ "$actual" = "$expected" ] || error "Checksum mismatch for $(basename "$archive")"
}

main() {
    need curl
    need tar

    detect_target

    VERSION="${SMARTLOOP_CLI_VERSION:-$(latest_version || true)}"
    [ -n "$VERSION" ] || error "Could not determine the latest release; set SMARTLOOP_CLI_VERSION"
    VERSION="${VERSION#v}"

    NAME="smartloop-${VERSION}-${TARGET}"
    ARCHIVE="${NAME}.${ARCHIVE_EXT}"
    BASE_URL="https://github.com/${REPO}/releases/download/v${VERSION}"

    TMP_DIR="$(mktemp -d)"
    trap 'rm -rf "$TMP_DIR"' EXIT INT TERM

    info "Installing ${BOLD}smartloop ${VERSION}${NC} (${TARGET})"

    curl -fsSL "${BASE_URL}/${ARCHIVE}" -o "${TMP_DIR}/${ARCHIVE}" \
        || error "Download failed: ${BASE_URL}/${ARCHIVE}"
    curl -fsSL "${BASE_URL}/SHA256SUMS" -o "${TMP_DIR}/SHA256SUMS" \
        || error "Could not download SHA256SUMS"

    verify_checksum "${TMP_DIR}/${ARCHIVE}" "${TMP_DIR}/SHA256SUMS"

    if [ "$ARCHIVE_EXT" = "zip" ]; then
        need unzip
        unzip -q "${TMP_DIR}/${ARCHIVE}" -d "$TMP_DIR"
    else
        tar -xzf "${TMP_DIR}/${ARCHIVE}" -C "$TMP_DIR"
    fi

    mkdir -p "$INSTALL_DIR"
    install -m 755 "${TMP_DIR}/${NAME}/${BIN_NAME}" "${INSTALL_DIR}/${BIN_NAME}" 2>/dev/null \
        || { cp "${TMP_DIR}/${NAME}/${BIN_NAME}" "${INSTALL_DIR}/${BIN_NAME}" && chmod 755 "${INSTALL_DIR}/${BIN_NAME}"; }

    printf "${GREEN}Installed${NC} %s\n" "${INSTALL_DIR}/${BIN_NAME}"

    case ":${PATH}:" in
        *":${INSTALL_DIR}:"*) ;;
        *)
            info ""
            info "Add it to your PATH:"
            info "    export PATH=\"${INSTALL_DIR}:\$PATH\""
            ;;
    esac
}

main "$@"
