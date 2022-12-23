/// File extraction from Pikmin 2 & romhack ISOs.

use std::{path::{Path, PathBuf}, fs::{create_dir_all, self, File}, io::{BufReader, Read, Seek, SeekFrom, Cursor}, panic::catch_unwind};
use anyhow::anyhow;
use gc_gcm::{GcmFile, DirEntry};
use indicatif::{ProgressBar, ParallelProgressIterator};
use log::warn;
use rayon::prelude::{IntoParallelIterator, ParallelIterator};
use regex::Regex;
use yaz0::Yaz0Archive;
use crate::{rarc::Rarc, bti::BtiImage};


pub fn extract_iso<P: AsRef<Path>>(game_name: Option<String>, iso_path: P, progress: &ProgressBar) -> Result<(), anyhow::Error> {
    progress.set_message("Reading ISO file system");
    progress.inc(1);

    let iso_path = iso_path.as_ref();
    let iso = GcmFile::open(iso_path).map_err(|_| anyhow!("Couldn't parse ISO!"))?;
    let all_files = traverse_filesystem(&iso);
    let game_id_raw = format!("{:?}", iso.game_id);
    let game_id = game_id_raw.trim_matches('"');

    let game_name = if let Some(override_name) = game_name {
        override_name
    }
    else {
        match game_id {
            "GPVE01" | "GPVJ01" | "GPVP01" => "pikmin2",
            "PIKE25" => "251",
            "WSAE64" => "newyear",
            _ => return Err(anyhow!("Unrecognized game ISO {} and no game override provided", game_id))
        }.to_string()
    };

    let matchers: Vec<DesiredFileMatcher> = match game_id {
        "PIKE25" => vec![
            // TODO: figure out how to not duplicate these dest strings, since they'll be the same for every arm
            DesiredFileMatcher::new(PathBuf::from("caveinfo/{0}.txt"), vec!["caves", r"(.+)\.txt"]),
            DesiredFileMatcher::new(PathBuf::from("treasures/{0}.bti"), vec!["piklopedia_us", "treasureicon.szs", r"(.+)\.bti"]),
            DesiredFileMatcher::new(PathBuf::from("mapunits/{0}/{1}/{2}"), vec!["caves", "assets", r"(.+)", r"(.+)\.szs", r"([^\.]+\.(?:bti|txt))"]),
            DesiredFileMatcher::new(PathBuf::from("unitfiles/{0}.txt"), vec!["caves", "unit_lists", r"(.+)\.txt"]),
            DesiredFileMatcher::new(PathBuf::from("teki/{0}.bti"), vec!["piklopedia_us", "enemyicon.szs", r"(.+)", "texture.bti"]),
            DesiredFileMatcher::new(PathBuf::from("otakara_config.txt"), vec!["Treasure", "pelletlist_us.szs", "otakara_config.txt"]),
            DesiredFileMatcher::new(PathBuf::from("item_config.txt"), vec!["Treasure", "pelletlist_us.szs", "item_config.txt"]),
        ],
        _ => vec![
            DesiredFileMatcher::new(PathBuf::from("caveinfo/{0}.txt"), vec!["user", "Mukki", "mapunits", "caveinfo", r"(.+)\.txt"]),
            DesiredFileMatcher::new(PathBuf::from("treasures/{0}.bti"), vec!["user", "Matoba", "resulttex", "us", "arc.szs", r"(.+)", "texture.bti"]),
            DesiredFileMatcher::new(PathBuf::from("mapunits/{0}/{1}/{2}"), vec!["user", "Mukki", "mapunits", "arc", r"(.+)", r"(.+)\.szs", r"([^\.]+\.(?:bti|txt))"]),
            DesiredFileMatcher::new(PathBuf::from("unitfiles/{0}.txt"), vec!["user", "Mukki", "mapunits", "units", r"(.+)\.txt"]),
            DesiredFileMatcher::new(PathBuf::from("teki/{0}.bti"), vec!["user", "Yamashita", "enemytex", "arc.szs", r"(.+)", "texture.bti"]),
            DesiredFileMatcher::new(PathBuf::from("otakara_config.txt"), vec!["user", "Abe", "Pellet", "us", "pelletlist_us.szs", "otakara_config.txt"]),
            DesiredFileMatcher::new(PathBuf::from("item_config.txt"), vec!["user", "Abe", "Pellet", "us", "pelletlist_us.szs", "item_config.txt"]),
        ]
    };

    all_files.into_par_iter().progress_with(progress.clone()).try_for_each(|f| -> Result<(), anyhow::Error> {
        progress.set_message(f.path.to_string_lossy().to_string());
        let mut iso_reader = BufReader::new(File::open(iso_path)?);

        if let Some("szs") = f.path.extension().and_then(|e| e.to_str()) {
            let is_prefix_of_desired_path = matchers.iter()
                .any(|m| {
                    let p_components = f.path.components();
                    m.source.iter().zip(p_components)
                        .all(|(m, p)| m.is_match(p.as_os_str().to_str().unwrap_or_default()))
                });
            if !is_prefix_of_desired_path {
                return Ok(());
            }

            let data = f.read(&mut iso_reader)?;
            let arc = if &data[..4] == b"Yaz0" {
                Yaz0Archive::new(Cursor::new(data.as_slice()))?.decompress()?
            }
            else {
                data
            };
            let rarc = Rarc::new(arc.as_slice()).expect("Rarc decompression error!");

            for (subpath, data) in rarc.files() {
                let mut full_path = f.path.clone();
                full_path.extend(&subpath);
                progress.set_message(full_path.to_string_lossy().to_string());

                for matcher in matchers.iter() {
                    if let Some(dest) = matcher.matches(&full_path) {
                        let mut full_dest = PathBuf::from_iter(["assets", &game_name]);
                        full_dest.push(dest);
                        write_file(&full_dest, data)?;
                        break;
                    }
                }
            }
        }
        else {
            for matcher in matchers.iter() {
                if let Some(dest) = matcher.matches(&f.path) {
                    let data = f.read(&mut iso_reader)?;
                    let mut full_dest = PathBuf::from_iter(["assets", &game_name]);
                    full_dest.push(dest);
                    write_file(&full_dest, &data)?;
                    break;
                }
            }
        }
        Ok(())
    })?;

    Ok(())
}

