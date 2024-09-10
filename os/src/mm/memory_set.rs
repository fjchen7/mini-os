//! 虚拟地址空间的数据结构表示。每个程序都有自己的地址空间。

use super::{
    address::{PhysAddr, PhysPageNum, VPNRange, VirtAddr, VirtPageNum},
    frame_allocator::{frame_alloc, FrameTracker},
    page_table::{PTEFlags, PageTable, PageTableEntry},
};
use crate::{
    config::{MEMORY_END, PAGE_SIZE, TRAMPOLINE, TRAP_CONTEXT, USER_STACK_SIZE},
    mm::address::StepByOne,
    sync::UPSafeCell,
};
use alloc::{collections::btree_map::BTreeMap, sync::Arc, vec, vec::Vec};
use core::{arch::asm, cmp::min};
use lazy_static::*;
use riscv::register::satp;

lazy_static! {
    // 用于管理内核地址空间的MemorySet实例
    pub static ref KERNEL_SPACE: Arc<UPSafeCell<MemorySet>> =
        Arc::new(unsafe { UPSafeCell::new(MemorySet::new_kernel()) });
}

// 表示内核或应用程序的地址空间。
// 它包含的物理页有：
// - 页表的物理页
// - 逻辑段的物理页
pub struct MemorySet {
    page_table: PageTable,
    // 逻辑段，如.text、.rodata、.data、.bss等
    // 不同逻辑段是关联的，但不一定连续
    areas: Vec<MapArea>,
}

// 表示逻辑段，即一段连续地址的虚拟地址空间。
// 这段地址空间，使用相同的映射方式（MapType）和映射权限（MapPermission）。
pub struct MapArea {
    // 该地址空间的虚拟页号的范围
    vpn_range: VPNRange,
    // 该地址空间的虚拟页号到物理页号的映射
    // 物理页的生命周期由该结构体管理，FrameTracker被回收后，该物理页也被回收
    data_frames: BTreeMap<VirtPageNum, FrameTracker>,
    map_type: MapType,
    map_perm: MapPermission,
}

#[derive(Copy, Clone, PartialEq, Debug)]
// 地址空间的映射方式
pub enum MapType {
    // 恒等映射，即虚拟页号等于物理页号。由于一个段的虚拟页号是连续的，因此对应的物理页号也是连续的
    // 内核的地址空间，使用该映射方式
    Identical,
    // 使用物理页分配器来分配，相对随机
    // 用户程序的地址空间，使用该映射方式
    Framed,
}

bitflags! {
    // 映射权限。这是页表项标志位PTEFlags的子集。
    pub struct MapPermission: u8 {
        const R = 1 << 1;  // 可读
        const W = 1 << 2;  // 可写
        const X = 1 << 3;  // 可执行
        const U = 1 << 4;  // 用户态（CPU处于U特权级时）可访问
    }
}

impl MemorySet {
    // 创建空的地址空间
    // 这将分配一个物理页，作为根页表
    pub fn new_bare() -> Self {
        Self {
            page_table: PageTable::new(),
            areas: Vec::new(),
        }
    }

    // 插入一个逻辑段到该地址空间中
    // 如果它是以Framed方式映射到物理页的，还可以提供数据，用来初始化这个逻辑段
    fn push(&mut self, mut map_area: MapArea, data: Option<&[u8]>) {
        map_area.map(&mut self.page_table);
        if let Some(data) = data {
            map_area.copy_data(&self.page_table, data);
        }
        self.areas.push(map_area);
    }

    // 插入一个范围为 [start_va, end_va) 的逻辑段，映射方式为Framed
    // 这里假设，该逻辑段不与已有的逻辑段重叠
    pub fn insert_framed_area(
        &mut self,
        start_va: VirtAddr,
        end_va: VirtAddr,
        permission: MapPermission,
    ) {
        self.push(
            MapArea::new(start_va, end_va, MapType::Framed, permission),
            None,
        );
    }

    // 映射跳板 (Trampoline）。跳板是存放切换地址空间的汇编代码的物理内存区域。
    // 不管是内核或程序，跳板的映射都是一致的。也就是，跳板的虚拟页都相同，且会映射到相同的物理页。
    fn map_trampoline(&mut self) {
        extern "C" {
            fn strampoline();
        }
        self.page_table.map(
            VirtAddr::from(TRAMPOLINE).into(),
            PhysAddr::from(strampoline as usize).into(),
            PTEFlags::R | PTEFlags::X,
        );
        // 但跳表的物理页，不会被逻辑段管理。它是特殊的物理页，不会被回收。映射关系是人为固定的。
    }

