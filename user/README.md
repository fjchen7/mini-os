本子项目实现用户程序。它们能被编译成ELF文件，然后加载进内核运行。

## 运行

Qemu有两种模拟模式：

- 系统级模拟（System mode）：模拟RISC-V计算机裸机。命令为`qemu-system-risc64`。
- 用户态模拟（User mode）：，模拟预装了Linux的RISC-V计算机。但仅支持载入和执行单个ELF可执行文件。使用`qemu-riscv64`。

因此使用`qemu-riscv64`命令，就可以测试我们写的应用程序。

运行过程如下：

1. 进入环境：在根目录执行`make docker`，并切换到`user`目录
2. 编译项目：`cargo build --release`
3. 进入编译目录：`cd target/riscv64gc-unknown-none-elf/release`
4. 在Qemu环境下执行程序：`qemu-riscv64 ./00hello_world`
