#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_NAME="rust-and-vulkan"
APPDIR="${ROOT_DIR}/AppDir"
DIST_DIR="${ROOT_DIR}/dist"
TOOLS_DIR="${ROOT_DIR}/tools"
TARGET_BIN="${ROOT_DIR}/target/release/${APP_NAME}"
DESKTOP_FILE="${APPDIR}/${APP_NAME}.desktop"
ICON_FILE="${APPDIR}/${APP_NAME}.png"

if [[ "$(uname -s)" != "Linux" ]]; then
  echo "This script builds AppImage on Linux only."
  exit 1
fi

mkdir -p "${DIST_DIR}" "${TOOLS_DIR}"

copy_runtime_deps() {
  local elf="$1"
  local out_dir="$2"

  mapfile -t deps < <(
    ldd "$elf" | awk '
      /=> \/.*/ { print $3 }
      /^\/.*/ { print $1 }
    ' | sed '/^$/d' | sort -u
  )

  for dep in "${deps[@]}"; do
    local base
    base="$(basename "$dep")"

    case "$base" in
      linux-vdso.so.*|ld-linux*.so.*|libc.so.*|libm.so.*|libgcc_s.so.*)
        continue
        ;;
    esac

    cp -L "$dep" "$out_dir/$base"
  done
}

echo "==> Building release binary"
cargo build --release

if [[ ! -f "${TARGET_BIN}" ]]; then
  echo "Release binary not found at ${TARGET_BIN}"
  exit 1
fi

echo "==> Preparing AppDir"
rm -rf "${APPDIR}"
mkdir -p \
  "${APPDIR}/usr/bin" \
  "${APPDIR}/usr/lib" \
  "${APPDIR}/usr/share/applications" \
  "${APPDIR}/usr/share/icons/hicolor/256x256/apps"

cp "${TARGET_BIN}" "${APPDIR}/usr/bin/${APP_NAME}"
chmod +x "${APPDIR}/usr/bin/${APP_NAME}"

echo "==> Copying runtime shared libraries (ldd)"
copy_runtime_deps "${APPDIR}/usr/bin/${APP_NAME}" "${APPDIR}/usr/lib"

# Runtime assets expected by relative paths in the app.
cp -r "${ROOT_DIR}/shaders" "${APPDIR}/usr/bin/shaders"

# Optional automation examples
for f in \
  "${ROOT_DIR}/housekeeping_config.toml" \
  "${ROOT_DIR}/power_management.toml" \
  "${ROOT_DIR}/events_diagnostics.toml" \
  "${ROOT_DIR}/complete_automation.json"; do
  if [[ -f "$f" ]]; then
    cp "$f" "${APPDIR}/usr/bin/"
  fi
done

cat > "${APPDIR}/AppRun" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export LD_LIBRARY_PATH="${HERE}/usr/lib:${LD_LIBRARY_PATH:-}"
cd "${HERE}/usr/bin"
exec "./rust-and-vulkan" "$@"
EOF
chmod +x "${APPDIR}/AppRun"

cat > "${DESKTOP_FILE}" <<EOF
[Desktop Entry]
Type=Application
Name=Rust and Vulkan
Exec=${APP_NAME}
Icon=${APP_NAME}
Categories=Development;
Terminal=false
EOF

# Small placeholder PNG icon (16x16 transparent) to satisfy AppImage metadata.
base64 -d > "${ICON_FILE}" <<'EOF'
iVBORw0KGgoAAAANSUhEUgAAABAAAAAQCAQAAAC1+jfqAAAAE0lEQVR42mNk+M+AFzBhqAQA
D8ABxQmS6QAAAABJRU5ErkJggg==
EOF
cp "${ICON_FILE}" "${APPDIR}/usr/share/icons/hicolor/256x256/apps/${APP_NAME}.png"

APPIMAGETOOL="${TOOLS_DIR}/appimagetool-x86_64.AppImage"

if [[ ! -f "${APPIMAGETOOL}" ]]; then
  echo "==> Downloading appimagetool"
  curl -L -o "${APPIMAGETOOL}" \
    "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage"
fi

chmod +x "${APPIMAGETOOL}"

export ARCH=x86_64
export APPIMAGE_EXTRACT_AND_RUN=1

OUTPUT_APPIMAGE="${DIST_DIR}/${APP_NAME}-x86_64.AppImage"

echo "==> Creating ${OUTPUT_APPIMAGE}"
"${APPIMAGETOOL}" "${APPDIR}" "${OUTPUT_APPIMAGE}"

chmod +x "${OUTPUT_APPIMAGE}"

echo

echo "Done. Portable executable created at:"
echo "  ${OUTPUT_APPIMAGE}"
