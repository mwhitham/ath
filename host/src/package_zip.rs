//! A package folder as one zip file, for upload and download. Unpacking refuses any
//! entry that would land outside the target folder.

use std::io::{Read, Write};
use std::path::{Component, Path};

pub fn pack(folder: &Path) -> Result<Vec<u8>, String> {
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut stack = vec![folder.to_path_buf()];
    let mut files = Vec::new();
    while let Some(directory) = stack.pop() {
        for entry in std::fs::read_dir(&directory).map_err(|error| error.to_string())? {
            let path = entry.map_err(|error| error.to_string())?.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                files.push(path);
            }
        }
    }
    files.sort();
    for path in files {
        let name = path.strip_prefix(folder).map_err(|error| error.to_string())?.to_string_lossy().replace('\\', "/");
        writer.start_file(name, options).map_err(|error| error.to_string())?;
        writer
            .write_all(&std::fs::read(&path).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    }
    Ok(writer.finish().map_err(|error| error.to_string())?.into_inner())
}

pub fn unpack(bytes: &[u8], folder: &Path) -> Result<(), String> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|error| error.to_string())?;
    std::fs::create_dir_all(folder).map_err(|error| error.to_string())?;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|error| error.to_string())?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().to_string();
        let relative = Path::new(&name);
        if relative.components().any(|component| !matches!(component, Component::Normal(_))) {
            return Err(format!("the zip names a path outside the package: {name}"));
        }
        let target = folder.join(relative);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let mut contents = Vec::new();
        entry.read_to_end(&mut contents).map_err(|error| error.to_string())?;
        std::fs::write(&target, contents).map_err(|error| error.to_string())?;
    }
    Ok(())
}
