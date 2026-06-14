#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Block {
    Air,
    Grass,
    Dirt,
    Stone,
    Sand,
}

impl Block {
    pub fn is_solid(self) -> bool {
        !matches!(self, Self::Air)
    }

    pub fn hotbar_tile(self) -> usize {
        match self {
            Self::Air | Self::Dirt => 0,
            Self::Grass => 1,
            Self::Stone => 3,
            Self::Sand => 4,
        }
    }

    pub fn mass(self) -> f32 {
        match self {
            Self::Air => 0.0,
            Self::Grass => 1.15,
            Self::Dirt => 1.25,
            Self::Stone => 2.4,
            Self::Sand => 1.45,
        }
    }

    pub fn hardness(self) -> f32 {
        match self {
            Self::Air => 0.0,
            Self::Grass => 0.58,
            Self::Dirt => 0.7,
            Self::Stone => 1.35,
            Self::Sand => 0.45,
        }
    }

    pub fn falls(self) -> bool {
        matches!(self, Self::Sand)
    }
}