    // 新建内核的地址空间。这里将映射内核的地址空间中的低256GB内存。
    pub fn new_kernel() -> Self {
        extern "C" {
            fn stext();
            fn etext();
            fn srodata();
            fn erodata();
            fn sdata();
            fn edata();
            fn sbss_with_stack();
            fn ebss();
            fn ekernel();
        }
        let mut memory_set = Self::new_bare();
        // 映射跳板
        memory_set.map_trampoline();
        println_kernel!("Mapping Kernel Memory...");
        let sections = vec![
            (
                ".text",
                stext as usize,
                etext as usize,
                MapType::Identical,
                MapPermission::R | MapPermission::X, // .text区不可修改
            ),
            (
                ".rodata",
                srodata as usize,
                erodata as usize,
                MapType::Identical,
                MapPermission::R, // .rodata区不可修改，不可执行
            ),
            (
                ".data",
                sdata as usize,
                edata as usize,
                MapType::Identical,
                MapPermission::R | MapPermission::W, // .data区不可执行
            ),
            (
                ".bss",
                sbss_with_stack as usize,
                ebss as usize,
                MapType::Identical,
                MapPermission::R | MapPermission::W, // .bss区不可执行
            ),
            (
                "physical memory",
                ekernel as usize,
                MEMORY_END,
                MapType::Identical,
                MapPermission::R | MapPermission::W, // 物理内存区域不可执行
            ),
        ];

        for (name, start, end, map_type, map_perm) in sections {
            println_kernel!("{:<15} [{:#010x}, {:#010x})", name, start, end);
            let map_area = MapArea::new(start.into(), end.into(), map_type, map_perm);
            memory_set.push(map_area, None);
        }
        memory_set
    }

    // 解析应用程序的ELF格式的二进制数据，找到对应的逻辑段地址，新建该程序的地址空间
    // 返回内容：(程序的地址空间, 用户栈顶指针, 程序入口地址)
    //
    // 地址空间的内容：
    // 低256GB（从低位到高位）
    // - 0x10000：起始位置
    // - 逻辑段：.text、.rodata、.data、.bss
    // - 保护页（guard page）：大小为一个页
    // - 用户栈：大小为USER_STACK_SIZE
    // 高256GB（从高位到低位）
    // - 跳板（Trampoline）：存放切换地址空间的汇编代码，大小为一个页
    // - Trap Context
    pub fn from_elf(elf_data: &[u8]) -> (Self, usize, usize) {
        let mut memory_set = Self::new_bare();
        // 映射跳板
        memory_set.map_trampoline();
        // 使用库xmas_elf来解析ELF数据
        // 可以用rust-readobj -all target/debug/os命令，来查看ELF文件的结构
        let elf = xmas_elf::ElfFile::new(elf_data).unwrap();
        let elf_header = elf.header;
        // 检查魔数
        let magic = elf_header.pt1.magic;
        assert_eq!(magic, [0x7f, 0x45, 0x4c, 0x46], "invalid elf!");
        // 遍历头（program header，ph），将各个区域加到对应的逻辑段中
        let ph_count = elf_header.pt2.ph_count();
        let mut max_end_vpn = VirtPageNum(0);
        for i in 0..ph_count {
            let ph = elf.program_header(i).unwrap();
            // 类型为Load，表示该区域需要被加载进内核
            if ph.get_type().unwrap() == xmas_elf::program::Type::Load {
                // 得到该区域的起始和结束地址
                let start_va: VirtAddr = (ph.virtual_addr() as usize).into();
                let end_va: VirtAddr = ((ph.virtual_addr() + ph.mem_size()) as usize).into();
                // 读取访问权限
                let mut map_perm = MapPermission::U;
                let ph_flags = ph.flags();
                if ph_flags.is_read() {
                    map_perm |= MapPermission::R;
                }
                if ph_flags.is_write() {
                    map_perm |= MapPermission::W;
                }
                if ph_flags.is_execute() {
                    map_perm |= MapPermission::X;
                }
                let map_area = MapArea::new(start_va, end_va, MapType::Framed, map_perm);
                // 记录最大的结束地址
                // 这里的header是按地址排序的，因此不需要再用max方法比较取值
                max_end_vpn = map_area.vpn_range.get_end();
                // 当前program header数据被存放的位置，可通过ph.offset()和ph.file_size()来找到
                memory_set.push(
                    map_area,
                    Some(&elf.input[ph.offset() as usize..(ph.offset() + ph.file_size()) as usize]),
                );
            }
        }
        // 初始化保护页和用户栈
        let max_end_va: VirtAddr = max_end_vpn.into();
        let mut user_stack_bottom: usize = max_end_va.into();
        // 映射保护页（guard page），大小为一个页
        user_stack_bottom += PAGE_SIZE;
        // 映射用户栈
        let user_stack_top = user_stack_bottom + USER_STACK_SIZE;
        memory_set.push(
            MapArea::new(
                user_stack_bottom.into(),
                user_stack_top.into(),
                MapType::Framed,
                MapPermission::R | MapPermission::W | MapPermission::U,
            ),
            None,
        );
        // 映射堆。通过系统调用sbrk可以申请/释放内存，改变堆的大小。
        memory_set.push(
            MapArea::new(
                user_stack_top.into(),
                user_stack_top.into(),
                MapType::Framed,
                MapPermission::R | MapPermission::W | MapPermission::U,
            ),
            None,
        );
        // 映射存放TrapContext的区域，这在地址空间的次高页（在地址空间的高256位中）。
        // 地址空间的最高页是跳板（Trampoline），但它的映射关系是独立的，不归该地址空间结构体管理。
        memory_set.push(
            MapArea::new(
                TRAP_CONTEXT.into(),
                TRAMPOLINE.into(),
                MapType::Framed,
                MapPermission::R | MapPermission::W,
            ),
            None,
        );
        (
            memory_set,
            user_stack_top,
            elf.header.pt2.entry_point() as usize,
        )
    }

