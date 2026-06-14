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
}