fn write_file(dest: &Path, data: &[u8]) -> Result<(), anyhow::Error> {
    create_dir_all(dest.parent().unwrap())?;
    if let Some("bti") = dest.extension().and_then(|e| e.to_str()) {
        let mut dest = dest.to_path_buf();
        dest.set_extension("png");

        // TODO: fix file errors in 251?
        let res = catch_unwind(|| {
            let bti = BtiImage::decode(data);
            image::save_buffer_with_format(
                &dest,
                bti.pixels().flatten().cloned().collect::<Vec<_>>().as_slice(),
                bti.width as u32,
                bti.height as u32,
                image::ColorType::Rgba8,
                image::ImageFormat::Png,
            ).unwrap();
        });
        if res.is_err() {
            warn!("Decoding and saving {:?} failed. Skipping.", dest);
        }
    }
    else {
        fs::write(dest, data)?;
    }

    Ok(())
}

struct DesiredFileMatcher {
    destination: PathBuf,
    source: Vec<Regex>,
}

impl DesiredFileMatcher {
    pub fn new(destination: PathBuf, source: Vec<&str>) -> Self {
        Self {
            destination,
            source: source.into_iter().map(|r| Regex::new(r).unwrap()).collect()
        }
    }

    /// Returns the reified final path upon successful match
    pub fn matches(&self, path: &Path) -> Option<PathBuf> {
        let path_components = path.components().collect::<Vec<_>>();
        let all_components_match = self.source.iter()
            .zip(path_components.iter())
            .all(|(m, p)| m.is_match(p.as_os_str().to_str().unwrap_or_default()));

        if self.source.len() != path_components.len() || !all_components_match {
            return None;
        }

        let fillers = self.source.iter()
            .zip(path_components.iter())
            .filter_map(|(m, p)| m.captures(p.as_os_str().to_str()?))
            .flat_map(|c| c.iter().skip(1).filter_map(|c| Some(c?.as_str().trim())).collect::<Vec<_>>());

        let mut final_path = self.destination.to_string_lossy();
        for (i, filler) in fillers.enumerate() {
            final_path = final_path.replace(&format!("{{{i}}}"), filler).into();
        }
        Some(PathBuf::from(&*final_path))
    }
}

#[derive(Debug)]
struct VirtualFile<'a> {
    pub path: PathBuf,
    pub entry: DirEntry<'a>
}

impl<'a> VirtualFile<'a> {
    fn wrap(entry: DirEntry<'a>, path: PathBuf) -> Self {
        Self { path, entry }
    }

    fn read(&self, iso_reader: &mut BufReader<File>) -> std::io::Result<Vec<u8>> {
        let file_location = self.entry.as_file().unwrap();
        let mut data = vec![0u8; file_location.size as usize];
        iso_reader.seek(SeekFrom::Start(file_location.offset as u64))?;
        iso_reader.read_exact(&mut data)?;
        Ok(data)
    }
}

fn traverse_filesystem(iso: &GcmFile) -> Vec<VirtualFile<'_>> {
    traverse_fs_recursive(iso.filesystem.iter_root().map(|e| VirtualFile::wrap(e, PathBuf::new())).collect())
}

fn traverse_fs_recursive(entries: Vec<VirtualFile<'_>>) -> Vec<VirtualFile<'_>> {
    let (mut files, directories): (Vec<_>, Vec<_>) = entries.into_iter().partition(|e| e.entry.is_file());
    files.iter_mut().for_each(|f| f.path.push(f.entry.entry_name()));
    files.extend(directories.into_iter()
        .flat_map(|mut d| {
            d.path.push(d.entry.entry_name());
            traverse_fs_recursive(d.entry.iter_dir().unwrap().map(|e| VirtualFile::wrap(e, d.path.clone())).collect())
        })
    );
    files
}
