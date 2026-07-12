# 构建与兼容性验证

## 兼容性策略

本项目提供两种Linux binary以最大化兼容性：

1. **glibc版本** (`linux-x64-gnu.node`)
   - 在Ubuntu 20.04 (GLIBC 2.31)上构建
   - 兼容所有GLIBC 2.31+的系统
   - 覆盖大部分Linux发行版

2. **musl版本** (`linux-x64-musl.node`)
   - 静态链接，无GLIBC依赖
   - 用于Alpine、容器环境等

## 验证GLIBC依赖

检查构建出的glibc版本依赖：

```bash
# 构建后检查依赖的GLIBC版本
objdump -T pi-switch-native.linux-x64-gnu.node | grep GLIBC

# 应该看到的最高版本不超过 GLIBC_2.31
# ✅ GLIBC_2.2.5, GLIBC_2.3.4, GLIBC_2.31
# ❌ GLIBC_2.39 (太新)
```

---

## 安装musl工具链

### Linux
```bash
# Ubuntu/Debian
sudo apt-get install musl-tools

# 添加Rust musl target
rustup target add x86_64-unknown-linux-musl
```

### macOS (交叉编译)
```bash
# 安装musl交叉编译工具
brew install messense/macos-cross-toolchains/x86_64-unknown-linux-musl

# 添加Rust target
rustup target add x86_64-unknown-linux-musl
```

## 构建命令

```bash
# 构建所有平台（包括musl）
npm run build:native

# 只构建musl target
napi build --platform --release --target x86_64-unknown-linux-musl
```

## 发布前检查

```bash
# 验证所有platform文件都已生成
ls -la *.node

# 应该看到:
# pi-switch-native.darwin-arm64.node
# pi-switch-native.darwin-x64.node
# pi-switch-native.linux-x64-gnu.node
# pi-switch-native.linux-x64-musl.node  ← 新增
# pi-switch-native.win32-x64-msvc.node
```

## 测试musl版本

在Alpine Linux或其他musl系统上测试：
```bash
docker run --rm -it -v $(pwd):/app node:20-alpine sh
cd /app
npm install
node bin/pi-switch.js --version
```
