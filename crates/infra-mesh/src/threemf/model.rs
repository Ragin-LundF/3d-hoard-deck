//! One `.model` part: objects (meshes or components) and build items.

use std::collections::HashMap;

use quick_xml::Reader;
use quick_xml::events::Event;

use super::MeshError;
use super::transform::Transform;
use super::xml::{attribute, name, number};

#[derive(Debug, Default)]
pub(crate) struct Mesh {
    pub vertices: Vec<[f32; 3]>,
    pub triangles: Vec<[usize; 3]>,
}

#[derive(Debug)]
pub(crate) struct Component {
    /// Another model part (production extension), or `None` for this part.
    pub path: Option<String>,
    pub object: u32,
    pub transform: Transform,
}

#[derive(Debug, Default)]
pub(crate) struct Object {
    pub mesh: Mesh,
    pub components: Vec<Component>,
}

#[derive(Debug)]
pub(crate) struct Item {
    pub object: u32,
    pub transform: Transform,
}

#[derive(Debug, Default)]
pub(crate) struct Model {
    pub objects: HashMap<u32, Object>,
    pub build: Vec<Item>,
}

/// Fails once more than `max_triangles` triangles were read, before any instance fan-out.
pub(crate) fn parse_model(xml: &str, max_triangles: usize) -> Result<Model, MeshError> {
    let mut reader = Reader::from_str(xml);
    let mut model = Model::default();
    let mut current: Option<(u32, Object)> = None;
    let mut triangles = 0_usize;
    loop {
        match reader.read_event().map_err(|error| MeshError(format!("3MF model: {error}")))? {
            Event::Start(element) | Event::Empty(element) => match name(&element).as_str() {
                "object" => current = number(&element, "id").map(|id| (id, Object::default())),
                "vertex" => {
                    if let (Some((_, object)), Some(x), Some(y), Some(z)) =
                        (current.as_mut(), number(&element, "x"), number(&element, "y"), number(&element, "z"))
                    {
                        object.mesh.vertices.push([x, y, z]);
                    }
                }
                "triangle" => {
                    triangles += 1;
                    if triangles > max_triangles {
                        return Err(MeshError("this 3MF has too many triangles for the preview yet".to_owned()));
                    }
                    if let (Some((_, object)), Some(a), Some(b), Some(c)) =
                        (current.as_mut(), number(&element, "v1"), number(&element, "v2"), number(&element, "v3"))
                    {
                        object.mesh.triangles.push([a, b, c]);
                    }
                }
                "component" => {
                    if let (Some((_, object)), Some(id)) = (current.as_mut(), number(&element, "objectid")) {
                        let path = attribute(&element, "path").map(|path| path.trim_start_matches('/').to_owned());
                        let transform = Transform::parse(attribute(&element, "transform").as_deref());
                        object.components.push(Component { path, object: id, transform });
                    }
                }
                "item" => {
                    if let Some(id) = number(&element, "objectid") {
                        let transform = Transform::parse(attribute(&element, "transform").as_deref());
                        model.build.push(Item { object: id, transform });
                    }
                }
                _ => {}
            },
            Event::End(element) if element.local_name().as_ref() == "object" => {
                if let Some((id, object)) = current.take() {
                    model.objects.insert(id, object);
                }
            }
            Event::Eof => return Ok(model),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stops_at_the_triangle_limit() {
        let xml = r#"<model><resources><object id="1"><mesh><triangles><triangle v1="0" v2="1" v3="2"/><triangle v1="0" v2="1" v3="2"/></triangles></mesh></object></resources></model>"#;
        assert!(parse_model(xml, 1).is_err());
        assert!(parse_model(xml, 2).is_ok());
    }

    #[test]
    fn reads_meshes_components_and_build_items() {
        let xml = r#"<model xmlns:p="http://schemas.microsoft.com/3dmanufacturing/production/2015/06">
          <resources>
            <object id="1"><mesh><vertices><vertex x="0" y="0" z="0"/><vertex x="1" y="0" z="0"/><vertex x="0" y="1" z="0"/></vertices>
              <triangles><triangle v1="0" v2="1" v3="2"/></triangles></mesh></object>
            <object id="2"><components><component p:path="/3D/Objects/a.model" objectid="7" transform="1 0 0 0 1 0 0 0 1 5 0 0"/></components></object>
          </resources>
          <build><item objectid="2" transform="1 0 0 0 1 0 0 0 1 100 100 0"/></build>
        </model>"#;
        let model = parse_model(xml, 100).expect("valid model");
        assert_eq!(model.objects[&1].mesh.triangles, vec![[0, 1, 2]]);
        let component = &model.objects[&2].components[0];
        assert_eq!((component.path.as_deref(), component.object), (Some("3D/Objects/a.model"), 7));
        assert_eq!(model.build.len(), 1);
        assert_eq!(model.build[0].transform.apply([0.0, 0.0, 0.0]), [100.0, 100.0, 0.0]);
    }
}
