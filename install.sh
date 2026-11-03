#!/bin/bash
# 构建并把 wgp 安装到系统。
set -e

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if ! command -v cargo >/dev/null 2>&1; then
    echo "未找到 cargo，请先安装 Rust 工具链: https://rustup.rs" >&2
    exit 1
fi

echo "==> 构建中 (cargo build --release)..."
(cd "$DIR" && cargo build --release)

echo "==> 安装二进制..."
install -m 755 "$DIR/target/release/wgp" /usr/local/bin/wgp

echo "==> 安装启动器..."
install -m 755 "$DIR/run.sh" /usr/local/bin/wgp-run

echo "==> 安装图标..."
install -D -m 644 "$DIR/resources/icons/wireguard-icon.png" \
    /usr/share/icons/hicolor/256x256/apps/wgp.png

echo "==> 安装桌面文件..."
sed -e 's|^Exec=.*|Exec=/usr/local/bin/wgp-run|' \
    -e 's|^Icon=.*|Icon=wgp|' \
    "$DIR/resources/wgp.desktop" > /usr/share/applications/wgp.desktop

echo "安装完成，可以在应用菜单中找到 wgp。"
