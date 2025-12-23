#!/bin/bash

# Memexia 构建脚本

set -e

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}Building Memexia...${NC}"

# 检查 Rust 安装
if ! command -v cargo &> /dev/null; then
    echo -e "${RED}Error: Cargo not found. Please install Rust first.${NC}"
    exit 1
fi

# 清理之前的构建
echo -e "${YELLOW}Cleaning previous build...${NC}"
cargo clean

# 设置构建标志
BUILD_FLAGS=""
RELEASE=false

# 解析参数
while [[ $# -gt 0 ]]; do
    case $1 in
        --release)
            RELEASE=true
            BUILD_FLAGS="--release"
            shift
            ;;
        --features)
            FEATURES="$2"
            BUILD_FLAGS="$BUILD_FLAGS --features $FEATURES"
            shift 2
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

# 构建
echo -e "${YELLOW}Building with flags: $BUILD_FLAGS${NC}"
if [ "$RELEASE" = true ]; then
    echo -e "${GREEN}Building release version...${NC}"
    cargo build --release
    
    # 优化二进制文件
    if command -v strip &> /dev/null; then
        echo -e "${YELLOW}Stripping binary...${NC}"
        strip target/release/memexia
    fi
else
    echo -e "${GREEN}Building debug version...${NC}"
    cargo build
fi

# 运行测试
echo -e "${YELLOW}Running tests...${NC}"
cargo test

# 运行 Clippy 检查
echo -e "${YELLOW}Running Clippy...${NC}"
cargo clippy -- -D warnings

# 运行格式化检查
echo -e "${YELLOW}Checking formatting...${NC}"
cargo fmt -- --check

echo -e "${GREEN}Build complete!${NC}"

# 显示二进制信息
if [ "$RELEASE" = true ]; then
    BINARY_PATH="target/release/memexia"
else
    BINARY_PATH="target/debug/memexia"
fi

if [ -f "$BINARY_PATH" ]; then
    echo -e "${YELLOW}Binary location: $BINARY_PATH${NC}"
    echo -e "${YELLOW}Binary size: $(du -h "$BINARY_PATH" | cut -f1)${NC}"
fi