#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Block {
    Air = 0,
    Grass = 1,
    Dirt = 2,
    Stone = 3,
    Sand = 4,
    Log = 5,
    Leaves = 6,
}

impl Block {
    pub fn is_solid(self) -> bool {
        !matches!(self, Self::Air)
    }

    pub fn atlas_index(self) -> usize {
        match self {
            Self::Air | Self::Dirt => 0,
            Self::Grass => 1,
            Self::Stone => 3,
            Self::Sand => 4,
            Self::Log => 5,
            Self::Leaves => 6,
        }
    }

    pub fn mass(self) -> f32 {
        match self {
            Self::Air => 0.0,
            Self::Grass => 1.15,
            Self::Dirt => 1.25,
            Self::Stone => 2.4,
            Self::Sand => 1.45,
            Self::Log => 1.8,
            Self::Leaves => 0.25,
        }
    }

    pub fn hardness(self) -> f32 {
        match self {
            Self::Air => 0.0,
            Self::Grass => 0.58,
            Self::Dirt => 0.7,
            Self::Stone => 1.35,
            Self::Sand => 0.45,
            Self::Log => 1.15,
            Self::Leaves => 0.25,
        }
    }

    pub fn falls(self) -> bool {
        matches!(self, Self::Sand)
    }

    pub fn emitted_light(self) -> u8 {
        0
    }
}
