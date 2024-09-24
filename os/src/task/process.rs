use core::cell::RefMut;

use super::{
    action::SignalActions,
    add_task,
    id::{PidHandle, RecycleAllocator},
    manager::insert_into_pid2process,
    pid_alloc,
    task::TaskControlBlock,
    SignalFlags,
};
use crate::{
    fs::{File, Stdin, Stdout},
    mm::{
        kernel_token, translated_refmut, FileMapping, MemorySet, VirtAddr, VirtualAddressAllocator,
    },
    sync::{Mutex, UPSafeCell},
    trap::{trap_handler, TrapContext},
};
use alloc::{
    string::String,
    sync::{Arc, Weak},
    vec,
    vec::Vec,
};
use easy_fs::Inode;

// 进程的控制块。进程的执行状态、资源控制等元数据，都保存在该结构体中。
pub struct ProcessControlBlock {
    pub pid: PidHandle,
    inner: UPSafeCell<ProcessControlBlockInner>,
}

pub struct ProcessControlBlockInner {
    // 是否为僵尸进程
    pub is_zombie: bool,
    // 地址空间
    pub memory_set: MemorySet,
    // 父进程。使用Weak，避免循环引用。
    pub parent: Option<Weak<ProcessControlBlock>>,
    // 子进程
    pub children: Vec<Arc<ProcessControlBlock>>,
    // 进程退出时，返回的退出码保存在这里
    pub exit_code: i32,
    // 文件描述符表
    // 下标就是文件描述符。如果元素为None，则表示该文件描述符未被使用，可以重新被分配。
    pub fd_table: Vec<Option<Arc<dyn File + Send + Sync>>>,

    // 线程列表。下标就是tid。
    pub tasks: Vec<Option<Arc<TaskControlBlock>>>,
    // 线程的资源分配器
    pub task_res_allocator: RecycleAllocator,

    // 进程对每个信号的处理函数
    pub signal_actions: SignalActions,
    // 全局的信号掩码集合。该集合中的信号，将始终被该进程屏蔽。
    pub signal_mask: SignalFlags,
    // 当前进程已收到，但尚未处理的信号集合
    pub signals: SignalFlags,
    // 当前进程正在处理的信号
    pub handling_sig: isize,
    // 执行进程定义的信号处理逻辑时，要保存的上下文。
    // 从信号处理逻辑返回后，要恢复该上下文。
    pub trap_ctx_backup: Option<TrapContext>,
    // 进程是否已经被杀死
    pub killed: bool,
    // 进程是否被挂起（收到SIGSTOP后的状态，并由SIGCONT恢复）
    pub frozen: bool,

    // 该进程所拥有的互斥锁列表
    pub mutex_list: Vec<Option<Arc<dyn Mutex>>>,

    // 堆的底部，即堆的起始地址。数字小（堆从低地址向高地址增长）。
    pub heap_bottom: usize,
    // 堆的顶部，即堆的结束地址。数字大。
    // 这个指针的名字就叫program break。
    pub program_brk: usize,

    // mmap
    pub mmap_va_allocator: VirtualAddressAllocator,
    pub file_mappings: Vec<FileMapping>,
}

impl ProcessControlBlockInner {
    pub fn get_user_token(&self) -> usize {
        self.memory_set.token()
    }

    pub fn is_zombie(&self) -> bool {
        self.is_zombie
    }

    pub fn alloc_fd(&mut self) -> usize {
        if let Some(fd) = (0..self.fd_table.len()).find(|fd| self.fd_table[*fd].is_none()) {
            fd
        } else {
            self.fd_table.push(None);
            self.fd_table.len() - 1
        }
    }

    pub fn alloc_tid(&mut self) -> usize {
        self.task_res_allocator.alloc()
    }

    pub fn dealloc_tid(&mut self, tid: usize) {
        self.task_res_allocator.dealloc(tid)
    }

    pub fn thread_count(&self) -> usize {
        self.tasks.len()
    }

    pub fn get_task(&self, tid: usize) -> Arc<TaskControlBlock> {
        self.tasks[tid].as_ref().unwrap().clone()
    }

    pub fn find_file_mapping_mut(&mut self, file: &Arc<Inode>) -> Option<&mut FileMapping> {
        self.file_mappings
            .iter_mut()
            .find(|m| Arc::ptr_eq(&m.file, file))
    }
}

