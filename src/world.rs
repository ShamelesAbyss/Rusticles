use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};

const PARTICLE_KINDS: usize = 8;
const PARTICLE_COUNT_MIN: usize = 500;
const PARTICLE_COUNT_MAX: usize = 1500;

const WALL_MARGIN: f32 = 5.0;
const WALL_PUSH: f32 = 0.075;
const WALL_BOUNCE: f32 = 0.86;
const PHI: f32 = 1.618_034;
const BUCKET_SIZE: f32 = 8.0;

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
    #[serde(default)]
    pub orbit: f32,
    #[serde(default)]
    pub density: f32,
    #[serde(default = "default_drag")]
    pub drag: f32,
    #[serde(default)]
    pub resonance: f32,
    #[serde(default)]
    pub harmonic: f32,
    #[serde(default)]
    pub symmetry: f32,
    #[serde(default)]
    pub pulse: f32,
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
            if rng.gen::<f32>() < 0.012 {
                *cell = Cell::Alive;
            }
        }

        let total = rng.gen_range(PARTICLE_COUNT_MIN..=PARTICLE_COUNT_MAX);

        let weights: Vec<f32> = (0..PARTICLE_KINDS)
            .map(|_| rng.gen_range(0.08..0.38))
            .collect();
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
                vx: rng.gen_range(-0.22..0.22),
                vy: rng.gen_range(-0.22..0.22),
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
                    orbit: 0.0,
                    density: 0.0,
                    drag: 0.96,
                    resonance: 0.0,
                    harmonic: 0.0,
                    symmetry: 0.0,
                    pulse: 0.0,
                };
                PARTICLE_KINDS
            ];
            PARTICLE_KINDS
        ];

        for a in 0..PARTICLE_KINDS {
            for b in 0..PARTICLE_KINDS {
                let self_bias = if a == b {
                    rng.gen_range(-0.15..0.55)
                } else {
                    0.0
                };

                rules[a][b] = Rule {
                    attraction: rng.gen_range(-1.45..1.65) + self_bias,
                    radius: rng.gen_range(5.0..34.0),
                    repel_radius: rng.gen_range(1.2..6.5),
                    orbit: rng.gen_range(-1.45..1.45),
                    density: rng.gen_range(-0.95..1.35),
                    drag: rng.gen_range(0.945..0.988),
                    resonance: rng.gen_range(-1.25..1.25),
                    harmonic: rng.gen_range(0.35..2.75),
                    symmetry: rng.gen_range(3.0f32..12.0f32).round(),
                    pulse: rng.gen_range(0.35..1.85),
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
        }
    }

    pub fn step(&mut self) {
        self.tick += 1;
        self.step_particles();

        if self.tick % 2 == 0 {
            self.step_cells();
        }

        if self.tick % 9 == 0 {
            self.seed_cells_from_particles();
        }

        if self.tick % 240 == 0 {
            self.mutate_rules();
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

    fn mutate_rules(&mut self) {
        for a in 0..self.rules.len() {
            for b in 0..self.rules[a].len() {
                let wave = mutation_wave(self.seed, self.tick, a, b);
                let counter = mutation_wave(self.seed ^ 0xA53A_9E37, self.tick, b, a);

                let rule = &mut self.rules[a][b];

                rule.attraction = (rule.attraction + wave * 0.035).clamp(-1.95, 2.05);
                rule.orbit = (rule.orbit + counter * 0.025).clamp(-1.85, 1.85);
                rule.resonance = (rule.resonance + wave * counter * 0.030).clamp(-1.75, 1.75);
                rule.pulse = (rule.pulse + wave.abs() * 0.010 - 0.004).clamp(0.20, 2.40);
                rule.harmonic = (rule.harmonic + counter * 0.012).clamp(0.25, 3.25);
                rule.radius = (rule.radius + wave * 0.080).clamp(4.0, 38.0);
                rule.repel_radius = (rule.repel_radius + counter * 0.018).clamp(1.0, 7.5);
                rule.drag = (rule.drag + wave * 0.0008).clamp(0.935, 0.992);
            }
        }
    }

    fn step_particles(&mut self) {
        let snapshot = self.particles.clone();
        let pressure_map = build_pressure_map(&snapshot, self.width, self.height);
        let buckets = SpatialBuckets::new(&snapshot, self.width, self.height);

        let max_x = (self.width - 1) as f32;
        let max_y = (self.height - 1) as f32;
        let center_x = max_x * 0.5;
        let center_y = max_y * 0.5;

        for i in 0..self.particles.len() {
            let current = &snapshot[i];
            let mut ax = 0.0;
            let mut ay = 0.0;
            let mut local_density = 0usize;
            let mut drag_accum = 0.0;
            let mut drag_count = 0.0;

            let search_radius = self.max_radius_for_kind(current.kind);
            let nearby = buckets.nearby_indices(current.x, current.y, search_radius);

            for other_index in nearby {
                if other_index == i {
                    continue;
                }

                let other = &snapshot[other_index];
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

                local_density += 1;
                drag_accum += rule.drag;
                drag_count += 1.0;

                let nx = dx / dist;
                let ny = dy / dist;
                let angle = dy.atan2(dx);

                let base_force = if dist < rule.repel_radius {
                    let pressure = 1.0 - dist / rule.repel_radius;
                    -2.20 * pressure
                } else {
                    let t = (dist - rule.repel_radius) / (rule.radius - rule.repel_radius);
                    let bell = 1.0 - (2.0 * t - 1.0).abs();
                    rule.attraction * bell
                };

                let resonance = resonance_force(dist, angle, self.tick, rule);
                let radial_force = base_force + resonance;

                let tangent_x = -ny;
                let tangent_y = nx;
                let orbit_force = rule.orbit * (1.0 - dist / rule.radius).max(0.0);
                let angular_gate = (angle * rule.symmetry).cos();

                ax += nx * radial_force * 0.018;
                ay += ny * radial_force * 0.018;

                ax += tangent_x * orbit_force * angular_gate * 0.014;
                ay += tangent_y * orbit_force * angular_gate * 0.014;
            }

            let density_f = local_density as f32;

            if density_f > 6.0 {
                let pressure = ((density_f - 6.0) / 24.0).min(1.0);
                let away_x = current.x - center_x;
                let away_y = current.y - center_y;
                let len = (away_x * away_x + away_y * away_y).sqrt().max(0.001);

                ax += away_x / len * pressure * 0.018;
                ay += away_y / len * pressure * 0.018;
            }

            let pulse = entropy_pulse(current.x, current.y, self.seed, self.tick, current.kind);
            ax += pulse.0;
            ay += pulse.1;

            apply_wall_pressure(current.x, current.y, max_x, max_y, &mut ax, &mut ay);

            let cx = current.x.round().clamp(0.0, max_x) as usize;
            let cy = current.y.round().clamp(0.0, max_y) as usize;
            let pressure = pressure_map[cy * self.width + cx] as usize;

            match self.cell_at(cx, cy) {
                Cell::Alive | Cell::Born => {
                    ax *= 1.12;
                    ay *= 1.12;
                }
                Cell::Dying => {
                    ax -= current.vx * 0.065;
                    ay -= current.vy * 0.065;
                }
                Cell::Dead => {}
            }

            if pressure >= 8 {
                ax += rand_push(current.x, current.y, self.seed, self.tick) * 0.018;
                ay += rand_push(current.y, current.x, self.seed ^ 0xA53A, self.tick) * 0.018;
            }

            let drag = if drag_count > 0.0 {
                drag_accum / drag_count
            } else {
                0.965
            };

            let mut vx = (current.vx + ax).clamp(-1.08, 1.08) * drag;
            let mut vy = (current.vy + ay).clamp(-1.08, 1.08) * drag;

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

    fn max_radius_for_kind(&self, kind: usize) -> f32 {
        self.rules
            .get(kind)
            .map(|row| row.iter().map(|rule| rule.radius).fold(1.0, f32::max))
            .unwrap_or(1.0)
    }

    fn step_cells(&mut self) {
        let old = self.cells.clone();
        let pressure_map = build_pressure_map(&self.particles, self.width, self.height);
        let mut next = vec![Cell::Dead; self.width * self.height];

        for y in 0..self.height {
            for x in 0..self.width {
                let idx = y * self.width + x;
                let neighbors = live_neighbors(&old, self.width, self.height, x, y);
                let pressure = pressure_map[idx] as usize;

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
        let pressure_map = build_pressure_map(&self.particles, self.width, self.height);

        for p in self.particles.iter() {
            let x = p.x.round().clamp(0.0, (self.width - 1) as f32) as usize;
            let y = p.y.round().clamp(0.0, (self.height - 1) as f32) as usize;
            let idx = y * self.width + x;
            let pressure = pressure_map[idx] as usize;

            if matches!(self.cells[idx], Cell::Dead) && pressure >= 5 {
                self.cells[idx] = Cell::Born;
            }
        }
    }
}

struct SpatialBuckets {
    buckets: Vec<Vec<usize>>,
    cols: usize,
    rows: usize,
}

impl SpatialBuckets {
    fn new(particles: &[Particle], width: usize, height: usize) -> Self {
        let cols = ((width as f32) / BUCKET_SIZE).ceil().max(1.0) as usize;
        let rows = ((height as f32) / BUCKET_SIZE).ceil().max(1.0) as usize;
        let mut buckets = vec![Vec::new(); cols * rows];

        for (idx, p) in particles.iter().enumerate() {
            let bx = ((p.x / BUCKET_SIZE).floor() as usize).min(cols - 1);
            let by = ((p.y / BUCKET_SIZE).floor() as usize).min(rows - 1);
            buckets[by * cols + bx].push(idx);
        }

        Self {
            buckets,
            cols,
            rows,
        }
    }

    fn nearby_indices(&self, x: f32, y: f32, radius: f32) -> Vec<usize> {
        let bx = ((x / BUCKET_SIZE).floor() as isize).clamp(0, (self.cols - 1) as isize);
        let by = ((y / BUCKET_SIZE).floor() as isize).clamp(0, (self.rows - 1) as isize);
        let range = (radius / BUCKET_SIZE).ceil() as isize + 1;

        let mut out = Vec::new();

        for oy in -range..=range {
            for ox in -range..=range {
                let nx = bx + ox;
                let ny = by + oy;

                if nx < 0 || ny < 0 || nx >= self.cols as isize || ny >= self.rows as isize {
                    continue;
                }

                out.extend(
                    self.buckets[ny as usize * self.cols + nx as usize]
                        .iter()
                        .copied(),
                );
            }
        }

        out
    }
}

fn build_pressure_map(particles: &[Particle], width: usize, height: usize) -> Vec<u16> {
    let mut map = vec![0u16; width * height];

    for p in particles.iter() {
        let cx = p.x.round() as isize;
        let cy = p.y.round() as isize;

        for oy in -2..=2 {
            for ox in -2..=2 {
                let x = cx + ox;
                let y = cy + oy;

                if x < 0 || y < 0 || x >= width as isize || y >= height as isize {
                    continue;
                }

                let idx = y as usize * width + x as usize;
                map[idx] = map[idx].saturating_add(1);
            }
        }
    }

    map
}

fn mutation_wave(seed: u64, tick: u64, a: usize, b: usize) -> f32 {
    let t = tick as f32 * 0.004;
    let sa = ((seed >> ((a % 8) * 7)) & 0xff) as f32 * 0.017;
    let sb = ((seed >> ((b % 8) * 5)) & 0xff) as f32 * 0.013;
    let x = (a as f32 + 1.0) * 0.73 + sa;
    let y = (b as f32 + 1.0) * 1.11 + sb;

    ((t + x).sin() * 0.65 + (t * 0.618_034 + y).cos() * 0.35).clamp(-1.0, 1.0)
}

fn default_drag() -> f32 {
    0.965
}

fn resonance_force(dist: f32, angle: f32, tick: u64, rule: &Rule) -> f32 {
    let time = tick as f32 * 0.028 * rule.pulse;
    let decay = (1.0 - dist / rule.radius).max(0.0);
    let golden = (dist * 0.37 * PHI * rule.harmonic + time).sin();
    let prime = (dist * 0.23 * 3.0 + time * 1.17).sin()
        + (dist * 0.17 * 5.0 - time * 0.83).sin() * 0.5
        + (dist * 0.11 * 7.0 + time * 0.41).sin() * 0.25;
    let angular = (angle * rule.symmetry + time * 0.35).cos();

    (golden + prime) * angular * decay * rule.resonance
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

fn entropy_pulse(x: f32, y: f32, seed: u64, tick: u64, kind: usize) -> (f32, f32) {
    let time = tick as f32 * 0.035;
    let k = kind as f32 + 1.0;
    let seed_phase = (seed % 10_000) as f32 * 0.0007;

    let wave_a = ((x * 0.19 * k) + time + seed_phase).sin();
    let wave_b = ((y * 0.17 * k) - time * 1.13 + seed_phase).cos();
    let wave_c = (((x + y) * 0.071 * k) + time * 0.77).sin();
    let wave_d = (((x - y) * 0.053 * k) - time * 1.41).cos();

    let ax = (wave_a + wave_c - wave_d) * 0.0048;
    let ay = (wave_b - wave_c + wave_d) * 0.0048;

    (ax, ay)
}

fn rand_push(x: f32, y: f32, seed: u64, tick: u64) -> f32 {
    let n = ((x as u64).wrapping_mul(73_856_093))
        ^ ((y as u64).wrapping_mul(19_349_663))
        ^ seed
        ^ tick.wrapping_mul(83_492_791);

    let v = (n % 2000) as f32 / 1000.0 - 1.0;
    v.clamp(-1.0, 1.0)
}
