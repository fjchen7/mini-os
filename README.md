这是参考清华大学的[rCore-Tutorial-Book](https://rcore-os.cn/rCore-Tutorial-Book-v3/index.html)课程实现的内核。有详尽的注释和README，方便理解和回顾。可将本项目当作学习笔记使用。

## 运行

在根目录下，执行

- `make docker`：进入Docker环境。该环境安装了qemu、rust、cargo等工具。
- `make run`：进入Docker环境，并启动Qemu模拟器，运行内核。

在Docker环境中，切换到`os`目录下，可执行命令：

- `make run`：运行内核。
- `make dbgserver`：运行内核，并启动GDB调试服务器（监听端口`1234`）。
- `make dbgclient`：连接GDB调试服务器，连接到内核进程。

## 实现的功能

本项目会编译到目标平台`riscv64gc-unknown-none-elf`。`unknown`表示该目标平台不使用操作系统，`elf`表示编译出来的文件是ELF格式。

### 1. 基本的执行环境

使用Qemu模拟RISC-V计算机。计算机启动时，控制权的变化为：

1. 硬件：由硬件固化的汇编程序负责，执行初始化后，将控制权交给`0x80000000`地址上的程序；
2. bootloader：引导程序。该程序需要被加载到`0x80000000`，做相关的初始化工作后，并将控制权交给`0x80200000`地址上的程序。我们用的RustSBI就属于这个角色。
3. 内核：最终接管计算机的控制权。内核必须被加载进`0x80200000`地址处。

还需要定义内核的内存布局：

- 入口函数是`_start`，由汇编代码（[`entry.asm`](./os/src/entry.asm)）编写。它会进入Rust方法`rust_main`，打印`Hello, world!`。
- 链接脚本（[`linker.ld`](./os/src/linker.ld)）定义内存布局。需要注意的是，启动时要将`.bss`段清零。
- 要让`0x80200000`处的第一条指令，是入口函数`_start`，才能保证控制权的交接。
  - 直接使用`cargo build --release`编译出来的ELF文件是不行的，因为它还携带头信息和符号表。
  - 需要用`rust-objcopy`工具裁剪（strip）它们，才能使第一条指令在内核内存段的初始位置。

此外，内核处于Supervisor特权级别，而RustSBI处于更高的Machine特权级别。内核需要借助RustSBI提供的SBI接口，才能操作硬件。这里使用了库`sbi-rt`提供的SBI接口封装。
