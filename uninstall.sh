#!/bin/bash

# Ollama Proxy 卸载脚本

set -e

SERVICE_NAME="ollama-proxy"
SERVICE_FILE="/etc/systemd/system/$SERVICE_NAME.service"

# 检查是否以root权限运行
if [ "$EUID" -ne 0 ]; then
  echo "请使用sudo运行此脚本"
  exit 1
fi

echo "卸载 Ollama Proxy 服务..."

# 停止服务（如果正在运行）
if systemctl is-active --quiet $SERVICE_NAME; then
  echo "停止服务..."
  systemctl stop $SERVICE_NAME
fi

# 禁用服务
if systemctl is-enabled --quiet $SERVICE_NAME; then
  echo "禁用服务..."
  systemctl disable $SERVICE_NAME
fi

# 删除服务文件
if [ -f "$SERVICE_FILE" ]; then
  echo "删除服务文件..."
  rm -f $SERVICE_FILE
fi

# 重新加载systemd
systemctl daemon-reload

echo "服务已卸载"
