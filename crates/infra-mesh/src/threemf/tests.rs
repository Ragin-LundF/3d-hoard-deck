use std::io::Write;

use zip::write::SimpleFileOptions;

use super::*;

const TRIANGLE_OBJECT: &str = r#"<model><resources><object id="1"><mesh>
  <vertices><vertex x="0" y="0" z="-5"/><vertex x="10" y="0" z="-5"/><vertex x="0" y="10" z="5"/></vertices>
  <triangles><triangle v1="0" v2="1" v3="2"/></triangles></mesh></object></resources></model>"#;

fn write_3mf(dir: &Path, parts: &[(&str, &str)]) -> std::path::PathBuf {
    let path = dir.join("plates.3mf");
    let mut zip = zip::ZipWriter::new(File::create(&path).expect("create"));
    for (name, content) in parts {
        zip.start_file(*name, SimpleFileOptions::default()).expect("start");
        zip.write_all(content.as_bytes()).expect("write");
    }
    zip.finish().expect("finish");
    path
}

fn bambu_root(items: &str) -> String {
    format!(
        r#"<model xmlns:p="http://schemas.microsoft.com/3dmanufacturing/production/2015/06"><resources>
        <object id="2"><components><component p:path="/3D/Objects/object_1.model" objectid="1"/></components></object>
        <object id="3"><components><component p:path="/3D/Objects/object_1.model" objectid="1"/></components></object>
        </resources><build>{items}</build></model>"#
    )
}

const PLATES: &str = r#"<config>
  <plate><metadata key="plater_id" value="1"/><model_instance><metadata key="object_id" value="2"/></model_instance></plate>
  <plate><metadata key="plater_id" value="2"/><model_instance><metadata key="object_id" value="3"/></model_instance></plate>
</config>"#;

fn z_values(plate: &Plate) -> Vec<f32> {
    plate.positions.iter().skip(2).step_by(3).copied().collect()
}

#[test]
fn bambu_plates_are_separated_and_every_object_rests_on_its_bed() {
    let dir = tempfile::tempdir().expect("temp dir");
    // Bambu places plate 2's objects far away in world space and lifts them with the item transform.
    let root = bambu_root(
        r#"<item objectid="2" transform="1 0 0 0 1 0 0 0 1 128 128 5"/><item objectid="3" transform="1 0 0 0 1 0 0 0 1 435 128 40"/>"#,
    );
    let file = write_3mf(
        dir.path(),
        &[
            ("3D/3dmodel.model", &root),
            ("3D/Objects/object_1.model", TRIANGLE_OBJECT),
            ("Metadata/model_settings.config", PLATES),
        ],
    );
    let plates = read_plates(&file).expect("read plates");
    assert_eq!(plates.iter().map(|plate| plate.number).collect::<Vec<_>>(), vec![1, 2]);
    for plate in &plates {
        assert_eq!(plate.positions.len(), 9, "one triangle per plate");
        let lowest = z_values(plate).into_iter().fold(f32::INFINITY, f32::min);
        assert!(lowest.abs() < 1e-4, "plate {} floats at {lowest}", plate.number);
        let xs: Vec<f32> = plate.positions.iter().step_by(3).copied().collect();
        let (low, high) =
            (xs.iter().copied().fold(f32::INFINITY, f32::min), xs.iter().copied().fold(f32::NEG_INFINITY, f32::max));
        assert!(low.midpoint(high).abs() < 1e-4, "plate {} is centered", plate.number);
    }
}

#[test]
fn files_without_plate_information_are_one_plate() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = TRIANGLE_OBJECT.replace("</resources>", r#"</resources><build><item objectid="1"/></build>"#);
    let plates = read_plates(&write_3mf(dir.path(), &[("3D/3dmodel.model", &root)])).expect("read");
    assert_eq!(plates.len(), 1);
    assert_eq!(plates[0].number, 1);
}

#[test]
fn triangles_with_broken_indexes_are_skipped_whole() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = TRIANGLE_OBJECT
        .replace(
            r#"<triangle v1="0" v2="1" v3="2"/>"#,
            r#"<triangle v1="0" v2="1" v3="9"/><triangle v1="0" v2="1" v3="2"/>"#,
        )
        .replace("</resources>", r#"</resources><build><item objectid="1"/></build>"#);
    let plates = read_plates(&write_3mf(dir.path(), &[("3D/3dmodel.model", &root)])).expect("read");
    assert_eq!(plates[0].positions.len(), 9, "only the valid triangle remains");
}

#[test]
fn broken_archives_fail_with_a_message() {
    let dir = tempfile::tempdir().expect("temp dir");
    let file = dir.path().join("broken.3mf");
    std::fs::write(&file, b"not a zip").expect("write");
    assert!(read_plates(&file).is_err());
}
