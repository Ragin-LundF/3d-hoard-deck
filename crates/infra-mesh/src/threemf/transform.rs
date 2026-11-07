/// A 3MF affine transform: `m00 m01 m02 m10 m11 m12 m20 m21 m22 m30 m31 m32`, applied to row vectors
/// (`x' = x·m00 + y·m10 + z·m20 + m30`), as the 3MF core specification defines.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Transform([f32; 12]);

impl Transform {
    pub(crate) const IDENTITY: Self = Self([1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0]);

    /// Missing or malformed values fall back to the identity, like slicers do.
    pub(crate) fn parse(text: Option<&str>) -> Self {
        let Some(text) = text else { return Self::IDENTITY };
        let values: Vec<f32> = text.split_whitespace().filter_map(|value| value.parse().ok()).collect();
        values.try_into().map_or(Self::IDENTITY, Self)
    }

    pub(crate) fn apply(&self, [x, y, z]: [f32; 3]) -> [f32; 3] {
        let m = &self.0;
        [
            x * m[0] + y * m[3] + z * m[6] + m[9],
            x * m[1] + y * m[4] + z * m[7] + m[10],
            x * m[2] + y * m[5] + z * m[8] + m[11],
        ]
    }

    /// `self` first, then `outer`.
    pub(crate) fn then(&self, outer: &Self) -> Self {
        let (a, b) = (&self.0, &outer.0);
        let mut m = [0.0; 12];
        for row in 0..4 {
            for column in 0..3 {
                let linear = (0..3).map(|k| a[row * 3 + k] * b[k * 3 + column]).sum::<f32>();
                m[row * 3 + column] = linear + if row == 3 { b[9 + column] } else { 0.0 };
            }
        }
        Self(m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translation_and_composition_follow_the_row_vector_convention() {
        let shift = Transform::parse(Some("1 0 0 0 1 0 0 0 1 10 20 30"));
        assert_eq!(shift.apply([1.0, 1.0, 1.0]), [11.0, 21.0, 31.0]);
        let quarter_turn = Transform::parse(Some("0 1 0 -1 0 0 0 0 1 0 0 0"));
        assert_eq!(quarter_turn.apply([1.0, 0.0, 0.0]), [0.0, 1.0, 0.0]);
        assert_eq!(quarter_turn.then(&shift).apply([1.0, 0.0, 0.0]), [10.0, 21.0, 30.0], "turn, then shift");
        assert_eq!(Transform::parse(Some("garbage")), Transform::IDENTITY);
    }
}
