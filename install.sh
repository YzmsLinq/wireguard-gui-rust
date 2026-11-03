#!/bin/bash
# 构建并把 wgr 安装到系统。
# wgr 自身会通过 pkexec/sudo 提权，不再需要启动脚本。
set -e

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if ! command -v cargo >/dev/null 2>&1; then
    echo "未找到 cargo，请先安装 Rust 工具链: https://rustup.rs" >&2
    exit 1
fi

echo "==> 构建中 (cargo build --release)..."
(cd "$DIR" && cargo build --release)

echo "==> 安装二进制..."
install -m 755 "$DIR/target/release/wgr" /usr/local/bin/wgr

echo "==> 安装图标..."
install -D -m 644 "$DIR/resources/icons/wireguard-icon.png" \
    /usr/share/icons/hicolor/256x256/apps/wgr.png

echo "==> 安装桌面文件..."
sed -e 's|^Exec=.*|Exec=/usr/local/bin/wgr|' \
    -e 's|^Icon=.*|Icon=wgr|' \
    "$DIR/resources/wgr.desktop" > /usr/share/applications/wgr.desktop

# 旧版本（wgp）通过 wgp-run 启动，wgr 现在自己提权，该脚本已无用。
if [ -e /usr/local/bin/wgp-run ]; then
    echo "==> 删除旧启动器 wgp-run..."
    rm -f /usr/local/bin/wgp-run
fi

update-desktop-database /usr/share/applications >/dev/null 2>&1 || true
gtk-update-icon-cache -q /usr/share/icons/hicolor >/dev/null 2>&1 || true

echo "安装完成，可以在应用菜单中找到 wgr。"
