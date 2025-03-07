use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct StartList {
    #[serde(rename = "@createTime")]
    pub create_time: String,
    #[serde(rename = "@creator")]
    pub creator: String,
    #[serde(rename = "@iofVersion")]
    pub iof_version: String,
    #[serde(rename = "@xmlns")]
    pub xmlns: String,
    #[serde(rename = "$text")]
    pub text: Option<String>,
    #[serde(rename = "Event")]
    pub event: Event,
    #[serde(rename = "ClassStart")]
    pub class_start: Vec<ClassStart>,
}

#[derive(Serialize, Deserialize)]
pub struct Event {
    #[serde(rename = "$text")]
    pub text: Option<String>,
    #[serde(rename = "Id")]
    pub id: EventId,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "StartTime")]
    pub start_time: EventStartTime,
    #[serde(rename = "Official")]
    pub official: Vec<Official>,
}

#[derive(Serialize, Deserialize)]
pub struct EventId {
    #[serde(rename = "@type")]
    pub id_type: String,
    #[serde(rename = "$text")]
    pub text: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct EventStartTime {
    #[serde(rename = "$text")]
    pub text: Option<String>,
    #[serde(rename = "Date")]
    pub date: String,
    #[serde(rename = "Time")]
    pub time: String,
}

#[derive(Serialize, Deserialize)]
pub struct Official {
    #[serde(rename = "@type")]
    pub official_type: String,
    #[serde(rename = "$text")]
    pub text: Option<String>,
    #[serde(rename = "Person")]
    pub person: OfficialPerson,
}

#[derive(Serialize, Deserialize)]
pub struct OfficialPerson {
    #[serde(rename = "$text")]
    pub text: Option<String>,
    #[serde(rename = "Name")]
    pub name: OfficialPersonName,
}

#[derive(Serialize, Deserialize)]
pub struct OfficialPersonName {
    #[serde(rename = "$text")]
    pub text: Option<String>,
    #[serde(rename = "Family")]
    pub family: String,
    #[serde(rename = "Given")]
    pub given: String,
}

#[derive(Serialize, Deserialize)]
pub struct ClassStart {
    #[serde(rename = "$text")]
    pub text: Option<String>,
    #[serde(rename = "Class")]
    pub class: Class,
    #[serde(rename = "Course")]
    pub course: Course,
    #[serde(rename = "StartName")]
    pub start_name: String,
    #[serde(rename = "PersonStart")]
    pub person_start: Vec<PersonStart>,
}

#[derive(Serialize, Deserialize)]
pub struct Class {
    #[serde(rename = "$text")]
    pub text: Option<String>,
    #[serde(rename = "Id")]
    pub id: String,
    #[serde(rename = "Name")]
    pub name: String,
}

#[derive(Serialize, Deserialize)]
pub struct Course {
    #[serde(rename = "$text")]
    pub text: Option<String>,
    #[serde(rename = "Length")]
    pub length: String,
    #[serde(rename = "Climb")]
    pub climb: String,
    #[serde(rename = "NumberOfControls")]
    pub number_of_controls: String,
}

#[derive(Serialize, Deserialize)]
pub struct PersonStart {
    #[serde(rename = "$text")]
    pub text: Option<String>,
    #[serde(rename = "Person")]
    pub person: PersonStartPerson,
    #[serde(rename = "Organisation")]
    pub organisation: Organisation,
    #[serde(rename = "Start")]
    pub start: Start,
}

#[derive(Serialize, Deserialize)]
pub struct PersonStartPerson {
    #[serde(rename = "$text")]
    pub text: Option<String>,
    #[serde(rename = "Id")]
    pub id: Vec<PersonId>,
    #[serde(rename = "Name")]
    pub name: PersonStartPersonName,
}

#[derive(Serialize, Deserialize)]
pub struct PersonId {
    #[serde(rename = "@type")]
    pub id_type: String,
    #[serde(rename = "$text")]
    pub text: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct PersonStartPersonName {
    #[serde(rename = "$text")]
    pub text: Option<String>,
    #[serde(rename = "Family")]
    pub family: String,
    #[serde(rename = "Given")]
    pub given: String,
}

#[derive(Serialize, Deserialize)]
pub struct Organisation {
    #[serde(rename = "$text")]
    pub text: Option<String>,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "ShortName")]
    pub short_name: String,
}

#[derive(Serialize, Deserialize)]
pub struct Start {
    #[serde(rename = "$text")]
    pub text: Option<String>,
    #[serde(rename = "StartTime")]
    pub start_time: String,
    #[serde(rename = "ControlCard")]
    pub control_card: Option<String>,
}


