use anyhow::anyhow;
use chrono::{DateTime, FixedOffset, MappedLocalTime, NaiveDateTime, SecondsFormat, TimeDelta};
use rocket::serde::{Deserialize, Serialize};
use sqlx::{Encode, Sqlite};
use sqlx::sqlite::SqliteArgumentValue;

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone, Copy)]
pub struct QxDateTime(pub DateTime<FixedOffset>);
impl QxDateTime {
    pub fn now() -> Self {
        Self::from_fixed_offset(chrono::Local::now().fixed_offset())
    }
    pub fn from_fixed_offset(datetime: DateTime<FixedOffset>) -> QxDateTime {
        let millis = datetime.timestamp_subsec_millis();
        let nanos = datetime.timestamp_subsec_nanos() - millis * 1_000_000;
        if let Some(dt) = datetime.checked_sub_signed(TimeDelta::nanoseconds(nanos as i64)) {
            QxDateTime(dt)
        } else {
            QxDateTime(datetime)
        }
    }
    pub fn trimmed_to_sec(&self) -> Self {
        let nanos = self.0.timestamp_subsec_nanos();
        if let Some(dt) = self.0.checked_sub_signed(TimeDelta::nanoseconds(nanos as i64)) {
            QxDateTime(dt)
        } else {
            *self
        }
    }
    pub fn from_local_timezone(local_dt: NaiveDateTime, offset: &FixedOffset) -> Option<QxDateTime> {
        let fixed_offset = FixedOffset::east_opt(offset.local_minus_utc())?;
        match local_dt.and_local_timezone(fixed_offset) {
            MappedLocalTime::Single(dt) => {Some(QxDateTime(dt))}
            MappedLocalTime::Ambiguous(_, _) => None,
            MappedLocalTime::None => None,
        }
    }
    fn to_display_string(self) -> String {
        self.0.format("%F %T").to_string()
    }
    pub(crate) fn to_iso_string(self) -> String {
        if self.0.timestamp_subsec_millis() == 0 {
            self.0.to_rfc3339_opts(SecondsFormat::Secs, true)
        } else {
            self.0.to_rfc3339_opts(SecondsFormat::Millis, true)
        }
    }
    pub(crate) fn parse_from_iso(datetime_str: &str) -> Result<Self, anyhow::Error> {
        let dt = DateTime::parse_from_rfc3339(datetime_str)?;
        // println!("{datetime_str} -> {dt:?}");
        Ok(Self::from_fixed_offset(dt))
    }
    pub(crate) fn parse_from_string(datetime_str: &str, local_time_offset: Option<&FixedOffset>) -> Result<Self, anyhow::Error> {
        // ISO 8601 / RFC 3339 date & time format, https://docs.rs/chrono/latest/chrono/format/strftime/index.html
        for format in [
            "%Y-%m-%dT%H:%M:%S%.f%:z",
            "%Y-%m-%d %H:%M:%S%.f%:z",
        ] {
            if let Ok(dt) = DateTime::parse_from_str(datetime_str, format) {
                return Ok(Self::from_fixed_offset(dt));
            }
        }
        if let Some(local_offset) = local_time_offset {
            for format in [
                "%Y-%m-%dT%H:%M:%S%.f",
                "%Y-%m-%d %H:%M:%S%.f",
            ] {
                if let Ok(dt) = NaiveDateTime::parse_from_str(datetime_str, format) {
                    if let Some(dt) = Self::from_local_timezone(dt, local_offset) {
                        return Ok(dt);
                    }
                }
            }
        }
        Err(anyhow!("Invalid datetime: {}", datetime_str))
    }
    pub fn msec_since(&self, since: &Option<QxDateTime>) -> Option<i64> {
        since.map(|since| self.0.signed_duration_since(since.0).num_milliseconds())
    }
    pub fn msec_since_until(since: &Option<QxDateTime>, until: &Option<QxDateTime>) -> Option<i64> {
        until.and_then(|until| until.msec_since(since))
    }
}

impl From<DateTime<FixedOffset>> for QxDateTime {
    fn from(value: DateTime<FixedOffset>) -> Self {
        Self::from_fixed_offset(value)
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
        let res = QxDateTime::parse_from_iso(value);
        // println!("DECODE: {}, res: {:?}", value, res);
        Ok(res?)
    }
}
impl<'r> Encode<'r, Sqlite> for QxDateTime {
    fn encode_by_ref(&self, buf: &mut Vec<SqliteArgumentValue<'r>>) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        <DateTime<FixedOffset> as Encode<Sqlite>>::encode(self.0, buf)
    }
}
#[test]
fn test_trimmed_to_sec() {
    let dt = QxDateTime::now().trimmed_to_sec();
    assert_eq!(dt.0.timestamp_subsec_nanos(), 0);
}

#[test]
fn test_from_local_timezone() {
    let loc_dt = chrono::Local::now();
    let fx_dt = chrono::Local::now().fixed_offset();
    let qx_dt = QxDateTime::from_local_timezone(loc_dt.naive_local(), fx_dt.offset()).unwrap();
    assert_eq!(qx_dt.0.timestamp(), loc_dt.timestamp());
}
#[test]
fn test_parse_from_string() {
    let correct_dt = QxDateTime::parse_from_iso("2025-03-23T13:32:44+01:00").unwrap();
    let correct_dt_msec = QxDateTime::parse_from_iso("2025-03-23T13:32:44.123+01:00").unwrap();
    let local_offset = correct_dt.0.offset();
    for (dtstr, dt) in [
        ("2025-03-23T13:32:44+01:00", correct_dt),
        ("2025-03-23 13:32:44.123+01:00", correct_dt_msec),
        ("2025-03-23T13:32:44", correct_dt),
        ("2025-03-23T13:32:44.123", correct_dt_msec),
        ("2025-03-23 13:32:44", correct_dt),
        ("2025-03-23 13:32:44.123", correct_dt_msec),
    ] {
        let parsed_dt = QxDateTime::parse_from_string(dtstr, Some(local_offset));
        // println!("{} vs {}", dtstr, dt.to_iso_string());
        assert_eq!(parsed_dt.unwrap(), dt);
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
        let dt = QxDateTime::parse_from_iso(dtstr)
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
    if let Ok(dt) = QxDateTime::parse_from_iso(s) {
        dt.to_display_string()

    } else {
        s.to_string()
    }
}
