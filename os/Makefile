# 编译参数
TARGET := riscv64gc-unknown-none-elf
MODE := release
KERNEL_ELF := target/$(TARGET)/$(MODE)/os
KERNEL_BIN := $(KERNEL_ELF).bin
# 打包了应用ELF的文件系统镜像
FS_IMG := ../user/target/$(TARGET)/$(MODE)/fs.img
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

build: env $(KERNEL_BIN) fs-img

env:
	(rustup target list | grep "riscv64gc-unknown-none-elf (installed)") || rustup target add $(TARGET)
	cargo install cargo-binutils
	rustup component add rust-src
	rustup component add llvm-tools-preview

$(KERNEL_BIN): kernel
	@$(OBJCOPY) $(KERNEL_ELF) --strip-all -O binary $@

fs-img: $(APPS)
	@cd ../user && make build TEST=$(TEST)
	@rm -f $(FS_IMG)
	@cd ../easy-fs-fuse && cargo run --release -- -s ../user/src/bin/ -t ../user/target/riscv64gc-unknown-none-elf/release/

$(APPS):

kernel:
	@echo Platform: $(BOARD)
	@cargo build $(MODE_ARG)

# 参数说明
# -drive：添加虚拟硬盘，命名为x0，内容为FS_IMG所指文件
# -device virtio-blk-device：将硬盘x0作为VirtIO总线上的一个块设备，接入到虚拟机中。
#                            virtio-mmio-bus.0表示VirtIO总线通过MMIO进行控制，且该块设备的编号为0。
QEMU_ARGS := -machine virt \
			-nographic \
			-bios $(BOOTLOADER) \
			-device loader,file=$(KERNEL_BIN),addr=$(KERNEL_ENTRY_PA) \
			-drive file=$(FS_IMG),if=none,format=raw,id=x0 \
			-device virtio-blk-device,drive=x0,bus=virtio-mmio-bus.0

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
