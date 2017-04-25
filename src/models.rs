use chrono::NaiveDate;
use std::fmt::{Display, Formatter, Error};

#[derive(Debug, Queryable, Serialize)]
pub struct Quote {
    pub id: i32,
    pub quote: String,
    pub attrib_name: Option<String>,
    #[serde(skip_serializing)]
    pub attrib_date: Option<NaiveDate>,
    pub deleted: bool,
    pub context: Option<String>,
    pub game_id: Option<i32>,
    pub show_id: Option<i32>,
}

impl Display for Quote {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        write!(f, "#{}: \"{}\"", self.id, self.quote)?;
        if let Some(ref name) = self.attrib_name {
            write!(f, " —{}", name)?;
        }
        if let Some(ref context) = self.context {
            write!(f, ", {}", context)?;
        }
        if let Some(ref date) = self.attrib_date {
            write!(f, " [{}]", date)?;
        }
        Ok(())
    }
}