impl ProcessControlBlock {
    pub fn inner_exclusive_access(&self) -> RefMut<'_, ProcessControlBlockInner> {
        self.inner.exclusive_access()
    }

    pub fn getpid(&self) -> usize {
        self.pid.0
    }

    fn init_fd_table() -> Vec<Option<Arc<dyn File + Send + Sync>>> {
        vec![
            Some(Arc::new(Stdin)),  // 0 -> stdin
            Some(Arc::new(Stdout)), // 1 -> stdout
            Some(Arc::new(Stdout)), // 2 -> stderr
        ]
    }

    // 解析ELF格式的二进制数据，创建一个新的进程
    pub fn new(elf_data: &[u8]) -> Arc<Self> {
        // 解析ELF，得到地址空间、用户栈顶、入口地址
        let (memory_set, ustack_base, entry_point) = MemorySet::from_elf(elf_data);
        // 分配新的PID
        let pid_handle = pid_alloc();
        let process = Self {
            pid: pid_handle,
            inner: unsafe {
                UPSafeCell::new(ProcessControlBlockInner {
                    is_zombie: false,
                    memory_set,
                    parent: None,
                    children: vec![],
                    exit_code: 0,
                    fd_table: Self::init_fd_table(),
                    tasks: vec![],
                    task_res_allocator: RecycleAllocator::new(),
                    signal_actions: SignalActions::default(),
                    signal_mask: SignalFlags::empty(),
                    signals: SignalFlags::empty(),
                    handling_sig: -1,
                    trap_ctx_backup: None,
                    killed: false,
                    frozen: false,
                    mutex_list: vec![],
                    heap_bottom: ustack_base,
                    program_brk: ustack_base,
                    mmap_va_allocator: VirtualAddressAllocator::default(),
                    file_mappings: vec![],
                })
            },
        };
        let process = Arc::new(process);

        // 创建主线程
        let task = Arc::new(TaskControlBlock::new(
            Arc::clone(&process),
            ustack_base,
            true,
        ));
        // 初始化主线程的TrapContext
        let task_inner = task.inner_exclusive_access();
        let trap_cx = task_inner.get_trap_cx();
        let ustack_top = task_inner.res.as_ref().unwrap().ustack_top();
        let kstack_top = task.kstack.get_top();
        drop(task_inner);
        *trap_cx = TrapContext::app_init_context(
            entry_point,
            ustack_top,
            kernel_token(),
            kstack_top,
            trap_handler as usize,
        );
        // 将该主线程加入进程中
        let mut process_inner = process.inner_exclusive_access();
        process_inner.tasks.push(Some(task.clone()));
        drop(process_inner);
        insert_into_pid2process(process.getpid(), process.clone());
        // 将该主线程加入任务队列
        add_task(task);
        process
    }

    // 从父进程复制出一个子进程
    pub fn fork(self: &Arc<Self>) -> Arc<Self> {
        let mut parent = self.inner_exclusive_access();
        // 目前只支持单线程
        assert_eq!(parent.thread_count(), 1);
        // 为子进程分配新的地址空间
        let memory_set = MemorySet::from_existed_user(&parent.memory_set);
        // 为子进程分配新的PID
        let pid = pid_alloc();
        // 复制父进程的fd
        let fd_table = parent.fd_table.clone();
        let child = Arc::new(ProcessControlBlock {
            pid,
            inner: unsafe {
                let value = ProcessControlBlockInner {
                    is_zombie: false,
                    memory_set,
                    parent: Some(Arc::downgrade(self)),
                    children: Vec::new(),
                    exit_code: 0,
                    fd_table,
                    tasks: vec![],
                    task_res_allocator: RecycleAllocator::new(),
                    signal_actions: parent.signal_actions.clone(),
                    signal_mask: parent.signal_mask,
                    signals: SignalFlags::empty(),
                    handling_sig: -1,
                    trap_ctx_backup: None,
                    killed: false,
                    frozen: false,
                    mutex_list: vec![],
                    heap_bottom: parent.heap_bottom,
                    program_brk: parent.program_brk,
                    mmap_va_allocator: VirtualAddressAllocator::default(),
                    file_mappings: vec![],
                };
                UPSafeCell::new(value)
            },
        });
        // 更新父进程的children
        parent.children.push(child.clone());
        // 创建子进程的主线程
        let ustack_base = parent
            .get_task(0)
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .ustack_base();
        // 这里传入的alloc_user_res为false，
        // 不再分配新的用户栈和TrapContext内存，因为复制memroy_set时已经复制了这些内容
        // 但仍然会会分配新的kstack
        let task = Arc::new(TaskControlBlock::new(child.clone(), ustack_base, false));
        // TODO: 优化这里的代码
        // 将该主线程加入子进程中
        let mut child_inner = child.inner_exclusive_access();
        child_inner.tasks.push(Some(task.clone()));
        drop(child_inner);
        // 更新主线程的TrapContext
        // 只需更新kernel_sp，因为其他字段都是用户地址空间里的地址，已经都复制过了。
        let task_inner = task.inner_exclusive_access();
        let trap_cx = task_inner.get_trap_cx();
        trap_cx.kernel_sp = task.kstack.get_top();
        drop(task_inner);

        insert_into_pid2process(child.getpid(), child.clone());
        // 将子进程的主线程加入任务队列
        add_task(task);
        child
    }

    // 申请新的地址空间，加载ELF文件。这将替换原来的地址空间，同时初始化TrapContext。
    // 在操作系统上执行程序，都会fork父进程，然后再调用这个方法。
    pub fn exec(&self, elf_data: &[u8], args: Vec<String>) {
        // 目前只支持单线程
        assert_eq!(self.inner_exclusive_access().thread_count(), 1);
        // 申请新的地址空间，加载ELF文件
        let (memory_set, ustack_base, entry_point) = MemorySet::from_elf(elf_data);
        let new_token = memory_set.token();

        let mut inner = self.inner_exclusive_access();
        inner.memory_set = memory_set;
        inner.heap_bottom = ustack_base; // TODO: fix new heap bottom
        inner.program_brk = ustack_base;
        inner.mmap_va_allocator = VirtualAddressAllocator::default();
        inner.file_mappings = vec![];
        drop(inner);

        // 替换主线程
        let task = self.inner_exclusive_access().get_task(0);
        let mut task_inner = task.inner_exclusive_access();
        // TODO: 优化这里的代码
        task_inner.res.as_mut().unwrap().ustack_base = ustack_base;
        task_inner.res.as_mut().unwrap().alloc_user_res();
        task_inner.trap_cx_ppn = task_inner.res.as_mut().unwrap().trap_cx_ppn();

        // 将exec的参数压入用户栈
        let mut user_sp = task_inner.res.as_mut().unwrap().ustack_top();
        let size_of_ptr = core::mem::size_of::<usize>();
        user_sp -= (args.len() + 1) * size_of_ptr;
        let argv_base = user_sp;
        let mut argv: Vec<_> = (0..=args.len())
            .map(|arg| translated_refmut(new_token, (argv_base + arg * size_of_ptr) as *mut usize))
            .collect();
        // 多出来的一个指针，指向NULL，表示数组结束
        *argv[args.len()] = 0;
        // 再压入参数的字符串的值
        for i in 0..args.len() {
            user_sp -= args[i].len() + 1;
            *argv[i] = user_sp;
            let mut p = user_sp;
            // 从栈的低位往高位存放字符串
            for c in args[i].as_bytes() {
                *translated_refmut(new_token, p as *mut u8) = *c;
                p += 1;
            }
            // 字符串要以\0结尾。该字节位于栈的高位。
            *translated_refmut(new_token, p as *mut u8) = 0;
        }
        // 对齐到指针大小
        user_sp -= user_sp % size_of_ptr;

        // 替换TrapContext
        let mut trap_cx = TrapContext::app_init_context(
            entry_point,
            user_sp,
            kernel_token(),
            task.kstack.get_top(),
            trap_handler as usize,
        );
        // 进入方法时，寄存器a0(x[10])和a1(x[11])会分别作为方法的第1个和第2个参数传入
        // 这里对应的，就是main(int argc, char *argv[])中的两个参数
        // （在我们的用户程序里，入口函数是_start，它包了一层main）
        //
        // 实际上这里x[10]的赋值没有意义，因为在trap_handler里，执行系统调用后，会把其返回值（sys_exec的返回值就是argc）赋给x[10]
        trap_cx.x[10] = args.len(); // argc
        trap_cx.x[11] = argv_base; // argv
        *task_inner.get_trap_cx() = trap_cx;
    }

    // 增加或减少堆的大小
    // 改变成功时，返回原来堆的结束位置（最高位）
    pub fn change_program_brk(&self, size: i32) -> Option<usize> {
        let mut inner = self.inner_exclusive_access();
        let old_break = inner.program_brk;
        let new_brk = inner.program_brk as isize + size as isize;
        if new_brk < inner.heap_bottom as isize {
            return None;
        }
        let heap_bottom = VirtAddr(inner.heap_bottom);
        let new_end = VirtAddr(new_brk as usize);
        let result = if size < 0 {
            inner.memory_set.shrink_to(heap_bottom, new_end)
        } else {
            inner.memory_set.append_to(heap_bottom, new_end)
        };
        if result {
            inner.program_brk = new_brk as usize;
            Some(old_break)
        } else {
            None
        }
    }
}
