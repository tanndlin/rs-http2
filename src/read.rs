use std::{
    collections::HashMap,
    fs::{DirEntry, read, read_dir},
};

pub fn cache_all_files(base_path: &str) -> Result<HashMap<String, Vec<u8>>, String> {
    dbg!(&base_path);
    let mut ret = HashMap::new();
    for path in get_all_paths(base_path)? {
        let path = path.path().to_str().unwrap().to_string().replace("\\", "/");
        ret.insert(
            path.trim_start_matches(base_path).to_string(),
            read(path).unwrap(),
        );
    }

    Ok(ret)
}

fn get_all_paths(path: &str) -> Result<Vec<DirEntry>, String> {
    let mut ret = vec![];
    for path in read_dir(path).map_err(|e| format!("Unable to read directory: {e}"))? {
        let path = path.map_err(|e| format!("Unable to read path: {e}"))?;
        if path.path().is_dir() {
            ret.extend(get_all_paths(path.path().to_str().unwrap())?);
        } else {
            ret.push(path);
        }
    }

    Ok(ret)
}
