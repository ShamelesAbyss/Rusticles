use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};

const PARTICLE_KINDS: usize = 8;
const PARTICLE_COUNT_MIN: usize = 500;
const PARTICLE_COUNT_MAX: usize = 1500;

const WALL_MARGIN: f32 = 4.0;
const WALL_PUSH: f32 = 0.055;
const WALL_BOUNCE: f32 = 0.82;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Cell {
    Dead,
    Born,
    Alive,
    Dying,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Particle {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub kind: usize,
    pub age: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Rule {
    pub attraction: f32,
    pub radius: f32,
    pub repel_radius: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct World {
    pub seed: u64,
    pub tick: u64,
    pub width: usize,
    pub height: usize,
    pub cells: Vec<Cell>,
    pub particles: Vec<Particle>,
    pub rules: Vec<Vec<Rule>>,
}

impl World {
    pub fn new_random(width: usize, height: usize) -> Self {
        let seed = rand::thread_rng().gen::<u64>();
        Self::from_seed(seed, width, height)
    }

    pub fn from_seed(seed: u64, width: usize, height: usize) -> Self {
        let width = width.max(40);
        let height = height.max(12);

        let mut rng = StdRng::seed_from_u64(seed);
        let mut cells = vec![Cell::Dead; width * height];

        for cell in cells.iter_mut() {
            if rng.gen::<f32>() < 0.015 {
                *cell = Cell::Alive;
            }
        }

        let total = rng.gen_range(PARTICLE_COUNT_MIN..=PARTICLE_COUNT_MAX);

        let weights = [
            rng.gen_range(0.18..0.48),
            rng.gen_range(0.18..0.48),
            rng.gen_range(0.18..0.48),
            rng.gen_range(0.18..0.48),
        ];
        let weight_sum: f32 = weights.iter().sum();

        let mut particles = Vec::with_capacity(total);

        for _ in 0..total {
            let mut roll = rng.gen::<f32>() * weight_sum;
            let mut kind = 0;

            for (idx, weight) in weights.iter().enumerate() {
                if roll <= *weight {
                    kind = idx;
                    break;
                }
                roll -= *weight;
            }

            particles.push(Particle {
                x: rng.gen_range(WALL_MARGIN..(width as f32 - WALL_MARGIN)),
                y: rng.gen_range(WALL_MARGIN..(height as f32 - WALL_MARGIN)),
                vx: rng.gen_range(-0.18..0.18),
                vy: rng.gen_range(-0.18..0.18),
                kind,
                age: 0,
            });
        }

        let mut rules = vec![
            vec![
                Rule {
                    attraction: 0.0,
                    radius: 0.0,
                    repel_radius: 0.0,
                };
                PARTICLE_KINDS
            ];
            PARTICLE_KINDS
        ];

        for a in 0..PARTICLE_KINDS {
            for b in 0..PARTICLE_KINDS {
                let self_bias = if a == b { 0.35 } else { 0.0 };

                rules[a][b] = Rule {
                    attraction: rng.gen_range(-0.75..1.15) + self_bias,
                    radius: rng.gen_range(7.0..22.0),
                    repel_radius: rng.gen_range(1.6..3.8),
                };
            }
        }

        Self {
            seed,
            tick: 0,
            width,
            height,
            cells,
            particles,
            rules,
        }
    }

    pub fn resize_to(&mut self, new_width: usize, new_height: usize) {
        let new_width = new_width.max(40);
        let new_height = new_height.max(12);

        if self.width == new_width && self.height == new_height {
            return;
        }

        let old_width = self.width;
        let old_height = self.height;
        let old_cells = self.cells.clone();

        let mut new_cells = vec![Cell::Dead; new_width * new_height];

        for y in 0..old_height.min(new_height) {
            for x in 0..old_width.min(new_width) {
                new_cells[y * new_width + x] = old_cells[y * old_width + x];
            }
        }

        self.width = new_width;
        self.height = new_height;
        self.cells = new_cells;

        let max_x = (self.width - 1) as f32;
        let max_y = (self.height - 1) as f32;

        for p in self.particles.iter_mut() {
            p.x = p.x.clamp(0.0, max_x);
            p.y = p.y.clamp(0.0, max_y);

            if p.x <= 0.0 || p.x >= max_x {
                p.vx *= -WALL_BOUNCE;
            }

            if p.y <= 0.0 || p.y >= max_y {
                p.vy *= -WALL_BOUNCE;
            }
        }
    }

    pub fn step(&mut self) {
        self.tick += 1;
        self.step_particles();

        if self.tick % 2 == 0 {
            self.step_cells();
        }

        if self.tick % 11 == 0 {
            self.seed_cells_from_particles();
        }
    }

    pub fn kind_count(&self, kind: usize) -> usize {
        self.particles.iter().filter(|p| p.kind == kind).count()
    }

    pub fn cell_at(&self, x: usize, y: usize) -> Cell {
        if x >= self.width || y >= self.height {
            return Cell::Dead;
        }

        self.cells[y * self.width + x]
    }

    fn step_particles(&mut self) {
        let snapshot = self.particles.clone();
        let max_x = (self.width - 1) as f32;
        let max_y = (self.height - 1) as f32;

        for i in 0..self.particles.len() {
            let current = &snapshot[i];
            let mut ax = 0.0;
            let mut ay = 0.0;

            for other in snapshot.iter() {
                let dx = other.x - current.x;
                let dy = other.y - current.y;
                let dist_sq = dx * dx + dy * dy;

                if dist_sq <= 0.0001 {
                    continue;
                }

                let dist = dist_sq.sqrt();
                let rule = &self.rules[current.kind][other.kind];

                if dist > rule.radius {
                    continue;
                }

                let nx = dx / dist;
                let ny = dy / dist;

                let force = if dist < rule.repel_radius {
                    let pressure = 1.0 - dist / rule.repel_radius;
                    -1.65 * pressure
                } else {
                    let t = (dist - rule.repel_radius) / (rule.radius - rule.repel_radius);
                    rule.attraction * (1.0 - (2.0 * t - 1.0).abs())
                };

                ax += nx * force * 0.018;
                ay += ny * force * 0.018;
            }

            apply_wall_pressure(current.x, current.y, max_x, max_y, &mut ax, &mut ay);

            let cx = current.x.round().clamp(0.0, max_x) as usize;
            let cy = current.y.round().clamp(0.0, max_y) as usize;
            let pressure = self.local_particle_pressure(cx, cy);

            match self.cell_at(cx, cy) {
                Cell::Alive | Cell::Born => {
                    ax *= 1.10;
                    ay *= 1.10;
                }
                Cell::Dying => {
                    ax -= current.vx * 0.05;
                    ay -= current.vy * 0.05;
                }
                Cell::Dead => {}
            }

            if pressure >= 8 {
                ax += rand_push(current.x, current.y, self.seed, self.tick) * 0.015;
                ay += rand_push(current.y, current.x, self.seed ^ 0xA53A, self.tick) * 0.015;
            }

            let mut vx = (current.vx + ax).clamp(-0.85, 0.85) * 0.965;
            let mut vy = (current.vy + ay).clamp(-0.85, 0.85) * 0.965;

            let mut x = current.x + vx;
            let mut y = current.y + vy;

            bounce_axis(&mut x, &mut vx, 0.0, max_x);
            bounce_axis(&mut y, &mut vy, 0.0, max_y);

            self.particles[i].x = x;
            self.particles[i].y = y;
            self.particles[i].vx = vx;
            self.particles[i].vy = vy;
            self.particles[i].age += 1;
        }
    }

    fn step_cells(&mut self) {
        let old = self.cells.clone();
        let mut next = vec![Cell::Dead; self.width * self.height];

        for y in 0..self.height {
            for x in 0..self.width {
                let idx = y * self.width + x;
                let neighbors = live_neighbors(&old, self.width, self.height, x, y);
                let pressure = self.local_particle_pressure(x, y);

                next[idx] = match old[idx] {
                    Cell::Dead => {
                        if neighbors == 3 && pressure >= 2 {
                            Cell::Born
                        } else if pressure >= 7 {
                            Cell::Born
                        } else {
                            Cell::Dead
                        }
                    }
                    Cell::Born => Cell::Alive,
                    Cell::Alive => {
                        if neighbors == 2 || neighbors == 3 || pressure >= 4 {
                            Cell::Alive
                        } else {
                            Cell::Dying
                        }
                    }
                    Cell::Dying => Cell::Dead,
                };
            }
        }

        self.cells = next;
    }

    fn seed_cells_from_particles(&mut self) {
        for p in self.particles.iter() {
            let x = p.x.round().clamp(0.0, (self.width - 1) as f32) as usize;
            let y = p.y.round().clamp(0.0, (self.height - 1) as f32) as usize;
            let idx = y * self.width + x;
            let pressure = self.local_particle_pressure(x, y);

            if matches!(self.cells[idx], Cell::Dead) && pressure >= 5 {
                self.cells[idx] = Cell::Born;
            }
        }
    }

    fn local_particle_pressure(&self, x: usize, y: usize) -> usize {
        self.particles
            .iter()
            .filter(|p| {
                let dx = (p.x - x as f32).abs();
                let dy = (p.y - y as f32).abs();
                dx <= 1.75 && dy <= 1.75
            })
            .count()
    }
}

fn apply_wall_pressure(x: f32, y: f32, max_x: f32, max_y: f32, ax: &mut f32, ay: &mut f32) {
    if x < WALL_MARGIN {
        *ax += (WALL_MARGIN - x) * WALL_PUSH;
    }

    if x > max_x - WALL_MARGIN {
        *ax -= (x - (max_x - WALL_MARGIN)) * WALL_PUSH;
    }

    if y < WALL_MARGIN {
        *ay += (WALL_MARGIN - y) * WALL_PUSH;
    }

    if y > max_y - WALL_MARGIN {
        *ay -= (y - (max_y - WALL_MARGIN)) * WALL_PUSH;
    }
}

fn live_neighbors(cells: &[Cell], width: usize, height: usize, x: usize, y: usize) -> usize {
    let mut count = 0;

    for oy in [-1isize, 0, 1] {
        for ox in [-1isize, 0, 1] {
            if ox == 0 && oy == 0 {
                continue;
            }

            let nx = x as isize + ox;
            let ny = y as isize + oy;

            if nx < 0 || ny < 0 || nx >= width as isize || ny >= height as isize {
                continue;
            }

            let nx = nx as usize;
            let ny = ny as usize;

            if matches!(cells[ny * width + nx], Cell::Alive | Cell::Born) {
                count += 1;
            }
        }
    }

    count
}

fn bounce_axis(pos: &mut f32, vel: &mut f32, min: f32, max: f32) {
    if *pos <= min {
        *pos = min;
        *vel = vel.abs() * WALL_BOUNCE;
    } else if *pos >= max {
        *pos = max;
        *vel = -vel.abs() * WALL_BOUNCE;
    }

    *pos = pos.clamp(min, max);
}

fn rand_push(x: f32, y: f32, seed: u64, tick: u64) -> f32 {
    let n = ((x as u64).wrapping_mul(73_856_093))
        ^ ((y as u64).wrapping_mul(19_349_663))
        ^ seed
        ^ tick.wrapping_mul(83_492_791);

    let v = (n % 2000) as f32 / 1000.0 - 1.0;
    v.clamp(-1.0, 1.0)
}
