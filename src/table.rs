use std::fs::File;
use std::io;
use std::io::BufRead;

struct Table {
    path: String,
    index: Vec<usize>,
}
impl Table {
    fn new(path: &str) -> io::Result<Self> {
        let mut file = io::BufReader::new(File::open(path)?);
        let mut line = String::new();
        let mut index = vec![];
        let mut offset = 0_usize;
        loop {
            line.clear();
            let n = file.read_line(&mut line)?;
            if n == 0 {
                break;
            }
            index.push(offset);
            offset += n;
        }
        Ok(Self { path: path.to_string(), index })
    }
}
