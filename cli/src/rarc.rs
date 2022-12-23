use std::{borrow::Cow, path::PathBuf};

use crate::util::{read_u32, read_u16, read_str_until_null};


pub struct Rarc<'a> {
    data: &'a [u8],
    file_data_list_offset: u32,
    nodes: Vec<RarcNode>,
    files: Vec<RarcFile<'a>>,
}

#[derive(Debug)]
pub enum RarcError {
    MagicError(usize),
}

impl<'a> Rarc<'a> {
    pub fn new(data: &'a [u8]) -> Result<Rarc<'a>, RarcError> {
        if &data[0..4] != b"RARC" {
            return Err(RarcError::MagicError(0));
        }

        let data_header_offset = read_u32(data, 0x8);
        if data_header_offset != 0x20 {
            println!("{data_header_offset:#X}");
            return Err(RarcError::MagicError(1));
        }

        let file_data_list_offset = read_u32(data, 0xC) + data_header_offset;
        let unk1 = read_u32(data, 0x1C);
        if unk1 != 0 {
            return Err(RarcError::MagicError(2));
        }

        let num_nodes = read_u32(data, data_header_offset);
        let node_list_offset = read_u32(data, data_header_offset + 0x4) + data_header_offset;
        let total_num_file_entries = read_u32(data, data_header_offset + 0x8);
        let file_entries_list_offset = read_u32(data, data_header_offset + 0x0C) + data_header_offset;
        let string_list_offset = read_u32(data, data_header_offset + 0x14) + data_header_offset;
        let unk2 = data[data_header_offset as usize + 0x1B];
        let unk3 = data[data_header_offset as usize + 0x1C];
        if unk2 != 0 || unk3 != 0 {
            return Err(RarcError::MagicError(3));
        }

        let mut nodes = Vec::with_capacity(num_nodes as usize);
        for node_idx in 0..num_nodes {
            nodes.push(RarcNode::read(data, node_list_offset + node_idx*0x10));
        }

        let mut files = Vec::with_capacity(total_num_file_entries as usize);
        for file_idx in 0..total_num_file_entries {
            files.push(RarcFile::read(data, file_entries_list_offset + file_idx*0x14, string_list_offset));
        }

        Ok(Rarc {
            data,
            file_data_list_offset,
            nodes,
            files,
        })
    }

    pub fn files(&self) -> impl Iterator<Item=(PathBuf, &[u8])> {
        let root_node = &self.nodes[0];
        let files_with_paths = self.files_for_node(root_node, PathBuf::new());
        files_with_paths.into_iter()
            .filter(|(_, file)| ![".", ".."].contains(&&file.name[..]))
            .map(|(mut path, file)| {
                path.push(&file.name[..]);
                let file_start = (self.file_data_list_offset + file.data_offset_or_node_index) as usize;
                let file_end = file_start + file.data_size as usize;
                (path, &self.data[file_start..file_end])
            })
    }

    fn files_for_node(&self, node: &RarcNode, parent_path: PathBuf) -> Vec<(PathBuf, &RarcFile)> {
        let file_entries = &self.files[node.first_file_index as usize .. (node.first_file_index + node.num_files as u32) as usize];
        let (dirs, files): (Vec<_>, Vec<_>) = file_entries.iter().partition(|e| e.is_dir());
        let mut files_with_paths: Vec<_> = files.into_iter().map(|f| (parent_path.clone(), f)).collect();
        for file in dirs {
            if ![".", ".."].contains(&&file.name[..]) {
                let sub_node = &self.nodes[file.data_offset_or_node_index as usize];
                let mut new_parent_path = parent_path.clone();
                new_parent_path.push(&file.name[..]);
                files_with_paths.extend(self.files_for_node(sub_node, new_parent_path));
            }
        }
        files_with_paths
    }
}

struct RarcNode {
    pub num_files: u16,
    pub first_file_index: u32,
}

impl RarcNode {
    fn read(data: &[u8], node_offset: u32) -> Self {
        let num_files = read_u16(data, node_offset + 0xA);
        let first_file_index = read_u32(data, node_offset + 0xC);

        RarcNode {
            num_files, first_file_index
        }
    }
}

struct RarcFile<'a> {
    pub name: Cow<'a, str>,
    pub data_size: u32,
    pub data_offset_or_node_index: u32,
    pub file_type_flags: u32,
}

impl<'a> RarcFile<'a> {
    fn read(data: &'a [u8], file_offset: u32, string_list_offset: u32) -> Self {
        let type_and_name_offset = read_u32(data, file_offset + 0x4);
        let data_offset_or_node_index = read_u32(data, file_offset + 0x8);
        let data_size = read_u32(data, file_offset + 0xC);
        let file_type_flags = (type_and_name_offset & 0xFF000000) >> 24;
        let name_offset = type_and_name_offset & 0x00FFFFFF;
        let name = read_str_until_null(data, string_list_offset + name_offset);

        RarcFile {
            name, data_size, data_offset_or_node_index, file_type_flags
        }
    }

    fn is_dir(&self) -> bool {
        self.file_type_flags & 0x02 != 0
    }
}
