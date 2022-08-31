/// File extraction from Pikmin 2 & romhack ISOs.

use std::{error::Error, path::{Path, PathBuf}, fs::{create_dir_all, self, File}, io::{BufReader, Read, Seek, SeekFrom, Cursor}};
use gc_gcm::{GcmFile, DirEntry};
use indicatif::ProgressBar;
use yaz0::Yaz0Archive;

use crate::{rarc::Rarc, bti::BtiImage};


#[derive(Debug)]
struct VirtualFile<'a> {
    pub name: &'a str,
    pub path_components: Vec<&'a str>,
    pub file: DirEntry<'a>
}

impl<'a> VirtualFile<'a> {
    fn wrap(entry: DirEntry<'a>, path_components: Vec<&'a str>) -> Self {
        Self {
            name: entry.entry_name(),
            path_components,
            file: entry,
        }
    }

    pub fn path_starts_with(&self, prefix: &[&str]) -> bool {
        prefix.iter()
            .zip(self.path_components.iter())
            .all(|(expected, actual)| expected == actual)
    }
}

pub fn extract_iso<P: AsRef<Path>>(game_name: &str, iso_path: P, progress: &ProgressBar) -> Result<(), Box<dyn Error>> {
    progress.set_message("Reading ISO file system");
    progress.inc(1);

    let iso = GcmFile::open(&iso_path).map_err(|e| format!("Error opening ISO file: {:?}", e))?;
    let mut iso_reader = BufReader::new(File::open(&iso_path)?);
    let all_files = traverse_filesystem(&iso);

    let relevant_files = all_files.iter()
        .filter(|f| {
            f.path_starts_with(&["user", "Mukki", "mapunits"])
            || f.path_starts_with(&["user", "Matoba", "resulttex"])
            || f.path_starts_with(&["user", "Yamashita", "enemytex"])
            || f.path_starts_with(&["user", "Abe", "Pellet"])
        });

    for f in relevant_files {
        let mut path = PathBuf::from("assets");
        path.push(game_name);
        path.extend(f.path_components.iter());

        // Discard these file types
        if let Some("ini" | "bnr" | "MAP") = f.name.split('.').last() {
            continue;
        }

        progress.set_message(path.to_string_lossy().into_owned());

        let file_location = f.file.as_file().unwrap();
        let mut data = vec![0u8; file_location.size as usize];
        iso_reader.seek(SeekFrom::Start(file_location.offset as u64))?;
        iso_reader.read_exact(&mut data)?;
        

        if f.name.ends_with(".szs") {
            path.push(f.name.strip_suffix(".szs").unwrap());
            let arc = Yaz0Archive::new(Cursor::new(data.as_slice()))?.decompress()?;
            let rarc = Rarc::new(arc.as_slice()).unwrap();
            for (sub_path, file_data) in rarc.files() {
                let mut decompressed_file_path = path.clone();
                decompressed_file_path.push(sub_path);
                progress.set_message(decompressed_file_path.to_string_lossy().into_owned());
                create_dir_all(decompressed_file_path.parent().unwrap())?;
                
                match decompressed_file_path.extension().unwrap().to_str().unwrap() {
                    "bmd" | "bin" => continue,  // Discard these file types
                    "bti" => {
                        progress.set_message(format!("Decoding {}", decompressed_file_path.to_string_lossy()));
                        decompressed_file_path.set_extension("png");
                        let bti = BtiImage::decode(file_data);
                        image::save_buffer_with_format(
                            &decompressed_file_path, 
                            bti.pixels().flatten().cloned().collect::<Vec<_>>().as_slice(), 
                            bti.width as u32, 
                            bti.height as u32, 
                            image::ColorType::Rgba8, 
                            image::ImageFormat::Png,
                        )?;
                    },
                    _ => {
                        fs::write(&decompressed_file_path, file_data)?;
                    },
                }
                progress.inc(1);
            }
        }
        else {
            create_dir_all(&path)?;
            path.push(f.name);
            fs::write(&path, data)?;
            progress.inc(1);
        }
    }

    Ok(())
}

fn traverse_filesystem(iso: &GcmFile) -> Vec<VirtualFile<'_>> {
    traverse_fs_recursive(iso.filesystem.iter_root().map(|e| VirtualFile::wrap(e, vec![])).collect())
}

fn traverse_fs_recursive(entries: Vec<VirtualFile<'_>>) -> Vec<VirtualFile<'_>> {
    let (mut files, directories): (Vec<_>, Vec<_>) = entries.into_iter().partition(|e| e.file.is_file());
    files.extend(directories.into_iter()
        .flat_map(|mut d| {
            d.path_components.push(d.name);
            traverse_fs_recursive(d.file.iter_dir().unwrap().map(|e| VirtualFile::wrap(e, d.path_components.clone())).collect())
        })
    );
    files
}
