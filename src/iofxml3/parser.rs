use crate::iofxml3::structs::StartList;
use quick_xml::de::from_reader;

// thanks to https://github.com/Thomblin/xml_schema_generator
pub fn parse_startlist(data: &[u8]) -> anyhow::Result<StartList> {
    let startlist: StartList = from_reader(data)?;
    Ok(startlist)
}

#[cfg(test)]
mod test {
    use super::*;
    use std::fs::OpenOptions;
    use std::io::BufReader;
    use crate::event::START_LIST_IOFXML3_FILE;
    use quick_xml::de::from_reader;
    
    #[test]
    fn parse_start_list() {
        let file = OpenOptions::new().read(true).open(format!("tests/{START_LIST_IOFXML3_FILE}")).unwrap();
        let reader = BufReader::new(file);
        let startlist: StartList = from_reader(reader).unwrap();
        assert_eq!(&startlist.event.name, "Mistrovství oblasti na krátké trati");
        assert!(!startlist.class_start.is_empty());
    }
}