    pub fn translate(&self, vpn: VirtPageNum) -> Option<PageTableEntry> {
        self.page_table.translate(vpn)
    }

    #[allow(unused)]
    pub fn shrink_to(&mut self, start: VirtAddr, new_end: VirtAddr) -> bool {
        if let Some(area) = self
            .areas
            .iter_mut()
            .find(|area| area.vpn_range.get_start() == start.floor())
        {
            area.shrink_to(&mut self.page_table, new_end.ceil());
            true
        } else {
            false
        }
    }

    #[allow(unused)]
    pub fn append_to(&mut self, start: VirtAddr, new_end: VirtAddr) -> bool {
        if let Some(area) = self
            .areas
            .iter_mut()
            .find(|area| area.vpn_range.get_start() == start.floor())
        {
            area.append_to(&mut self.page_table, new_end.ceil());
            true
        } else {
            false
        }
    }

    // 设置CSR寄存器satp的值，激活该地址空间（只有内核空间才调用）
    pub fn activate(&self) {
        let satp = self.page_table.token();
        unsafe {
            // 写satp的指令不是跳转指令，PC只会简单地自增取指的地址。
            // 该指令前后，地址空间已经不同了，MMU会以不同的方式翻译地址。
            // 不过这对内核空间用该方法来开启分页，没有影响：
            // - 该指令前，分页机制尚未开启，直接用物理地址访问指令
            // - 该指令后，开启分页机制。但当前属于内核空间，映射为恒等映射，访问的虚拟内存等同于物理内存
            // 因此前后是连续的
            satp::write(satp);
            // sfence.vma指令是内存屏障，可清空快表（TLB, Translation Lookaside Buffer）
            // 由于地址空间已经变化，因此要清除这些过期的映射关系的缓存，保证MMU不再看到。
            asm!("sfence.vma");
        }
    }

    pub fn token(&self) -> usize {
        self.page_table.token()
    }
}

impl MapArea {
    // 新建逻辑段
    pub fn new(
        start_va: VirtAddr,
        end_va: VirtAddr,
        map_type: MapType,
        map_perm: MapPermission,
    ) -> Self {
        let start_vpn: VirtPageNum = start_va.floor(); // 向下取整
        let end_vpn: VirtPageNum = end_va.ceil(); // 向上取整
        Self {
            vpn_range: VPNRange::new(start_vpn, end_vpn),
            data_frames: BTreeMap::new(),
            map_type,
            map_perm,
        }
    }

