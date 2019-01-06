#[derive(Debug)]
pub enum Validation {
    Good,
    Bad,
}

pub fn validate(file_name: &str, zip_bytes: &[u8]) -> Validation {
    let z = match zip::ZipArchive::new(std::io::Cursor::new(zip_bytes)) {
        Ok(z) => z,
        Err(_) => return Validation::Bad,
    };
    // TODO: Validate more than just "is a zip file".
    Validation::Good
}
