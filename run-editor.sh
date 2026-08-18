#!/usr/bin/env bash
# Strips VS Code snap's GTK/GDK env vars and sets webkit workarounds before launching.
#
# Problem 1 — libpthread symbol lookup error:
#   VS Code (Snap, base: core20) sets GDK_PIXBUF_MODULEDIR, GIO_MODULE_DIR, GTK_PATH, etc.
#   to snap-packaged gdk-pixbuf loaders compiled for glibc 2.31.  Those loaders embed
#   DT_RPATH=/snap/core20/current/lib/x86_64-linux-gnu, so when GTK dlopen's them they
#   drag in core20's libpthread.so.0, which fails to find __libc_pthread_init in the
#   host glibc 2.34+.  Fix: unset all snap GTK module paths.
#
# Problem 2 — GLXBadWindow / winit panic + wgpu swap chain stutter:
#   webkit2gtk's DMABUF renderer (Linux zero-copy path) causes two issues on NVIDIA + X11:
#   (a) Its init calls glXDestroyWindow on an invalid drawable → GLXBadWindow stored in the
#       shared Xlib error queue → winit's XSetICFocus handler reads it and panics.
#   (b) Its software-paint fallback floods the parent window with XPutImage calls, which
#       cause NVIDIA's Vulkan WSI to mark the wgpu swap chain out-of-date every frame,
#       producing repeated "surface has changed" stutter warnings.
#   Fix: WEBKIT_DISABLE_DMABUF_RENDERER=1 makes webkit use EGL compositing on its own
#   child surface, avoiding both GLX operations and direct X11 painting on the parent.
set -e
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

RELEASE="${SCRIPT_DIR}/target/release/xrds-editor"
DEBUG="${SCRIPT_DIR}/target/debug/xrds-editor"

if   [ -f "$RELEASE" ]; then BINARY="$RELEASE"
elif [ -f "$DEBUG"   ]; then BINARY="$DEBUG"
else
    echo "Binary not found — run: cargo build -p xrds-editor [--release]" >&2
    exit 1
fi

exec env \
    -u LD_LIBRARY_PATH \
    -u GDK_PIXBUF_MODULEDIR \
    -u GDK_PIXBUF_MODULE_FILE \
    -u GIO_MODULE_DIR \
    -u GTK_IM_MODULE_FILE \
    -u GTK_PATH \
    -u GTK_EXE_PREFIX \
    WEBKIT_DISABLE_DMABUF_RENDERER=1 \
    WINIT_UNIX_BACKEND=x11 \
    "$BINARY" "$@"
