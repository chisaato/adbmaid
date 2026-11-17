# adbmaid Justfile
# 常用开发、检查与本地一键打包命令

set shell := ["bash", "-uc"]

bundle_dir := "src-tauri/target/release/bundle"
out_dir := "dist/release"

# 默认列出所有可用命令
default:
    @just --list

# 启动 Tauri 桌面端开发模式
dev:
    pnpm tauri dev

# 仅启动前端 Vite 预览
web-dev:
    pnpm dev

# 安装前端依赖
install:
    pnpm install

# 执行与 CI 一致的前后端检查与测试
check:
    pnpm build
    cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test

# 格式化 Rust 代码
fmt:
    cd src-tauri && cargo fmt

# 仅运行 Rust 单元测试
test:
    cd src-tauri && cargo test

# 清理构建缓存与输出目录
clean:
    rm -rf dist
    cd src-tauri && cargo clean

# 根据 src-tauri/icons/icon.png 重新生成全平台图标
icons:
    pnpm tauri icon src-tauri/icons/icon.png

# 收集 Tauri 打包产物到 dist/release/
[private]
collect-bundles:
    #!/usr/bin/env bash
    set -euo pipefail
    mkdir -p "{{out_dir}}"
    find "{{bundle_dir}}" -maxdepth 2 -type f \( \
        -name '*.AppImage' -o \
        -name '*.deb' -o \
        -name '*.rpm' -o \
        -name '*.exe' -o \
        -name '*.msi' -o \
        -name '*.dmg' \
    \) -exec cp -vf {} "{{out_dir}}/" \;
    echo "==> 产物已汇总至: {{out_dir}}/"

# 一键打包当前平台全部原生安装包（Linux: AppImage/deb/rpm; Win: exe/msi; macOS: dmg）
pack:
    APPIMAGE_EXTRACT_AND_RUN=1 pnpm tauri build
    @just collect-bundles

# 仅打包 Linux AppImage
pack-appimage:
    APPIMAGE_EXTRACT_AND_RUN=1 pnpm tauri build --bundles appimage
    @just collect-bundles

# 仅打包 Linux deb
pack-deb:
    pnpm tauri build --bundles deb
    @just collect-bundles

# 仅打包 Linux rpm
pack-rpm:
    pnpm tauri build --bundles rpm
    @just collect-bundles

# 本地一键构建 Linux Flatpak 单文件包 (依赖 flatpak-builder 与 bsdtar)
pack-flatpak:
    #!/usr/bin/env bash
    set -euo pipefail
    pnpm tauri build --bundles deb
    DEB_FILE="$(find "{{bundle_dir}}/deb" -maxdepth 1 -name '*.deb' | head -n 1)"
    if [ -z "$DEB_FILE" ]; then
        echo "错误：未找到 .deb 安装包" >&2
        exit 1
    fi

    FP_DIR="packaging/flatpak"
    rm -rf "$FP_DIR/rootfs" "$FP_DIR/repo" "$FP_DIR/.flatpak-builder"
    mkdir -p "$FP_DIR/rootfs" "{{out_dir}}"

    # 使用 bsdtar 解包 .deb（兼容 Fedora 与 Debian/Ubuntu）
    TMP_DEB="$(mktemp -d)"
    trap 'rm -rf "$TMP_DEB"' EXIT
    bsdtar -xf "$DEB_FILE" -C "$TMP_DEB"
    bsdtar -xf "$TMP_DEB"/data.tar.* -C "$FP_DIR/rootfs"

    cat << 'EOF' > "$FP_DIR/com.adbmaid.app.desktop"
    [Desktop Entry]
    Name=adbmaid
    Comment=Android 设备可视化工作台
    Exec=adbmaid
    Icon=com.adbmaid.app
    Type=Application
    Categories=Development;Utility;
    Terminal=false
    EOF

    cat << 'EOF' > "$FP_DIR/com.adbmaid.app.metainfo.xml"
    <?xml version="1.0" encoding="UTF-8"?>
    <component type="desktop-application">
      <id>com.adbmaid.app</id>
      <name>adbmaid</name>
      <summary>Android 设备可视化工作台</summary>
      <metadata_license>MIT</metadata_license>
      <project_license>MIT</project_license>
      <description>
        <p>基于 Tauri v2 的 Android ADB 设备可视化工作台。</p>
      </description>
      <launchable type="desktop-id">com.adbmaid.app.desktop</launchable>
    </component>
    EOF

    flatpak-builder --user --force-clean --repo="$FP_DIR/repo" "$FP_DIR/build-dir" "$FP_DIR/com.adbmaid.app.yml"
    flatpak build-bundle "$FP_DIR/repo" "{{out_dir}}/adbmaid-x86_64.flatpak" com.adbmaid.app
    rm -rf "$FP_DIR/rootfs" "$FP_DIR/build-dir" "$FP_DIR/repo" "$FP_DIR/.flatpak-builder" "$FP_DIR/com.adbmaid.app.desktop" "$FP_DIR/com.adbmaid.app.metainfo.xml"
    echo "==> Flatpak 构建完成: {{out_dir}}/adbmaid-x86_64.flatpak"
