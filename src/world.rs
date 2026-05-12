use rand::rngs::StdRng;
use rand::{seq::SliceRandom, Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use std::time::Instant;

const PARTICLE_KINDS: usize = 8;
const PARTICLE_COUNT_MIN: usize = 900;
const PARTICLE_COUNT_MAX: usize = 2200;

const WALL_MARGIN: f32 = 7.0;
const WALL_PUSH: f32 = 0.090;
const WALL_BOUNCE: f32 = 0.86;
const PHI: f32 = 1.618_034;
const BUCKET_SIZE: f32 = 8.0;
const MAX_NEIGHBOR_INTERACTIONS: usize = 32;

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
pub struct HabitatField {
    pub attract_strength: f32,
    pub repel_strength: f32,
    pub swirl_strength: f32,
    pub pulse_speed: f32,
    pub wave_x: f32,
    pub wave_y: f32,
    pub diagonal_wave: f32,
    pub phase_a: f32,
    pub phase_b: f32,
    pub symmetry: f32,
    pub turbulence: f32,
    pub center_pull: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct PerfStats {
    pub step_ms: f32,
    pub particle_ms: f32,
    pub cell_ms: f32,
    pub seed_ms: f32,
    pub bucket_count: usize,
    pub max_bucket_load: usize,
    pub avg_bucket_load: f32,
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
    #[serde(default = "default_habitat")]
    pub habitat: HabitatField,

    #[serde(skip, default)]
    runtime: RuntimeBuffers,

    #[serde(skip, default)]
    cached_kind_counts: [usize; PARTICLE_KINDS],

    #[serde(skip, default)]
    perf: PerfStats,
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
        let habitat = HabitatField::random(&mut rng);

        let mut cells = vec![Cell::Dead; width * height];
        for cell in cells.iter_mut() {
            if rng.gen::<f32>() < 0.012 {
                *cell = Cell::Alive;
            }
        }

        let total = rng.gen_range(PARTICLE_COUNT_MIN..=PARTICLE_COUNT_MAX);
        let active_count = rng.gen_range(4..=PARTICLE_KINDS);
        let mut active_kinds: Vec<usize> = (0..PARTICLE_KINDS).collect();
        active_kinds.shuffle(&mut rng);
        active_kinds.truncate(active_count);

        let weights: Vec<f32> = active_kinds
            .iter()
            .map(|_| rng.gen_range(0.08..0.38))
            .collect();
        let weight_sum: f32 = weights.iter().sum();

        let mut particles = Vec::with_capacity(total);
        for _ in 0..total {
            let mut roll = rng.gen::<f32>() * weight_sum;
            let mut kind = 0;

            for (idx, weight) in weights.iter().enumerate() {
                if roll <= *weight {
                    kind = active_kinds[idx];
                    break;
                }
                roll -= *weight;
            }

            particles.push(Particle {
                x: rng.gen_range(
                    WALL_MARGIN.min(width as f32 * 0.25)
                        ..(width as f32 - WALL_MARGIN).max(width as f32 * 0.75),
                ),
                y: rng.gen_range(
                    WALL_MARGIN.min(height as f32 * 0.25)
                        ..(height as f32 - WALL_MARGIN).max(height as f32 * 0.75),
                ),
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
                    rng.gen_range(-0.25..0.25)
                } else {
                    0.0
                };

                rules[a][b] = Rule {
                    attraction: rng.gen_range(-1.35..1.35) + self_bias,
                    radius: rng.gen_range(5.0..34.0),
                    repel_radius: rng.gen_range(2.0..7.5),
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

        let mut world = Self {
            seed,
            tick: 0,
            width,
            height,
            cells,
            particles,
            rules,
            habitat,
            runtime: RuntimeBuffers::default(),
            cached_kind_counts: [0; PARTICLE_KINDS],
            perf: PerfStats::default(),
        };

        world.prepare_runtime();
        world.recount_kinds();
        world
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

        self.prepare_runtime();
    }

    pub fn step(&mut self) {
        let step_start = Instant::now();

        self.prepare_runtime();
        self.tick += 1;

        let particle_start = Instant::now();
        self.step_particles();
        self.perf.particle_ms = particle_start.elapsed().as_secs_f32() * 1000.0;

        self.perf.cell_ms = 0.0;
        self.perf.seed_ms = 0.0;

        if self.tick % 2 == 0 {
            let cell_start = Instant::now();
            self.step_cells();
            self.perf.cell_ms = cell_start.elapsed().as_secs_f32() * 1000.0;
        }

        if self.tick % 9 == 0 {
            let seed_start = Instant::now();
            self.seed_cells_from_particles();
            self.perf.seed_ms = seed_start.elapsed().as_secs_f32() * 1000.0;
        }

        if false && self.tick % 240 == 0 {
            self.mutate_rules();
        }

        self.recount_kinds();
        self.perf.step_ms = step_start.elapsed().as_secs_f32() * 1000.0;
    }

    pub fn kind_counts(&self) -> [usize; PARTICLE_KINDS] {
        self.cached_kind_counts
    }

    pub fn cell_counts(&self) -> (usize, usize, usize, usize) {
        let mut dead = 0usize;
        let mut born = 0usize;
        let mut alive = 0usize;
        let mut dying = 0usize;

        for cell in self.cells.iter() {
            match cell {
                Cell::Dead => dead += 1,
                Cell::Born => born += 1,
                Cell::Alive => alive += 1,
                Cell::Dying => dying += 1,
            }
        }

        (dead, born, alive, dying)
    }

    pub fn perf(&self) -> PerfStats {
        self.perf
    }

    pub fn cell_at(&self, x: usize, y: usize) -> Cell {
        if x >= self.width || y >= self.height {
            return Cell::Dead;
        }
        self.cells[y * self.width + x]
    }

    fn prepare_runtime(&mut self) {
        self.runtime.resize(self.width, self.height);
    }

    fn recount_kinds(&mut self) {
        self.cached_kind_counts = [0; PARTICLE_KINDS];

        for p in self.particles.iter() {
            if p.kind < PARTICLE_KINDS {
                self.cached_kind_counts[p.kind] += 1;
            }
        }
    }

    fn mutate_rules(&mut self) {
        for a in 0..self.rules.len() {
            for b in 0..self.rules[a].len() {
                let wave = mutation_wave(self.seed, self.tick, a, b);
                let counter = mutation_wave(self.seed ^ 0xA53A_9E37, self.tick, b, a);
                let rule = &mut self.rules[a][b];

                rule.attraction = (rule.attraction + wave * 0.014).clamp(-1.95, 2.05);
                rule.orbit = (rule.orbit + counter * 0.016).clamp(-1.85, 1.85);
                rule.resonance = (rule.resonance + wave * counter * 0.014).clamp(-1.75, 1.75);
                rule.pulse = (rule.pulse + wave.abs() * 0.010 - 0.004).clamp(0.20, 2.40);
                rule.harmonic = (rule.harmonic + counter * 0.012).clamp(0.25, 3.25);
                rule.radius = (rule.radius + wave * 0.035).clamp(4.0, 38.0);
                rule.repel_radius = (rule.repel_radius + counter * 0.018).clamp(1.0, 7.5);
                rule.drag = (rule.drag + wave * 0.0008).clamp(0.935, 0.992);
            }
        }
    }

    fn step_particles(&mut self) {
        let snapshot = self.particles.clone();

        fill_pressure_map(
            &mut self.runtime.pressure_map,
            &snapshot,
            self.width,
            self.height,
        );

        self.runtime
            .spatial
            .rebuild(&snapshot, self.width, self.height);

        self.perf.bucket_count = self.runtime.spatial.bucket_count();
        self.perf.max_bucket_load = self.runtime.spatial.max_bucket_load();
        self.perf.avg_bucket_load = self.runtime.spatial.avg_bucket_load();

        let max_x = (self.width - 1) as f32;
        let max_y = (self.height - 1) as f32;
        let center_x = max_x * 0.5;
        let center_y = max_y * 0.5;

        let mut next_particles = snapshot.clone();

        for i in 0..snapshot.len() {
            let current = &snapshot[i];
            let mut ax = 0.0;
            let mut ay = 0.0;
            let mut local_density = 0usize;
            let mut drag_accum = 0.0;
            let mut drag_count = 0.0;
            let mut neighbor_interactions = 0usize;

            let search_radius = self.max_radius_for_kind(current.kind);

            self.runtime.spatial.for_nearby_indices(
                current.x,
                current.y,
                search_radius,
                |other_index| {
                    if neighbor_interactions >= MAX_NEIGHBOR_INTERACTIONS {
                        return;
                    }

                    if other_index == i {
                        return;
                    }

                    let other = &snapshot[other_index];
                    let dx = other.x - current.x;
                    let dy = other.y - current.y;
                    let dist_sq = dx * dx + dy * dy;

                    if dist_sq <= 0.0001 {
                        return;
                    }

                    let rule = &self.rules[current.kind][other.kind];
                    let radius_sq = rule.radius * rule.radius;

                    if dist_sq > radius_sq {
                        return;
                    }

                    let dist = dist_sq.sqrt();

                    neighbor_interactions += 1;
                    local_density += 1;
                    drag_accum += rule.drag;
                    drag_count += 1.0;

                    let nx = dx / dist;
                    let ny = dy / dist;
                    let angle = dy.atan2(dx);

                    let base_force = if dist < rule.repel_radius {
                        let pressure = 1.0 - dist / rule.repel_radius;
                        -3.20 * pressure
                    } else {
                        let denom = (rule.radius - rule.repel_radius).max(0.001);
                        let t = (dist - rule.repel_radius) / denom;
                        let bell = 1.0 - (2.0 * t - 1.0).abs();
                        if rule.attraction > 0.0 {
                            let density_limit = 8.0 + rule.density.abs() * 5.0;
                            let density_factor =
                                1.0 - ((local_density as f32 - density_limit).max(0.0)).min(1.005);
                            rule.attraction * density_factor * bell
                        } else {
                            rule.attraction * bell
                        }
                    };

                    let resonance = resonance_force(dist, angle, self.tick, rule);
                    let radial_force = base_force + resonance * 0.0;
                    let density_shape = rule.density.clamp(-0.95, 0.35)
                        * (1.0 - dist / rule.radius).max(0.0)
                        * 0.000;

                    let tangent_x = -ny;
                    let tangent_y = nx;
                    let orbit_force = rule.orbit * (1.0 - dist / rule.radius).max(0.0);
                    let angular_gate = (angle * rule.symmetry).cos();

                    ax += nx * radial_force * 0.015;
                    ax += nx * density_shape;
                    ay += ny * density_shape;
                    ay += ny * radial_force * 0.015;
                    ax += tangent_x * orbit_force * angular_gate * 0.000;
                    ay += tangent_y * orbit_force * angular_gate * 0.000;
                },
            );

            let density_f = local_density as f32;
            if density_f > 6.0 {
                let pressure = ((density_f - 6.0) / 24.0).min(1.0);
                let away_x = current.x - center_x;
                let away_y = current.y - center_y;
                let len = (away_x * away_x + away_y * away_y).sqrt().max(0.001);

                ax += away_x / len * pressure * 0.000;
                ay += away_y / len * pressure * 0.000;
            }

            let pulse = entropy_pulse(current.x, current.y, self.seed, self.tick, current.kind);
            ax += pulse.0 * 0.0;
            ay += pulse.1 * 0.0;

            let habitat = habitat_force(
                current.x,
                current.y,
                self.width,
                self.height,
                self.seed,
                self.tick,
                current.kind,
                &self.habitat,
            );
            ax += habitat.0 * 0.0;
            ay += habitat.1 * 0.0;

            apply_wall_pressure(current.x, current.y, max_x, max_y, &mut ax, &mut ay);

            let cx = current.x.round().clamp(0.0, max_x) as usize;
            let cy = current.y.round().clamp(0.0, max_y) as usize;
            let pressure = self.runtime.pressure_map[cy * self.width + cx] as usize;

            match self.cell_at(cx, cy) {
                Cell::Alive | Cell::Born => {
                    ax *= 0.82;
                    ay *= 0.82;
                }
                Cell::Dying => {
                    ax -= current.vx * 0.065;
                    ay -= current.vy * 0.065;
                }
                Cell::Dead => {}
            }

            if pressure >= 8 {
                ax += rand_push(current.x, current.y, self.seed, self.tick) * 0.000;
                ay += rand_push(current.y, current.x, self.seed ^ 0xA53A, self.tick) * 0.000;
            }

            // Pressure-orbit well: dense local cores become mild gravity wells with orbital swirl.
            if false && pressure >= 24 {
                let core = ((pressure as f32 - 13.0) / 32.0).min(1.0);

                let left = if cx > 0 {
                    self.runtime.pressure_map[cy * self.width + (cx - 1)] as f32
                } else {
                    pressure as f32
                };

                let right = if cx + 1 < self.width {
                    self.runtime.pressure_map[cy * self.width + (cx + 1)] as f32
                } else {
                    pressure as f32
                };

                let up = if cy > 0 {
                    self.runtime.pressure_map[(cy - 1) * self.width + cx] as f32
                } else {
                    pressure as f32
                };

                let down = if cy + 1 < self.height {
                    self.runtime.pressure_map[(cy + 1) * self.width + cx] as f32
                } else {
                    pressure as f32
                };

                let gx = (right - left).clamp(-24.0, 24.0);
                let gy = (down - up).clamp(-24.0, 24.0);
                let glen = (gx * gx + gy * gy).sqrt().max(0.001);

                ax += gx / glen * core * 0.004;
                ay += gy / glen * core * 0.004;
                ax += -gy / glen * core * 0.007;
                ay += gx / glen * core * 0.007;
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

            next_particles[i].x = x;
            next_particles[i].y = y;
            next_particles[i].vx = vx;
            next_particles[i].vy = vy;
            next_particles[i].age = current.age.saturating_add(1);
        }

        self.particles = next_particles;
    }

    fn max_radius_for_kind(&self, kind: usize) -> f32 {
        self.rules
            .get(kind)
            .map(|row| row.iter().map(|rule| rule.radius).fold(1.0, f32::max))
            .unwrap_or(1.0)
    }

    fn step_cells(&mut self) {
        self.runtime.old_cells.clear();
        self.runtime.old_cells.extend_from_slice(&self.cells);

        fill_pressure_map(
            &mut self.runtime.pressure_map,
            &self.particles,
            self.width,
            self.height,
        );

        self.runtime.next_cells.clear();
        self.runtime
            .next_cells
            .resize(self.width * self.height, Cell::Dead);

        for y in 0..self.height {
            for x in 0..self.width {
                let idx = y * self.width + x;
                let neighbors =
                    live_neighbors(&self.runtime.old_cells, self.width, self.height, x, y);

                let is_alive = matches!(self.runtime.old_cells[idx], Cell::Alive | Cell::Born);
                self.runtime.next_cells[idx] = match (is_alive, neighbors) {
                    (true, 2) | (true, 3) => Cell::Alive,
                    (true, _) => Cell::Dying,
                    (false, 3) => Cell::Born,
                    (false, _) => Cell::Dead,
                };
            }
        }

        std::mem::swap(&mut self.cells, &mut self.runtime.next_cells);
    }

    fn seed_cells_from_particles(&mut self) {
        fill_pressure_map(
            &mut self.runtime.pressure_map,
            &self.particles,
            self.width,
            self.height,
        );

        for p in self.particles.iter() {
            let x = p.x.round().clamp(0.0, (self.width - 1) as f32) as usize;
            let y = p.y.round().clamp(0.0, (self.height - 1) as f32) as usize;
            let idx = y * self.width + x;
            let pressure = self.runtime.pressure_map[idx] as usize;

            if matches!(self.cells[idx], Cell::Dead) && pressure >= 10 {
                self.cells[idx] = Cell::Born;
            }
        }
    }
}

impl HabitatField {
    fn random(rng: &mut StdRng) -> Self {
        Self {
            attract_strength: rng.gen_range(-0.020..0.035),
            repel_strength: rng.gen_range(-0.018..0.026),
            swirl_strength: rng.gen_range(-0.030..0.030),
            pulse_speed: rng.gen_range(0.012..0.075),
            wave_x: rng.gen_range(0.035..0.160),
            wave_y: rng.gen_range(0.035..0.160),
            diagonal_wave: rng.gen_range(0.020..0.120),
            phase_a: rng.gen_range(0.0..std::f32::consts::TAU),
            phase_b: rng.gen_range(0.0..std::f32::consts::TAU),
            symmetry: rng.gen_range(3.0f32..14.0f32).round(),
            turbulence: rng.gen_range(0.002..0.022),
            center_pull: rng.gen_range(-0.002..0.002),
        }
    }
}

#[derive(Clone, Debug, Default)]
struct RuntimeBuffers {
    pressure_map: Vec<u16>,
    old_cells: Vec<Cell>,
    next_cells: Vec<Cell>,
    spatial: SpatialBuckets,
}

impl RuntimeBuffers {
    fn resize(&mut self, width: usize, height: usize) {
        let len = width * height;

        if self.pressure_map.len() != len {
            self.pressure_map.resize(len, 0);
        }

        if self.old_cells.capacity() < len {
            self.old_cells.reserve(len - self.old_cells.capacity());
        }

        if self.next_cells.capacity() < len {
            self.next_cells.reserve(len - self.next_cells.capacity());
        }

        self.spatial.resize(width, height);
    }
}

#[derive(Clone, Debug, Default)]
struct SpatialBuckets {
    buckets: Vec<Vec<usize>>,
    cols: usize,
    rows: usize,
}

impl SpatialBuckets {
    fn resize(&mut self, width: usize, height: usize) {
        let cols = ((width as f32) / BUCKET_SIZE).ceil().max(1.0) as usize;
        let rows = ((height as f32) / BUCKET_SIZE).ceil().max(1.0) as usize;
        let needed = cols * rows;

        if self.cols != cols || self.rows != rows || self.buckets.len() != needed {
            self.cols = cols;
            self.rows = rows;
            self.buckets.clear();
            self.buckets.resize_with(needed, Vec::new);
        }
    }

    fn rebuild(&mut self, particles: &[Particle], width: usize, height: usize) {
        self.resize(width, height);

        for bucket in self.buckets.iter_mut() {
            bucket.clear();
        }

        for (idx, p) in particles.iter().enumerate() {
            let bx = ((p.x / BUCKET_SIZE).floor() as usize).min(self.cols - 1);
            let by = ((p.y / BUCKET_SIZE).floor() as usize).min(self.rows - 1);
            self.buckets[by * self.cols + bx].push(idx);
        }
    }

    fn for_nearby_indices<F>(&self, x: f32, y: f32, radius: f32, mut f: F)
    where
        F: FnMut(usize),
    {
        if self.cols == 0 || self.rows == 0 {
            return;
        }

        let bx = ((x / BUCKET_SIZE).floor() as isize).clamp(0, (self.cols - 1) as isize);
        let by = ((y / BUCKET_SIZE).floor() as isize).clamp(0, (self.rows - 1) as isize);
        let range = (radius / BUCKET_SIZE).ceil() as isize + 1;

        for oy in -range..=range {
            for ox in -range..=range {
                let nx = bx + ox;
                let ny = by + oy;

                if nx < 0 || ny < 0 || nx >= self.cols as isize || ny >= self.rows as isize {
                    continue;
                }

                let bucket_idx = ny as usize * self.cols + nx as usize;
                for other_index in self.buckets[bucket_idx].iter().copied() {
                    f(other_index);
                }
            }
        }
    }

    fn bucket_count(&self) -> usize {
        self.buckets.len()
    }

    fn max_bucket_load(&self) -> usize {
        self.buckets.iter().map(Vec::len).max().unwrap_or(0)
    }

    fn avg_bucket_load(&self) -> f32 {
        if self.buckets.is_empty() {
            return 0.0;
        }

        let total: usize = self.buckets.iter().map(Vec::len).sum();
        total as f32 / self.buckets.len() as f32
    }
}

fn fill_pressure_map(map: &mut Vec<u16>, particles: &[Particle], width: usize, height: usize) {
    let len = width * height;

    if map.len() != len {
        map.resize(len, 0);
    } else {
        map.fill(0);
    }

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
}

fn default_drag() -> f32 {
    0.965
}

fn default_habitat() -> HabitatField {
    let mut rng = StdRng::seed_from_u64(0xA8A8_4040_1313_7777);
    HabitatField::random(&mut rng)
}

fn mutation_wave(seed: u64, tick: u64, a: usize, b: usize) -> f32 {
    let t = tick as f32 * 0.004;
    let sa = ((seed >> ((a % 8) * 7)) & 0xff) as f32 * 0.017;
    let sb = ((seed >> ((b % 8) * 5)) & 0xff) as f32 * 0.013;
    let x = (a as f32 + 1.0) * 0.73 + sa;
    let y = (b as f32 + 1.0) * 1.11 + sb;

    ((t + x).sin() * 0.65 + (t * PHI + y).cos() * 0.35).clamp(-1.0, 1.0)
}

fn habitat_force(
    x: f32,
    y: f32,
    width: usize,
    height: usize,
    seed: u64,
    tick: u64,
    kind: usize,
    habitat: &HabitatField,
) -> (f32, f32) {
    let time = tick as f32 * habitat.pulse_speed;
    let k = kind as f32 + 1.0;
    let cx = width as f32 * 0.5;
    let cy = height as f32 * 0.5;
    let nx = (x - cx) / cx.max(1.0);
    let ny = (y - cy) / cy.max(1.0);
    let angle = ny.atan2(nx);
    let radius = (nx * nx + ny * ny).sqrt();

    let wave_a = (x * habitat.wave_x + time + habitat.phase_a + k * 0.31).sin();
    let wave_b = (y * habitat.wave_y - time * PHI + habitat.phase_b + k * 0.17).cos();
    let wave_c = ((x + y) * habitat.diagonal_wave + time * 0.77).sin();
    let symmetry_gate = (angle * habitat.symmetry + time * 0.23).cos();
    let terrain = wave_a + wave_b + wave_c * symmetry_gate;

    let dx =
        (wave_a.cos() * habitat.wave_x) + (wave_c.cos() * habitat.diagonal_wave * symmetry_gate);
    let dy =
        (-wave_b.sin() * habitat.wave_y) + (wave_c.cos() * habitat.diagonal_wave * symmetry_gate);

    let mut ax = dx * habitat.attract_strength * terrain;
    let mut ay = dy * habitat.attract_strength * terrain;

    ax += -dx * habitat.repel_strength * (1.0 - terrain.abs()).max(0.0);
    ay += -dy * habitat.repel_strength * (1.0 - terrain.abs()).max(0.0);

    ax += -ny * habitat.swirl_strength * symmetry_gate;
    ay += nx * habitat.swirl_strength * symmetry_gate;

    ax += -nx * habitat.center_pull * radius;
    ay += -ny * habitat.center_pull * radius;

    ax += rand_push(x, y, seed ^ 0x514D_7A1B, tick + kind as u64) * habitat.turbulence;
    ay += rand_push(y, x, seed ^ 0xBEEF_91C7, tick + kind as u64) * habitat.turbulence;

    (ax, ay)
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
