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

## 运行结果

不同程序的运行结果

```shell
$ qemu-riscv64 00hello_world
Hello, world!
```

```shell
$ qemu-riscv64 01store_fault
Into Test store_fault, we will insert an invalid store operation...
Kernel should kill this application!
Segmentation fault
```

```shell
$ qemu-riscv64 02power
3^10000=5079(MOD 10007)
3^20000=8202(MOD 10007)
3^30000=8824(MOD 10007)
3^40000=5750(MOD 10007)
3^50000=3824(MOD 10007)
3^60000=8516(MOD 10007)
3^70000=2510(MOD 10007)
3^80000=9379(MOD 10007)
3^90000=2621(MOD 10007)
3^100000=2749(MOD 10007)
Test power OK!
```

```shell
$ qemu-riscv64 03priv_inst
Try to execute privileged instruction in U Mode
Kernel should kill this application!
Illegal instruction
```

```shell
$ qemu-riscv64 04priv_csr
Try to access privileged CSR in U Mode
Kernel should kill this application!
Illegal instruction
```

前三个程序都可以执行成功。03尝试直接执行内核级的特权指令，04尝试访问内核级的寄存器CSR，因此运行都被阻止了。
