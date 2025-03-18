use chrono::{DateTime, FixedOffset, MappedLocalTime, NaiveDateTime, SecondsFormat, TimeDelta};
use rocket::serde::{Deserialize, Serialize};

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
    pub(crate) fn from_iso_string(datetime_str: &str) -> Result<Self, anyhow::Error> {
        let dt = DateTime::parse_from_rfc3339(datetime_str)?;
        // println!("{datetime_str} -> {dt:?}");
        Ok(Self::from_fixed_offset(dt))
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
        let res = QxDateTime::from_iso_string(value);
        // println!("DECODE: {}, res: {:?}", value, res);
        Ok(res?)
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
