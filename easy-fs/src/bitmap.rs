use crate::{block_cache::get_block_cache, block_dev::BlockDevice, BLOCK_SZ};
use alloc::sync::Arc;

// 填充一个数据块的位图类型
type BitmapBlock = [u64; 64];
// 每个块能包含的位数
const BLOCK_BITS: usize = BLOCK_SZ * 8;

// 位图，每个比特位都表示一个数据块的使用情况
// 存储该数据的块有blocks个，从start_block_id开始
pub struct Bitmap {
    start_block_id: usize,
    blocks: usize,
}

impl Bitmap {
    pub fn new(start_block_id: usize, blocks: usize) -> Self {
        Self {
            start_block_id,
            blocks,
        }
    }

    // 分配一个块，即从位图中找到值为0的比特位，设为1。
    // 返回值为该比特位的编号（在整个位图中的位置）
    pub fn alloc(&self, block_device: &Arc<dyn BlockDevice>) -> Option<usize> {
        // 分配方式：找到第一个比特位为0的位置。
        // 这个位图放在多个块中，因此需要挨个遍历它们
        for block_id in 0..self.blocks {
            let pos = get_block_cache(
                block_id + self.start_block_id as usize,
                Arc::clone(block_device),
            )
            .lock()
            .modify(0, |bitmap_block: &mut BitmapBlock| {
                // 找到当前位图块的第一个比特位为0的位置
                if let Some((bits64_pos, inner_pos)) = bitmap_block
                    .iter()
                    .enumerate()
                    // 找到第一个不全为1的64位
                    .find(|(_, bits64)| **bits64 != u64::MAX)
                    // 找到最低位的0的位置。trailing_ones返回最低位的连续1的个数。
                    .map(|(bits64_pos, bits64)| (bits64_pos, bits64.trailing_ones() as usize))
                {
                    // 如果找到，将其设置为1，表示将该块分配出来。
                    bitmap_block[bits64_pos] |= 1u64 << inner_pos;
                    // 返回值为该比特位在所有位图中的位置
                    Some(block_id * BLOCK_BITS + bits64_pos * 64 + inner_pos as usize)
                } else {
                    None
                }
            });
            if pos.is_some() {
                return pos;
            }
        }
        None
    }

    // 释放一个块，即将对应的比特位设为0
    pub fn dealloc(&self, block_device: &Arc<dyn BlockDevice>, bit: usize) {
        let (block_pos, bits64_pos, inner_pos) = decomposition(bit);
        get_block_cache(block_pos + self.start_block_id, Arc::clone(block_device))
            .lock()
            .modify(0, |bitmap_block: &mut BitmapBlock| {
                assert!(bitmap_block[bits64_pos] & (1u64 << inner_pos) > 0);
                bitmap_block[bits64_pos] -= 1u64 << inner_pos;
            });
    }

    // 获取最大可分配的块数
    pub fn maximum(&self) -> usize {
        self.blocks * BLOCK_BITS
    }
}

// 将一个比特位的编号分解为：(块号，块中的第几个u64，该比特位在u64中的位置)
fn decomposition(mut bit: usize) -> (usize, usize, usize) {
    let block_pos = bit / BLOCK_BITS;
    bit %= BLOCK_BITS;
    (block_pos, bit / 64, bit % 64)
}
