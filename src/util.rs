use std::backtrace::Backtrace;
use std::io::Read;
use anyhow::anyhow;
use rocket::http::Status;
use rocket::response::status::Custom;

pub(crate) fn unzip_data(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut z = flate2::read::ZlibDecoder::new(bytes);
    let mut s = Vec::new();
    z.read_to_end(&mut s).map_err(|e| { e.to_string() })?;
    Ok(s)
}

#[cfg(test)]
pub(crate) mod test {
    use std::io::Read;
    use flate2::bufread::ZlibEncoder;
    use flate2::Compression;
    use crate::util::{unzip_data};

    pub(crate) fn zip_data(bytes: &[u8]) -> Result<Vec<u8>, String> {
        let mut ret_vec = Vec::new();
        let mut deflater = ZlibEncoder::new(bytes, Compression::fast());
        deflater.read_to_end(&mut ret_vec).map_err(|e| e.to_string())?;
        Ok(ret_vec)
    }

    #[test]
    fn test_zip() {
        let data = b"foo bar baz";
        let zdata = zip_data(data).unwrap();
        let udata = unzip_data(&zdata).unwrap();
        assert_eq!(udata, data);
    }
}

pub(crate) fn string_to_custom_error(err: &str) -> Custom<String> {
    error!("Error: {err}\nbacktrace: {}", Backtrace::capture());
    Custom(Status::InternalServerError, format!("Error: {}", err))
}
pub(crate) fn sqlx_to_custom_error(err: sqlx::Error) -> Custom<String> {
    error!("SQL Error: {err}\nbacktrace: {}", Backtrace::capture());
    Custom(Status::InternalServerError, format!("SQLx error: {}", err))
}
pub(crate) fn anyhow_to_custom_error(err: anyhow::Error) -> Custom<String> {
    error!("Error: {err}\nbacktrace: {}", Backtrace::capture());
    Custom(Status::InternalServerError, format!("Error: {}", err))
}
pub(crate) fn sqlx_to_anyhow(err: sqlx::Error) -> anyhow::Error {
    error!("SQL Error: {err}\nbacktrace: {}", Backtrace::capture());
    anyhow!("SQL error: {}", err)
}
// pub(crate) fn tee_any_error(err: anyhow::Error) -> anyhow::Error {
//     warn!("Error: {}", err);
//     err
// }
