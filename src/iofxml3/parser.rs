use std::collections::BTreeMap;
use crate::iofxml3::structs::StartList;
use quick_xml::de::from_reader;
use crate::event::EventId;
use crate::qxdatetime::QxDateTime;
use crate::runs::{ClassesRecord, RunsRecord};
// thanks to https://github.com/Thomblin/xml_schema_generator
// xml_schema_generator --derive "Serialize, Deserialize, Debug" ~/p/qxhttpd/tests/startlist-iof3.xml > ~/p/qxhttpd/src/iofxml3/structs.rs 

pub fn parse_startlist(data: &[u8]) -> anyhow::Result<StartList> {
    let startlist: StartList = from_reader(data)?;
    Ok(startlist)
}

pub async fn parse_startlist_xml_data(event_id: EventId, data: Vec<u8>) -> anyhow::Result<(Option<QxDateTime>, Vec<ClassesRecord>, Vec<RunsRecord>)> {
    let stlist = parse_startlist(&data)?;
    let start_00_str = format!("{}T{}", stlist.event.start_time.date, stlist.event.start_time.time);
    let mut fixed_offset = None;
    let mut runs = Vec::new();
    let mut classes = BTreeMap::new();
    for cs in &stlist.class_start {
        let class_name = cs.class.name.clone();
        if !classes.contains_key(&class_name) {
            let classrec = ClassesRecord {
                id: 0,
                event_id,
                name: class_name.clone(),
                length: cs.course.length.parse::<i64>().unwrap_or(0),
                climb: cs.course.climb.parse::<i64>().unwrap_or(0),
                control_count: cs.course.number_of_controls.parse::<i64>().unwrap_or(0),
                start_time: Default::default(),
                interval: 0,
                start_slot_count: 0,
            };
            classes.insert(class_name.clone(), classrec);
        }
        for ps in &cs.person_start {
            let mut runsrec = RunsRecord { class_name: class_name.clone(), ..Default::default() };
            let person = &ps.person;
            let name = &person.name;
            runsrec.first_name = name.given.to_string();
            runsrec.last_name = name.family.to_string();
            runsrec.registration = person.id.iter().find(|id| id.id_type == "CZE")
                .and_then(|id| id.text.clone()).unwrap_or_default();
            let Some(run_id) = person.id.iter().find(|id| id.id_type == "QuickEvent") else {
                warn!("QuickEvent ID not found in person_start {:?}", ps);
                continue;
            };
            let Some(run_id) = run_id.text.as_ref().and_then(|id| id.parse::<i64>().ok()) else {
                // still can be a vacant
                if !runsrec.registration.is_empty() {
                    warn!("QuickEvent ID value invalid: {:?}", ps);
                }
                continue;
            };
            runsrec.run_id = run_id;
            let Ok(start_time) = QxDateTime::parse_from_iso(&ps.start.start_time) else {
                warn!("Start time value invalid: {:?}", ps);
                continue;
            };
            if fixed_offset.is_none() {
                fixed_offset = Some(*start_time.0.offset());
            }
            runsrec.start_time = Some(start_time);
            let si = &ps.start.control_card.as_ref().and_then(|si| si.parse::<i64>().ok()).unwrap_or_default();
            runsrec.si_id = *si;
            runs.push(runsrec);
        }
    }
    let classes = classes.into_values().collect();
    let start00 = QxDateTime::parse_from_string(&start_00_str, fixed_offset.as_ref()).ok();
    Ok((start00, classes, runs))
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
        assert!(!startlist.class_start.first().unwrap().person_start.is_empty());
    }
}