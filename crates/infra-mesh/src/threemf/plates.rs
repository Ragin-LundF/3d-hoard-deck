//! Bambu Studio / Orca Slicer plate assignment from `Metadata/model_settings.config`.

use std::collections::HashMap;

use quick_xml::Reader;
use quick_xml::events::Event;

use super::xml::{attribute, name};

/// Build item object id → plate number. Empty when the file has no plates (other slicers: one plate).
pub(crate) fn plate_of_objects(config: &str) -> HashMap<u32, u32> {
    let mut reader = Reader::from_str(config);
    let mut assignment = HashMap::new();
    let mut plate: Option<u32> = None;
    let mut in_plate = false;
    let mut pending: Vec<u32> = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(element) | Event::Empty(element)) => match name(&element).as_str() {
                "plate" => {
                    in_plate = true;
                    plate = None;
                    pending.clear();
                }
                "metadata" if in_plate => {
                    let value = attribute(&element, "value").and_then(|value| value.trim().parse().ok());
                    match (attribute(&element, "key").as_deref(), value) {
                        (Some("plater_id"), Some(number)) => plate = Some(number),
                        (Some("object_id"), Some(object)) => pending.push(object),
                        _ => {}
                    }
                }
                _ => {}
            },
            Ok(Event::End(element)) if element.local_name().as_ref() == "plate" => {
                // `plater_id` may come before or after the instances, so assign when the plate closes.
                if let Some(number) = plate {
                    assignment.extend(pending.drain(..).map(|object| (object, number)));
                }
                in_plate = false;
            }
            Ok(Event::Eof) | Err(_) => return assignment,
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_objects_to_their_plates() {
        let config = r#"<config>
          <object id="2"><metadata key="name" value="Dragon"/></object>
          <plate><metadata key="plater_id" value="1"/>
            <model_instance><metadata key="object_id" value="2"/><metadata key="instance_id" value="0"/></model_instance></plate>
          <plate><model_instance><metadata key="object_id" value="5"/></model_instance><metadata key="plater_id" value="2"/></plate>
        </config>"#;
        let plates = plate_of_objects(config);
        assert_eq!((plates.get(&2), plates.get(&5)), (Some(&1), Some(&2)));
    }
}
