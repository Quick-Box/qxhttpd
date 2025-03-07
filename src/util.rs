use std::io::Read;
use anyhow::anyhow;
use chrono::{NaiveDateTime};
use rocket::http::Status;
use rocket::response::status::Custom;

pub(crate) fn obtime(sec_since_midnight: i64) -> String {
    let sec = sec_since_midnight % 60;
    let min = sec_since_midnight / 60;
    format!("{min}:{sec:0>2}")
}
pub(crate) fn dtstr(iso_date_str: Option<&str>) -> String {
    let Some(s) = iso_date_str else {
        return "--/--/--".to_string()
    };
    let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") else {
        return s.to_string()
    };
    dt.format("%Y-%m-%d %H:%M:%S").to_string()
}

pub(crate) fn try_parse_naive_datetime(datetime_str: &str) -> Option<NaiveDateTime> {
    let datetime_str = datetime_str.trim();
    // remove timezone if any
    let datetime_str = if let Some(time_sep) = datetime_str.find(|c| c == 'T' || c == ' ') {
        let time_str = &datetime_str[time_sep+1..];
        if let Some(tz_sep) = time_str.find(|c| c == '-' || c == '+') {
            &datetime_str[.. time_sep + tz_sep + 1]
        } else {
            datetime_str
        }
    } else {
        datetime_str
    };
    for &format in &[
        "%Y-%m-%d %H:%M:%S",       // 2025-03-05 14:32:45
        "%Y-%m-%d %H:%M",          // 2025-03-05 14:32
        "%Y-%m-%dT%H:%M:%S",       // 2025-03-05T14:32:45
        "%Y-%m-%dT%H:%M",          // 2025-03-05T14:32
        // "%d/%m/%Y %H:%M:%S",       // 05/03/2025 14:32:45
        // "%m/%d/%Y %H:%M:%S",       // 03/05/2025 14:32:45
        // "%Y/%m/%d %H:%M:%S",       // 2025/03/05 14:32:45
    ] {
        match NaiveDateTime::parse_from_str(datetime_str, format) {
            Ok(dt) => { return Some(dt) }
            Err(e) => {
                trace!("Failed to parse date time {datetime_str} with {format}: {:?}", e);
            }
        }
    }
    None
}
#[test]
fn test_parse_naive_datetime() {
    for (dtstr, dtstr2) in &[
        ("1970-03-05 14:32:45", "1970-03-05 14:32:45"),
        ("2025-03-05T14:32:45", "2025-03-05 14:32:45"),
        ("2025-03-05 14:32", "2025-03-05 14:32:00"),
        ("2025-03-05T14:32", "2025-03-05 14:32:00"),
        ("2025-03-05 14:32:45+10", "2025-03-05 14:32:45"),
        ("2025-03-05T14:32:45-01:30", "2025-03-05 14:32:45"),
    ] {
        let dt = try_parse_naive_datetime(dtstr).unwrap();
        // println!("{} -> {}", dtstr, dt.to_string());
        assert_eq!(&dt.to_string(), dtstr2)
    }
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

pub(crate) fn status_sqlx_error(err: sqlx::Error) -> Custom<String> {
    warn!("SQL Error: {}", err);
    Custom(Status::InternalServerError, format!("SQLx error: {}", err))
}
pub(crate) fn status_any_error(err: anyhow::Error) -> Custom<String> {
    warn!("Error: {}", err);
    Custom(Status::InternalServerError, format!("Error: {}", err))
}
pub(crate) fn tee_sqlx_error(err: sqlx::Error) -> anyhow::Error {
    warn!("SQL Error: {}", err);
    anyhow!("SQL error: {}", err)
}
// pub(crate) fn tee_any_error(err: anyhow::Error) -> anyhow::Error {
//     warn!("Error: {}", err);
//     err
// }
