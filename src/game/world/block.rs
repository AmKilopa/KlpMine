#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Block {
    Air,
    Grass,
    Dirt,
}

impl Block {
    pub fn is_solid(self) -> bool {
        !matches!(self, Self::Air)
    }

    pub fn hotbar_tile(self) -> usize {
        match self {
            Self::Air | Self::Dirt => 0,
            Self::Grass => 1,
        }
    }

    pub fn mass(self) -> f32 {
        match self {
            Self::Air => 0.0,
            Self::Grass => 1.15,
            Self::Dirt => 1.25,
        }
    }

    pub fn hardness(self) -> f32 {
        match self {
            Self::Air => 0.0,
            Self::Grass => 0.58,
            Self::Dirt => 0.7,
        }
    }
}
