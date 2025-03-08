use std::backtrace::Backtrace;
use std::io::Read;
use anyhow::anyhow;
use chrono::{DateTime, FixedOffset, SecondsFormat};
use rocket::http::Status;
use rocket::response::status::Custom;

pub struct QxDateTime(pub DateTime<FixedOffset>);
impl QxDateTime {
    fn to_display_string(&self) -> String {
        self.0.format("%F %T").to_string()
    }
    fn to_iso_string(&self) -> String {
        self.0.to_rfc3339_opts(SecondsFormat::Secs, true)
    }
    pub(crate) fn from_iso_string(datetime_str: &str) -> Result<Self, anyhow::Error> {
        let dt = DateTime::parse_from_rfc3339(datetime_str)?;
        // println!("{datetime_str} -> {dt:?}");
        Ok(Self(dt))
    }
}
impl From<DateTime<FixedOffset>> for QxDateTime {
    fn from(value: DateTime<FixedOffset>) -> Self {
        Self(value)
    }
}
#[test]
fn test_parse_qxdatetime() {
    for (dtstr, dtstr2) in &[
        ("1970-03-05 14:32:45Z", "1970-03-05T14:32:45Z"),
        ("2025-03-05T14:32:45Z", "2025-03-05T14:32:45Z"),
        ("2025-03-05 14:32:45+10:00", "2025-03-05T14:32:45+10:00"),
        ("2025-03-05T14:32:45-01:30", "2025-03-05T14:32:45-01:30"),
    ] {
        let dt = QxDateTime::from_iso_string(dtstr)
            .map_err(|e| println!("parse {dtstr} error: {e}")).unwrap();
        // println!("{} -> {:?}", dtstr, dt);
        assert_eq!(&dt.to_iso_string(), dtstr2)
    }
}
pub(crate) fn obtime(sec_since_midnight: i64) -> String {
    let sec = sec_since_midnight % 60;
    let min = sec_since_midnight / 60;
    format!("{min}:{sec:0>2}")
}
pub(crate) fn dtstr(iso_date_str: Option<&str>) -> String {
    let Some(s) = iso_date_str else {
        return "--/--/--".to_string()
    };
    if let Ok(dt) = QxDateTime::from_iso_string(s) {
        dt.to_display_string()
        
    } else {
        s.to_string()
    }
}

// pub(crate) fn try_parse_naive_datetime(datetime_str: &str) -> Option<NaiveDateTime> {
//     let datetime_str = datetime_str.trim();
//     // remove timezone if any
//     let datetime_str = if let Some(time_sep) = datetime_str.find(|c| c == 'T' || c == ' ') {
//         let time_str = &datetime_str[time_sep+1..];
//         if let Some(tz_sep) = time_str.find(|c| c == '-' || c == '+') {
//             &datetime_str[.. time_sep + tz_sep + 1]
//         } else {
//             datetime_str
//         }
//     } else {
//         datetime_str
//     };
//     for &format in &[
//         "%Y-%m-%d %H:%M:%S",       // 2025-03-05 14:32:45
//         "%Y-%m-%d %H:%M",          // 2025-03-05 14:32
//         "%Y-%m-%dT%H:%M:%S",       // 2025-03-05T14:32:45
//         "%Y-%m-%dT%H:%M",          // 2025-03-05T14:32
//         // "%d/%m/%Y %H:%M:%S",       // 05/03/2025 14:32:45
//         // "%m/%d/%Y %H:%M:%S",       // 03/05/2025 14:32:45
//         // "%Y/%m/%d %H:%M:%S",       // 2025/03/05 14:32:45
//     ] {
//         match NaiveDateTime::parse_from_str(datetime_str, format) {
//             Ok(dt) => { return Some(dt) }
//             Err(e) => {
//                 trace!("Failed to parse date time {datetime_str} with {format}: {:?}", e);
//             }
//         }
//     }
//     None
// }
//#[test]
// fn test_parse_naive_datetime() {
//     for (dtstr, dtstr2) in &[
//         ("1970-03-05 14:32:45", "1970-03-05 14:32:45"),
//         ("2025-03-05T14:32:45", "2025-03-05 14:32:45"),
//         ("2025-03-05 14:32", "2025-03-05 14:32:00"),
//         ("2025-03-05T14:32", "2025-03-05 14:32:00"),
//         ("2025-03-05 14:32:45+10", "2025-03-05 14:32:45"),
//         ("2025-03-05T14:32:45-01:30", "2025-03-05 14:32:45"),
//     ] {
//         let dt = try_parse_naive_datetime(dtstr).unwrap();
//         // println!("{} -> {}", dtstr, dt.to_string());
//         assert_eq!(&dt.to_string(), dtstr2)
//     }
// }

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
    error!("SQL Error: {err}\nbacktrace: {}", Backtrace::capture());
    Custom(Status::InternalServerError, format!("SQLx error: {}", err))
}
pub(crate) fn status_any_error(err: anyhow::Error) -> Custom<String> {
    error!("Error: {err}\nbacktrace: {}", Backtrace::capture());
    Custom(Status::InternalServerError, format!("Error: {}", err))
}
pub(crate) fn tee_sqlx_error(err: sqlx::Error) -> anyhow::Error {
    error!("SQL Error: {err}\nbacktrace: {}", Backtrace::capture());
    anyhow!("SQL error: {}", err)
}
// pub(crate) fn tee_any_error(err: anyhow::Error) -> anyhow::Error {
//     warn!("Error: {}", err);
//     err
// }
