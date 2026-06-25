// kernel/src/fs/ext2.rs
// Ext2 Filesystem Driver (Read-Only)

use alloc::string::String;
use alloc::vec::Vec;
use core::mem;
use crate::driver::DriverManager;
use crate::kprint;

const EXT2_SIGNATURE: u16 = 0xef53;
pub const EXT2_ROOT_INODE: u32 = 2;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Ext2Superblock {
    pub inodes_count: u32,
    pub blocks_count: u32,
    pub r_blocks_count: u32,
    pub free_blocks_count: u32,
    pub free_inodes_count: u32,
    pub first_data_block: u32,
    pub log_block_size: u32,
    pub log_frag_size: u32,
    pub blocks_per_group: u32,
    pub frags_per_group: u32,
    pub inodes_per_group: u32,
    pub mtime: u32,
    pub wtime: u32,
    pub mnt_count: u16,
    pub max_mnt_count: u16,
    pub magic: u16,
    pub state: u16,
    pub errors: u16,
    pub minor_rev_level: u16,
    pub lastcheck: u32,
    pub checkinterval: u32,
    pub creator_os: u32,
    pub rev_level: u32,
    pub def_resuid: u16,
    pub def_resgid: u16,
    // EXT2_DYNAMIC_REV Specific:
    pub first_ino: u32,
    pub inode_size: u16,
    pub block_group_nr: u16,
    pub feature_compat: u32,
    pub feature_incompat: u32,
    pub feature_ro_compat: u32,
    // remaining fields ignored for basic read...
    pub padding: [u8; 936],
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Ext2BlockGroupDescriptor {
    pub bg_block_bitmap: u32,
    pub bg_inode_bitmap: u32,
    pub bg_inode_table: u32,
    pub bg_free_blocks_count: u16,
    pub bg_free_inodes_count: u16,
    pub bg_used_dirs_count: u16,
    pub bg_pad: u16,
    pub bg_reserved: [u32; 3],
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Ext2Inode {
    pub i_mode: u16,
    pub i_uid: u16,
    pub i_size: u32,
    pub i_atime: u32,
    pub i_ctime: u32,
    pub i_mtime: u32,
    pub i_dtime: u32,
    pub i_gid: u16,
    pub i_links_count: u16,
    pub i_blocks: u32,
    pub i_flags: u32,
    pub i_osd1: u32,
    pub i_block: [u32; 15],
    pub i_generation: u32,
    pub i_file_acl: u32,
    pub i_dir_acl: u32,
    pub i_faddr: u32,
    pub i_osd2: [u32; 3],
}

pub struct Ext2Fs {
    pub superblock: Ext2Superblock,
    pub block_size: usize,
    pub inode_size: usize,
    pub groups_count: usize,
    pub block_groups: Vec<Ext2BlockGroupDescriptor>,
    pub device_path: String,
}

impl Ext2Fs {
    pub fn mount(device_path: &str, dm: &DriverManager) -> Result<Self, &'static str> {
        // Superblock is always at offset 1024
        let sb_data = dm.read_device(device_path, 1024, mem::size_of::<Ext2Superblock>())
            .ok_or("Failed to read superblock")?;
            
        if sb_data.len() < mem::size_of::<Ext2Superblock>() {
            return Err("Incomplete superblock read");
        }

        let superblock = unsafe { core::ptr::read_unaligned(sb_data.as_ptr() as *const Ext2Superblock) };

        if superblock.magic != EXT2_SIGNATURE {
            kprint!("[Ext2] Magic mismatch: {:x}\n", superblock.magic);
            return Err("Invalid Ext2 signature");
        }

        let block_size = 1024 << superblock.log_block_size;
        let inode_size = if superblock.rev_level == 0 { 128 } else { superblock.inode_size as usize };
        
        let groups_count = ((superblock.blocks_count - superblock.first_data_block + superblock.blocks_per_group - 1) / superblock.blocks_per_group) as usize;

        // Block Group Descriptor Table starts at the block following the superblock
        let bgdt_block = if block_size == 1024 { 2 } else { 1 };
        let bgdt_offset = bgdt_block * block_size;
        
        let bg_size = mem::size_of::<Ext2BlockGroupDescriptor>();
        let bg_total_size = groups_count * bg_size;
        
        let bg_data = dm.read_device(device_path, bgdt_offset, bg_total_size)
            .ok_or("Failed to read BGDT")?;

        let mut block_groups = Vec::with_capacity(groups_count);
        for i in 0..groups_count {
            let offset = i * bg_size;
            let bg = unsafe { core::ptr::read_unaligned(bg_data.as_ptr().add(offset) as *const Ext2BlockGroupDescriptor) };
            block_groups.push(bg);
        }

        kprint!("[Ext2] Mounted {} ({} blocks, {} inodes, Block size: {})\n", device_path, superblock.blocks_count, superblock.inodes_count, block_size);

        Ok(Ext2Fs {
            superblock,
            block_size,
            inode_size,
            groups_count,
            block_groups,
            device_path: String::from(device_path),
        })
    }

    pub fn read_block(&self, block: u32, dm: &DriverManager) -> Option<Vec<u8>> {
        let offset = (block as usize) * self.block_size;
        dm.read_device(&self.device_path, offset, self.block_size)
    }

    pub fn get_inode(&self, inode_num: u32, dm: &DriverManager) -> Option<Ext2Inode> {
        if inode_num == 0 || inode_num > self.superblock.inodes_count {
            return None;
        }

        let bg_idx = ((inode_num - 1) / self.superblock.inodes_per_group) as usize;
        let local_idx = ((inode_num - 1) % self.superblock.inodes_per_group) as usize;

        let bg = &self.block_groups[bg_idx];
        let inode_table_block = bg.bg_inode_table;
        
        let offset = (inode_table_block as usize) * self.block_size + local_idx * self.inode_size;
        
        let data = dm.read_device(&self.device_path, offset, mem::size_of::<Ext2Inode>())?;
        let inode = unsafe { core::ptr::read_unaligned(data.as_ptr() as *const Ext2Inode) };
        Some(inode)
    }

    pub fn read_inode_data(&self, inode: &Ext2Inode, dm: &DriverManager) -> Vec<u8> {
        let mut data = Vec::new();
        let total_size = inode.i_size as usize;
        let mut remaining = total_size;

        // Phase 1: Only support direct blocks for now (12 * block_size bytes max)
        for i in 0..12 {
            let block = inode.i_block[i];
            if block == 0 || remaining == 0 { break; }
            
            if let Some(block_data) = self.read_block(block, dm) {
                let to_copy = core::cmp::min(remaining, self.block_size);
                data.extend_from_slice(&block_data[0..to_copy]);
                remaining -= to_copy;
            } else {
                break;
            }
        }
        
        let pointers_per_block = self.block_size / 4;
        
        let read_data_blocks = |pointers: &[u32], remain: &mut usize, out: &mut Vec<u8>| {
            for &block in pointers {
                if block == 0 || *remain == 0 { break; }
                if let Some(block_data) = self.read_block(block, dm) {
                    let to_copy = core::cmp::min(*remain, self.block_size);
                    out.extend_from_slice(&block_data[0..to_copy]);
                    *remain -= to_copy;
                }
            }
        };

        // Singly indirect
        if remaining > 0 && inode.i_block[12] != 0 {
            if let Some(indirect_block) = self.read_block(inode.i_block[12], dm) {
                let pointers: &[u32] = unsafe {
                    core::slice::from_raw_parts(indirect_block.as_ptr() as *const u32, pointers_per_block)
                };
                read_data_blocks(pointers, &mut remaining, &mut data);
            }
        }

        // Doubly indirect
        if remaining > 0 && inode.i_block[13] != 0 {
            if let Some(d_indirect_block) = self.read_block(inode.i_block[13], dm) {
                let d_pointers: &[u32] = unsafe {
                    core::slice::from_raw_parts(d_indirect_block.as_ptr() as *const u32, pointers_per_block)
                };
                for &s_block in d_pointers {
                    if s_block == 0 || remaining == 0 { break; }
                    if let Some(s_indirect_block) = self.read_block(s_block, dm) {
                        let pointers: &[u32] = unsafe {
                            core::slice::from_raw_parts(s_indirect_block.as_ptr() as *const u32, pointers_per_block)
                        };
                        read_data_blocks(pointers, &mut remaining, &mut data);
                    }
                }
            }
        }

        // Triply indirect
        if remaining > 0 && inode.i_block[14] != 0 {
            if let Some(t_indirect_block) = self.read_block(inode.i_block[14], dm) {
                let t_pointers: &[u32] = unsafe {
                    core::slice::from_raw_parts(t_indirect_block.as_ptr() as *const u32, pointers_per_block)
                };
                for &d_block in t_pointers {
                    if d_block == 0 || remaining == 0 { break; }
                    if let Some(d_indirect_block) = self.read_block(d_block, dm) {
                        let d_pointers: &[u32] = unsafe {
                            core::slice::from_raw_parts(d_indirect_block.as_ptr() as *const u32, pointers_per_block)
                        };
                        for &s_block in d_pointers {
                            if s_block == 0 || remaining == 0 { break; }
                            if let Some(s_indirect_block) = self.read_block(s_block, dm) {
                                let pointers: &[u32] = unsafe {
                                    core::slice::from_raw_parts(s_indirect_block.as_ptr() as *const u32, pointers_per_block)
                                };
                                read_data_blocks(pointers, &mut remaining, &mut data);
                            }
                        }
                    }
                }
            }
        }
        
        data
    }

    pub fn resolve_path(&self, path: &str, dm: &DriverManager) -> Option<u32> {
        let mut current_inode_num = EXT2_ROOT_INODE;
        
        let parts = path.split('/').filter(|p| !p.is_empty());
        
        for part in parts {
            let inode = self.get_inode(current_inode_num, dm)?;
            
            // Check if directory
            if (inode.i_mode & 0xF000) != 0x4000 {
                return None; // Not a directory
            }

            let dir_data = self.read_inode_data(&inode, dm);
            let mut offset = 0;
            let mut found = false;

            while offset < dir_data.len() {
                let inode_n = u32::from_le_bytes(dir_data[offset..offset+4].try_into().unwrap());
                let rec_len = u16::from_le_bytes(dir_data[offset+4..offset+6].try_into().unwrap()) as usize;
                let name_len = dir_data[offset+6] as usize;
                
                if inode_n != 0 {
                    let name = core::str::from_utf8(&dir_data[offset+8..offset+8+name_len]).unwrap_or("");
                    if name == part {
                        current_inode_num = inode_n;
                        found = true;
                        break;
                    }
                }
                offset += rec_len;
            }

            if !found { return None; }
        }

        Some(current_inode_num)
    }

    pub fn list_dir(&self, inode_num: u32, dm: &DriverManager) -> Vec<String> {
        let mut entries = Vec::new();
        if let Some(inode) = self.get_inode(inode_num, dm) {
            if (inode.i_mode & 0xF000) == 0x4000 {
                let dir_data = self.read_inode_data(&inode, dm);
                let mut offset = 0;
                while offset < dir_data.len() {
                    let inode_n = u32::from_le_bytes(dir_data[offset..offset+4].try_into().unwrap());
                    let rec_len = u16::from_le_bytes(dir_data[offset+4..offset+6].try_into().unwrap()) as usize;
                    let name_len = dir_data[offset+6] as usize;
                    
                    if inode_n != 0 && rec_len > 0 {
                        let name = core::str::from_utf8(&dir_data[offset+8..offset+8+name_len]).unwrap_or("");
                        if name != "." && name != ".." {
                            entries.push(String::from(name));
                        }
                    }
                    if rec_len == 0 { break; }
                    offset += rec_len;
                }
            }
        }
        entries
    }
}
