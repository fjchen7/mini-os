TARGET := riscv64gc-unknown-none-elf
MODE := release
APP_DIR := src/bin
TARGET_DIR := target/$(TARGET)/$(MODE)
APPS := $(wildcard $(APP_DIR)/*.rs)
ELFS := $(patsubst $(APP_DIR)/%.rs, $(TARGET_DIR)/%, $(APPS))

OBJDUMP := rust-objdump --arch-name=riscv64
OBJCOPY := rust-objcopy --binary-architecture=riscv64

elf: $(APPS)
	@cargo build --release

binary: elf
	@$(foreach elf, $(ELFS), $(OBJCOPY) $(elf) --strip-all -O binary $(patsubst $(TARGET_DIR)/%, $(TARGET_DIR)/%.bin, $(elf));)

build: binary

clean:
	@cargo clean

.PHONY: elf binary build clean

# 命令说明
# 假设src/bin有app1.rs和app2.rs两个文件，则
# APPS := src/bin/app1.rs src/bin/app2.rs
# ELFS := target/riscv64gc-unknown-none-elf/release/app1 target/riscv64gc-unknown-none-elf/release/app2
#
# binary命令会对ELF中的每个文件，执行类似的命令：
#     $(OBJCOPY) target/riscv64gc-unknown-none-elf/release/app --strip-all -O binary target/riscv64gc-unknown-none-elf/release/app.bin
# 该命令将裁剪ELF文件中的Header和符号表等信息，生成纯二进制文件，才能让指令加载到正确的内存位置。