    // 将给定虚拟页号映射到物理页号，并将这个映射关系，更新到页表中的对应页表项
    pub fn map_one(&mut self, page_table: &mut PageTable, vpn: VirtPageNum) {
        let ppn: PhysPageNum;
        // 找到虚拟页号对应的物理页号。有两种方式
        // - Identical：虚拟页号等于物理页号
        // - Framed：让物理页帧分配器，分配一个物理页号
        match self.map_type {
            MapType::Identical => {
                ppn = PhysPageNum(vpn.0);
            }
            MapType::Framed => {
                let frame = frame_alloc().unwrap();
                ppn = frame.ppn;
                // 记录这个映射关系。该物理页号现在将由这个逻辑段管理。
                self.data_frames.insert(vpn, frame);
            }
        }
        // 更新页表
        let pte_flags = PTEFlags::from_bits(self.map_perm.bits).unwrap();
        page_table.map(vpn, ppn, pte_flags);
    }

    #[allow(unused)]
    // 取消给定虚拟页号到物理页号的映射，并更新到页表上
    pub fn unmap_one(&mut self, page_table: &mut PageTable, vpn: VirtPageNum) {
        match self.map_type {
            MapType::Identical => {}
            MapType::Framed => {
                // 移除后，该物理页号将被回收（回到物理页帧分配器的可用内存上）
                self.data_frames.remove(&vpn);
            }
        }
        page_table.unmap(vpn);
    }

    // 将整个逻辑段的虚拟页号，映射到物理页号，并将该映射关系更新到页表
    pub fn map(&mut self, page_table: &mut PageTable) {
        for vpn in self.vpn_range {
            self.map_one(page_table, vpn);
        }
    }

    #[allow(unused)]
    // 取消整个逻辑段到物理页的映射，并更新到页表上
    pub fn unmap(&mut self, page_table: &mut PageTable) {
        for vpn in self.vpn_range {
            self.unmap_one(page_table, vpn);
        }
    }

    #[allow(unused)]
    pub fn shrink_to(&mut self, page_table: &mut PageTable, new_end: VirtPageNum) {
        for vpn in VPNRange::new(new_end, self.vpn_range.get_end()) {
            self.unmap_one(page_table, vpn)
        }
        // VPNRange的范围不包括end
        self.vpn_range = VPNRange::new(self.vpn_range.get_start(), new_end);
    }

    #[allow(unused)]
    pub fn append_to(&mut self, page_table: &mut PageTable, new_end: VirtPageNum) {
        for vpn in VPNRange::new(self.vpn_range.get_end(), new_end) {
            self.map_one(page_table, vpn)
        }
        self.vpn_range = VPNRange::new(self.vpn_range.get_start(), new_end);
    }

    // 将数据拷贝到该逻辑段映射的物理页中
    // data长度不能超过逻辑段的地址范围，同时它会被对齐到逻辑段的开头
    pub fn copy_data(&mut self, page_table: &PageTable, data: &[u8]) {
        assert_eq!(self.map_type, MapType::Framed);
        let mut start: usize = 0;
        let mut current_vpn = self.vpn_range.get_start();
        let len = data.len();
        loop {
            let end = min(len, start + PAGE_SIZE);
            let src = &data[start..end];
            let dst = &mut page_table
                .translate(current_vpn)
                .unwrap()
                .ppn()
                .get_bytes_array()[..src.len()];
            dst.copy_from_slice(src);
            start += PAGE_SIZE;
            if start >= len {
                break;
            }
            current_vpn.step();
        }
    }
}

#[allow(unused)]
// 测试内核空间的多级页表是否正确设置
pub fn remap_test() {
    extern "C" {
        fn stext();
        fn etext();
        fn srodata();
        fn erodata();
        fn sdata();
        fn edata();
    }
    let mut kernel_space = KERNEL_SPACE.exclusive_access();
    let mid_text: VirtAddr = ((stext as usize + etext as usize) / 2).into();
    let mid_rodata: VirtAddr = ((srodata as usize + erodata as usize) / 2).into();
    let mid_data: VirtAddr = ((sdata as usize + edata as usize) / 2).into();
    assert!(!kernel_space
        .page_table
        .translate(mid_text.floor())
        .unwrap()
        .writable(),);
    assert!(!kernel_space
        .page_table
        .translate(mid_rodata.floor())
        .unwrap()
        .writable(),);
    assert!(!kernel_space
        .page_table
        .translate(mid_data.floor())
        .unwrap()
        .executable(),);
    println!("remap_test passed!");
}
