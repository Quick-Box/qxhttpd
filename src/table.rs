use std::cmp::min;
use std::fs::File;
use std::{fs, io};
use std::io::{BufRead, BufReader, Read, Write};
use std::marker::PhantomData;
use std::path::{PathBuf};
use rocket::serde::json::serde_json;
use serde::{Serialize};
use serde::de::DeserializeOwned;

#[derive(Debug)]
pub(crate) struct Table<T: Serialize + DeserializeOwned> {
    path: PathBuf,
    index: Vec<usize>,
    _marker: PhantomData<T>,
}
impl<T: Serialize + DeserializeOwned> Table<T> {
    pub fn new(path: &PathBuf) -> crate::Result<Self> {
        if fs::metadata(path).is_err() {
            File::create_new(path)?;
        }
        let mut file = BufReader::new(File::open(path)?);
        let mut line = String::new();
        let mut index = vec![];
        let mut offset = 0_usize;
        loop {
            line.clear();
            let n = file.read_line(&mut line)?;
            if n == 0 {
                break;
            }
            if n == 1 {
                // invalid record, empty line
            } else { 
                index.push(offset); 
            }
            offset += n;
        }
        Ok(Self { path: path.clone(), index, _marker: Default::default() })
    }
    pub fn add_record(&mut self, rec: &T) -> io::Result<usize> {
        let json = serde_json::to_string(rec)?;
        let mut file = File::options().append(true).open(&self.path)?;
        let offset = file.metadata()?.len() as usize;
        self.index.push(offset);
        file.write(json.as_bytes())?;
        file.write(&['\n' as u8])?;
        Ok(offset)
    }
    pub fn get_records(&self, index: usize, limit: Option<usize>) -> io::Result<Vec<T>> {
        let file = File::open(&self.path)?;
        let file_size = file.metadata()?.len() as usize;
        
        let mut res: Vec<T> = vec![];
        
        let mut buff: Vec<u8> = vec![];
        let mut reader = BufReader::new(file);
        let index2 = min(self.index.len(), limit.map(|u| index + u).unwrap_or(self.index.len()));
        for (ix, offset) in self.index[index .. index2].iter().enumerate() {
            let offset2 = if ix == index2 - 1 {
                file_size
            } else { 
                self.index[ix + 1]
            };
            let len = offset2 - offset;
            if buff.len() < len {
                buff.resize(len, 0);
            }
            let n = reader.read(&mut buff[..len])?;
            assert_eq!(n, len);
            let rec = serde_json::from_slice(&buff[..len])?;
            res.push(rec);
        }
        Ok(res)
    }
}
