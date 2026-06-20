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
    Water = 7,
}

impl Block {
    pub fn is_solid(self) -> bool {
        !matches!(self, Self::Air | Self::Water)
    }

    pub fn is_visible(self) -> bool {
        !matches!(self, Self::Air)
    }

    pub fn is_fluid(self) -> bool {
        matches!(self, Self::Water)
    }

    pub fn atlas_index(self) -> usize {
        match self {
            Self::Air | Self::Dirt => 0,
            Self::Grass => 1,
            Self::Stone => 3,
            Self::Sand => 4,
            Self::Log => 5,
            Self::Leaves => 6,
            Self::Water => 7,
        }
    }

    pub fn mass(self) -> f32 {
        match self {
            Self::Air => 0.0,
            Self::Grass => 1.1,
            Self::Dirt => 1.0,
            Self::Stone => 3.6,
            Self::Sand => 1.2,
            Self::Log => 4.0,
            Self::Leaves => 0.12,
            Self::Water => 0.0,
        }
    }

    pub fn drop_item(self) -> Option<Self> {
        match self {
            Self::Grass => Some(Self::Dirt),
            Self::Leaves => None,
            Self::Stone => None,
            block => Some(block),
        }
    }

    pub fn hardness(self) -> f32 {
        match self {
            Self::Air => 0.0,
            Self::Grass => 0.90,
            Self::Dirt => 0.75,
            Self::Stone => 7.50,
            Self::Sand => 0.75,
            Self::Log => 3.00,
            Self::Leaves => 0.25,
            Self::Water => 0.0,
        }
    }

    pub fn falls(self) -> bool {
        matches!(self, Self::Sand)
    }

    pub fn emitted_light(self) -> u8 {
        0
    }
}
