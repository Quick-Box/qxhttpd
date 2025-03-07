use std::io::Read;
use anyhow::anyhow;
use chrono::NaiveDateTime;
use rocket::http::Status;
use rocket::response::status::Custom;

pub(crate) fn dtstr(iso_date_str: Option<&str>) -> String {
    let Some(s) = iso_date_str else {
        return "--/--/--".to_string()
    };
    let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") else {
        return s.to_string()
    };
    dt.format("%Y-%m-%d %H:%M:%S").to_string()
}

pub(crate) fn parse_naive_datetime(datetime_str: &str) -> Option<NaiveDateTime> {
    for &format in &[
        "%Y-%m-%d %H:%M:%S",       // 2025-03-05 14:32:45
        "%Y-%m-%d %H:%M",          // 2025-03-05 14:32
        "%Y-%m-%dT%H:%M:%S",       // 2025-03-05T14:32:45
        "%Y-%m-%dT%H:%M",          // 2025-03-05T14:32
        "%d/%m/%Y %H:%M:%S",       // 05/03/2025 14:32:45
        // "%m/%d/%Y %H:%M:%S",       // 03/05/2025 14:32:45
        // "%Y/%m/%d %H:%M:%S",       // 2025/03/05 14:32:45
        // "%Y-%m-%d",                // 2025-03-05
        // "%m/%d/%Y",                // 03/05/2025
        "%d/%m/%Y",                // 05/03/2025
        "%H:%M:%S",                // 14:32:45
    ] {
        if let Ok(parsed) = NaiveDateTime::parse_from_str(datetime_str, format) {
            return Some(parsed);
        }
    }

    // Return None if no format matched
    None
}

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
    use crate::util::unzip_data;

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

pub(crate) fn status_sqlx_error(err: sqlx::Error) -> Custom<String> {
    Custom(Status::InternalServerError, format!("SQLx error: {}", err))
}
pub(crate) fn tee_sqlx_error(err: sqlx::Error) -> anyhow::Error {
    warn!("Error: {}", err);
    anyhow!("SQL error: {}", err)
}
pub(crate) fn tee_any_error(err: anyhow::Error) -> anyhow::Error {
    warn!("Error: {}", err);
    err
}
