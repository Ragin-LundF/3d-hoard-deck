use crate::Plate;

/// Little-endian binary for the viewer: `u32 plates`, then per plate `u32 number`, `u32 floats`, `f32 × floats`
/// (x, y, z per vertex, three vertices per triangle). No JSON: a plate can hold millions of floats.
#[must_use]
pub fn encode_plates(plates: &[Plate]) -> Vec<u8> {
    let floats: usize = plates.iter().map(|plate| plate.positions.len()).sum();
    let mut bytes = Vec::with_capacity(4 + plates.len() * 8 + floats * 4);
    push_u32(&mut bytes, plates.len());
    for plate in plates {
        bytes.extend_from_slice(&plate.number.to_le_bytes());
        push_u32(&mut bytes, plate.positions.len());
        for value in &plate.positions {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    bytes
}

fn push_u32(bytes: &mut Vec<u8>, value: usize) {
    bytes.extend_from_slice(&u32::try_from(value).unwrap_or(u32::MAX).to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_is_count_then_number_length_and_floats() {
        let bytes = encode_plates(&[Plate { number: 2, positions: vec![1.0, 2.0, 3.0] }]);
        assert_eq!(&bytes[0..4], &1_u32.to_le_bytes());
        assert_eq!(&bytes[4..8], &2_u32.to_le_bytes());
        assert_eq!(&bytes[8..12], &3_u32.to_le_bytes());
        assert_eq!(&bytes[12..16], &1.0_f32.to_le_bytes());
        assert_eq!(bytes.len(), 24);
    }
}
