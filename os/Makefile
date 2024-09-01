# 编译参数
TARGET := riscv64gc-unknown-none-elf
MODE := release
KERNEL_ELF := target/$(TARGET)/$(MODE)/os
KERNEL_BIN := $(KERNEL_ELF).bin
# 编译的模式
ifeq ($(MODE), release)
	MODE_ARG := --release
endif

# qemu环境参数
BOARD := qemu
SBI ?= rustsbi
BOOTLOADER := ../bootloader/$(SBI)-$(BOARD).bin
# 内核的入口
KERNEL_ENTRY_PA := 0x80200000

# 二进制工具
OBJDUMP := rust-objdump --arch-name=riscv64
OBJCOPY := rust-objcopy --binary-architecture=riscv64

build: env $(KERNEL_BIN)

env:
	(rustup target list | grep "riscv64gc-unknown-none-elf (installed)") || rustup target add $(TARGET)
	cargo install cargo-binutils
	rustup component add rust-src
	rustup component add llvm-tools-preview

$(KERNEL_BIN): kernel
	@$(OBJCOPY) $(KERNEL_ELF) --strip-all -O binary $@

kernel:
	@echo Platform: $(BOARD)
	@cargo build $(MODE_ARG)

QEMU_ARGS := -machine virt \
			-nographic \
			-bios $(BOOTLOADER) \
			-device loader,file=$(KERNEL_BIN),addr=$(KERNEL_ENTRY_PA)

QEMU_NAME := qemu-system-riscv64
# 检查Qemu版本
qemu-version-check:
	@sh scripts/qemu-ver-check.sh $(QEMU_NAME)

# 运行内核
run: qemu-version-check build
	qemu-system-riscv64 $(QEMU_ARGS)

# 运行内核，并开启gdb调试服务器
# -s 以TCP监听本地1234端口，等待gdb连接
# -S 收到gdb连接后才开始运行
dbgserver: qemu-version-check build
	qemu-system-riscv64 $(QEMU_ARGS) -s -S

# 运行gdb调试客户端，连接到内核进程
dbgclient: qemu-version-check build
	riscv64-unknown-elf-gdb \
	-ex 'file $(KERNEL_ELF)' \
	-ex 'set arch riscv:rv64' \
	-ex 'target remote localhost:1234'

.PHONY: build env kernel clean run-inner gdbserver gdbclient qemu-version-check
