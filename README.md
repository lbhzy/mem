# mem

通过 `/dev/mem` 访问物理内存的命令行工具，适用于嵌入式 Linux 下 MMIO 寄存器调试。

## 编译

```bash
# 本机编译
cargo build --release

# 交叉编译 aarch64（需要 cargo-zigbuild）
cargo aarch64
```

## 命令

所有命令支持简写。地址为十六进制，`0x` 前缀可选。

| 命令 | 简写 | 说明 |
|------|------|------|
| `read` | `r` | 读取内存并以 hex dump 显示 |
| `write` | `w` | 向地址写入一个或多个 u32 值 |
| `bit` | `b` | 读取或设置指定 bit |
| `dump` | `d` | 将内存区域导出到文件 |
| `load` | `l` | 将文件内容写入内存 |

## 用法示例

```bash
# 读取 0x1000 处 128 字节（默认）
mem r 0x1000

# 读取 0x1000 处 16 字节
mem r 0x1000 16

# 向 0x1000 写入一个 u32
mem w 0x1000 0xDEADBEEF

# 连续写入多个 u32
mem w 0x1000 0x01 0x02 0x03

# 读取 0x1000 的 bit 5
mem b 0x1000 5

# 设置 0x1000 的 bit 5 为 1
mem b 0x1000 5 1

# 清除 0x1000 的 bit 5
mem b 0x1000 5 0

# 导出 4096 字节到文件
mem d 0x1000 0x1000 out.bin

# 从文件加载到内存
mem l 0x1000 firmware.bin
```

## 输出说明

- **hex dump**：每行 16 字节，按 4 字节小端分组显示 u32 值
- **binary 视图**：首个 u32 的逐 bit 展示，1 为黄色高亮，0 为灰色
- 地址必须 4 字节对齐（32 位访问）

## 依赖

- Linux（需要 `/dev/mem`）
- 通常需要 root 权限
