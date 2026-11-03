#!/bin/bash
# 以 root 权限运行 wgp。
# 手动运行：./run.sh
set -e

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

BINARY=""
#for CANDIDATE in "$DIR/target/release/wgp" "$DIR/target/debug/wgp" "$DIR/wgp" /usr/local/bin/wgp; do
for CANDIDATE in "$DIR/wgp" /usr/local/bin/wgp; do
    if [ -x "$CANDIDATE" ]; then
        BINARY="$CANDIDATE"
        break
    fi
done

#if [ -z "$BINARY" ]; then
#    echo "未找到 wgp 可执行文件，请先运行: cargo build --release" >&2
#    exit 1
#fi

# 已经是 root（例如 sudo ./run.sh）
if [ "$(id -u)" = "0" ]; then
    exec "$BINARY"
fi

if ! command -v pkexec >/dev/null 2>&1; then
    echo "需要 pkexec 提权，请直接用 sudo 运行: sudo $BINARY" >&2
    exit 1
fi

# pkexec 会清空环境变量，这里把图形会话需要的那几个传进去
exec pkexec \
    env \
    DISPLAY="${DISPLAY:-:0}" \
    XAUTHORITY="${XAUTHORITY:-$HOME/.Xauthority}" \
    WAYLAND_DISPLAY="${WAYLAND_DISPLAY:-}" \
    XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-}" \
    "$BINARY"
