use std::backtrace::Backtrace;
use std::io::{Cursor, Read};
use anyhow::anyhow;
use base64::Engine;
use image::ImageFormat;
use rocket::http::Status;
use rocket::response::status::Custom;
use serde::de::DeserializeOwned;
use serde_json::Value;

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
pub(crate) fn empty_string_to_none<T: Into<String>>(input: T) -> Option<String> {
    let s = input.into();
    if s.trim().is_empty() {
        None
    } else {
        Some(s)
    }
}

pub(crate) fn zero_to_none<T>(value: T) -> Option<T>
where
    T: PartialEq + Copy + From<u8>,
{
    if value == T::from(0u8) {
        None
    } else {
        Some(value)
    }
}

pub(crate) fn create_qrc(data: &[u8]) -> anyhow::Result<String> {
    let code = qrcode::QrCode::new(data)?;
    // Render the bits into an image.
    let image = code.render::<::image::LumaA<u8>>().build();
    let mut buffer: Vec<u8> = Vec::new();
    let mut cursor = Cursor::new(&mut buffer);
    image.write_to(&mut cursor, ImageFormat::Png)?;
    // Encode the image buffer to base64
    Ok(base64::engine::general_purpose::STANDARD.encode(&buffer))
}

pub fn from_csv_json<T: DeserializeOwned>(data: Vec<Vec<Value>>) -> anyhow::Result<Vec<T>> {
    if data.is_empty() {
        return Ok(vec![]);
    }

    let headers = &data[0];
    let mut result = Vec::new();

    for row in data.iter().skip(1) {
        let mut map = serde_json::Map::new();

        for (key, value) in headers.iter().zip(row.iter()) {
            if let Value::String(k) = key {
                map.insert(k.clone(), value.clone());
            } else {
                return Err(anyhow!("Header must be list of strings"));
            }
        }
        info!("converting {:#?}", map);
        let obj: T = serde_json::from_value(Value::Object(map))?;
        result.push(obj);
    }
    Ok(result)
}