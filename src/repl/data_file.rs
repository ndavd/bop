use std::path::PathBuf;

pub static DATA_FILE: &str = ".bop-data";

pub fn get_data_file_path() -> Result<PathBuf, String> {
    let home = match dirs::config_dir() {
        Some(x) => x,
        None => return Err("Could not find config directory".to_string()),
    };
    Ok(home.join(DATA_FILE))
}

pub fn data_file_exists() -> Result<bool, String> {
    Ok(std::fs::metadata(get_data_file_path()?).is_ok())
}

pub fn read_data_file() -> Result<Vec<u8>, String> {
    match std::fs::read(get_data_file_path()?) {
        Ok(x) => Ok(x),
        _ => Err("Could not read data file".to_string()),
    }
}

pub fn write_data_file(contents: &[u8]) -> Result<(), String> {
    match std::fs::write(get_data_file_path()?, contents) {
        Ok(_) => Ok(()),
        _ => Err("Could not write data file".to_string()),
    }
}
