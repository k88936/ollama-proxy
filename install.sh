#!/bin/bash

# Ollama Proxy 安装脚本

set -e

SERVICE_NAME="ollama-proxy"
SERVICE_FILE="/etc/systemd/system/$SERVICE_NAME.service"
BINARY_PATH="$(pwd)/target/release/ollama-proxy"

# 检查是否以root权限运行
if [ "$EUID" -ne 0 ]; then
  echo "请使用sudo运行此脚本"
  exit 1
fi

# 检查二进制文件是否存在
if [ ! -f "$BINARY_PATH" ]; then
  echo "错误: 找不到二进制文件 $BINARY_PATH"
  echo "请先构建项目: cargo build --release"
  exit 1
fi

# 获取当前用户和组
if [ -z "$SUDO_USER" ]; then
  CURRENT_USER=$USER
else
  CURRENT_USER=$SUDO_USER
fi

CURRENT_GROUP=$(id -gn "$CURRENT_USER")

echo "安装 Ollama Proxy 服务..."

# 创建服务文件
cat > $SERVICE_FILE << EOF
[Unit]
Description=Ollama Proxy Service
After=network.target

[Service]
Type=simple
User=$CURRENT_USER
Group=$CURRENT_GROUP
WorkingDirectory=$(pwd)
ExecStart=$BINARY_PATH
Restart=always
Environment=RUST_LOG=info

[Install]
WantedBy=multi-user.target
EOF

# 设置权限
chmod 644 $SERVICE_FILE

# 重新加载systemd
systemctl daemon-reload

# 启用服务
systemctl enable $SERVICE_NAME

echo "服务已安装并设置为开机自启"
echo "服务文件位置: $SERVICE_FILE"
echo ""
echo "使用以下命令管理服务:"
echo "  启动服务: sudo systemctl start $SERVICE_NAME"
echo "  停止服务: sudo systemctl stop $SERVICE_NAME"
echo "  查看状态: sudo systemctl status $SERVICE_NAME"
echo "  查看日志: sudo journalctl -u $SERVICE_NAME -f"
