# FuckVim MVP - 构建脚本
#
# 使用方法:
#   make build-plugin  - 编译 Rust WASM 插件
#   make run           - 构建插件并运行编辑器
#   make clean         - 清理构建产物

.PHONY: all build-plugin run clean deps

# 默认目标
all: run

# 安装 Go 依赖
deps:
	@echo "📦 安装 Go 依赖..."
	go mod tidy

# 编译 Rust WASM 插件
build-plugin:
	@echo "🦀 编译 Rust WASM 插件..."
	cd plugin && cargo build --release --target wasm32-unknown-unknown
	@echo "📋 复制 WASM 文件到项目根目录..."
	cp plugin/target/wasm32-unknown-unknown/release/plugin.wasm ./plugin.wasm
	@echo "✅ 插件编译完成: plugin.wasm"

# 运行编辑器 (先构建插件)
run: build-plugin deps
	@echo "🚀 启动 FuckVim..."
	go run main.go

# 仅运行 (不重新编译插件，用于快速测试)
run-only:
	@echo "🚀 启动 FuckVim (使用现有插件)..."
	./fuckvim

# 构建可执行文件
build: build-plugin deps
	@echo "🔨 构建 Go 可执行文件..."
	go build -o fuckvim.exe main.go
	@echo "✅ 构建完成: fuckvim.exe"

# 清理构建产物
clean:
	@echo "🧹 清理构建产物..."
	rm -f plugin.wasm
	rm -f fuckvim.exe
	rm -rf plugin/target
	@echo "✅ 清理完成"

# 帮助信息
help:
	@echo "FuckVim MVP - 可用命令:"
	@echo "  make build-plugin  - 编译 Rust WASM 插件"
	@echo "  make run           - 构建插件并运行编辑器"
	@echo "  make run-only      - 仅运行 (使用现有插件)"
	@echo "  make build         - 构建可执行文件"
	@echo "  make clean         - 清理构建产物"
	@echo ""
	@echo "前置要求:"
	@echo "  - Go 1.21+"
	@echo "  - Rust + Cargo"
	@echo "  - rustup target add wasm32-unknown-unknown"
