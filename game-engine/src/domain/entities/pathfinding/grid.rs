use crate::infrastructure::ro_formats::gat::RoAltitude;

#[derive(Clone)]
pub struct PathfindingGrid {
    width: u32,
    height: u32,
    walkability: Vec<bool>,
}

impl PathfindingGrid {
    pub fn from_gat(gat: &RoAltitude) -> Self {
        let width = gat.width;
        let height = gat.height;
        let mut walkability = Vec::with_capacity((width * height) as usize);

        for y in 0..height {
            for x in 0..width {
                walkability.push(gat.is_walkable(x as usize, y as usize));
            }
        }

        Self {
            width,
            height,
            walkability,
        }
    }

    pub fn is_walkable(&self, x: u16, y: u16) -> bool {
        if x >= self.width as u16 || y >= self.height as u16 {
            return false;
        }

        let index = (y as usize) * (self.width as usize) + (x as usize);
        self.walkability.get(index).copied().unwrap_or(false)
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }
}
