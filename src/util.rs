use std::backtrace::Backtrace;
use std::io::Read;
use anyhow::anyhow;
use chrono::{DateTime, FixedOffset, SecondsFormat, TimeDelta};
use rocket::http::Status;
use rocket::response::status::Custom;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone, Copy)]
pub struct QxDateTime(pub DateTime<FixedOffset>);
impl QxDateTime {
    pub fn new(datetime: DateTime<FixedOffset>) -> QxDateTime {
        let millis = datetime.timestamp_subsec_millis();
        let nanos = datetime.timestamp_subsec_nanos() - millis * 1_000_000;
        if let Some(dt) = datetime.checked_sub_signed(TimeDelta::nanoseconds(nanos as i64)) {
            QxDateTime(dt)
        } else {
            QxDateTime(datetime)
        }
    }
    fn to_display_string(self) -> String {
        self.0.format("%F %T").to_string()
    }
    fn to_iso_string(self) -> String {
        if self.0.timestamp_subsec_millis() == 0 {
            self.0.to_rfc3339_opts(SecondsFormat::Secs, true)
        } else {
            self.0.to_rfc3339_opts(SecondsFormat::Millis, true)
        }
    }
    pub(crate) fn from_iso_string(datetime_str: &str) -> Result<Self, anyhow::Error> {
        let dt = DateTime::parse_from_rfc3339(datetime_str)?;
        // println!("{datetime_str} -> {dt:?}");
        Ok(Self::new(dt))
    }
}
impl From<DateTime<FixedOffset>> for QxDateTime {
    fn from(value: DateTime<FixedOffset>) -> Self {
        Self::new(value)
    }
}
impl<DB: sqlx::Database> sqlx::Type<DB> for QxDateTime
where
    str: sqlx::Type<DB>,
{
    fn type_info() -> <DB as sqlx::Database>::TypeInfo {
        // TEXT columns only
        <&str as sqlx::Type<DB>>::type_info()
    }
}
impl<'r, DB: sqlx::Database> sqlx::Decode<'r, DB> for QxDateTime
where
    &'r str: sqlx::Decode<'r, DB>,
{
    fn decode(value: <DB as sqlx::Database>::ValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let value = <&str as sqlx::Decode<DB>>::decode(value)?;
        let res = QxDateTime::from_iso_string(value);
        // println!("DECODE: {}, res: {:?}", value, res);
        Ok(res?)
    }
}

#[test]
fn test_parse_qxdatetime() {
    for (dtstr, dtstr2) in &[
        ("1970-03-05 14:32:45+00:00", "1970-03-05T14:32:45Z"),
        ("2025-03-05T14:32:45Z", "2025-03-05T14:32:45Z"),
        ("2025-03-05 14:32:45+10:00", "2025-03-05T14:32:45+10:00"),
        ("2025-03-05T14:32:45-01:30", "2025-03-05T14:32:45-01:30"),
        ("2025-03-17T20:45:38.565293063+01:00", "2025-03-17T20:45:38.565+01:00"),
        ("2025-03-17T21:27:04.095+01:00", "2025-03-17T21:27:04.095+01:00")
    ] {
        let dt = QxDateTime::from_iso_string(dtstr)
            .map_err(|e| println!("parse {dtstr} error: {e}")).unwrap();
        // println!("{} -> {:?}", dtstr, dt.0);
        assert_eq!(&dt.to_iso_string(), dtstr2)
    }
}
pub(crate) fn obtime(sec_since_midnight: i64) -> String {
    let sec = sec_since_midnight % 60;
    let min = sec_since_midnight / 60;
    format!("{min}:{sec:0>2}")
}
pub(crate) fn obtimems(msec_since_midnight: i64) -> String {
    let msec = msec_since_midnight % 1000;
    let sec = msec_since_midnight / 1000;
    let min = sec / 60;
    let sec = sec % 60;
    format!("{min}:{sec:0>2}.{msec:0>3}")
}
pub(crate) fn dtstr(iso_date_str: Option<&str>) -> String {
    let Some(s) = iso_date_str else {
        return "---".to_string()
    };
    if let Ok(dt) = QxDateTime::from_iso_string(s) {
        dt.to_display_string()
        
    } else {
        s.to_string()
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
