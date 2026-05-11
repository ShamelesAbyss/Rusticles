use crate::world::{Cell, World};
use anyhow::Result;
use crossterm::{
    cursor, execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Terminal as RatTerminal,
};
use std::io::{stdout, Stdout};
use std::time::Duration;

pub struct Terminal {
    inner: RatTerminal<CrosstermBackend<Stdout>>,
}

#[derive(Clone, Copy, Debug)]
struct CellStack {
    total: usize,
    kinds: [usize; 8],
}

impl Terminal {
    pub fn enter() -> Result<Self> {
        terminal::enable_raw_mode()?;

        let mut out = stdout();
        execute!(out, EnterAlternateScreen, cursor::Hide)?;

        let backend = CrosstermBackend::new(out);
        let mut inner = RatTerminal::new(backend)?;
        inner.clear()?;

        Ok(Self { inner })
    }

    pub fn draw(&mut self, world: &World, paused: bool, _tick_delay: Duration) -> Result<()> {
        self.inner.draw(|frame| {
            let area = frame.size();

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(8), Constraint::Length(4)])
                .split(area);

            let field_area = chunks[0];
            let hud_area = chunks[1];

            let draw_w = field_area.width.saturating_sub(2) as usize;
            let draw_h = field_area.height.saturating_sub(2) as usize;

            let draw_w = draw_w.min(world.width);
            let draw_h = draw_h.min(world.height);

            let mut stacks = vec![
                CellStack {
                    total: 0,
                    kinds: [0; 8],
                };
                draw_w.saturating_mul(draw_h)
            ];

            for p in world.particles.iter() {
                let x = p.x.round() as isize;
                let y = p.y.round() as isize;

                if x < 0 || y < 0 {
                    continue;
                }

                let x = x as usize;
                let y = y as usize;

                if x >= draw_w || y >= draw_h {
                    continue;
                }

                let idx = y * draw_w + x;
                let kind = p.kind.min(3);

                stacks[idx].total += 1;
                stacks[idx].kinds[kind] += 1;
            }

            let visible_cells = stacks.iter().filter(|stack| stack.total > 0).count();
            let compressed_particles = world.particles.len().saturating_sub(visible_cells);
            let mixed_cells = stacks
                .iter()
                .filter(|stack| stack.kinds.iter().filter(|count| **count > 0).count() >= 2)
                .count();

            let mut lines = Vec::with_capacity(draw_h);

            for y in 0..draw_h {
                let mut spans = Vec::with_capacity(draw_w);

                for x in 0..draw_w {
                    let idx = y * draw_w + x;
                    let stack = stacks[idx];

                    if stack.total > 0 {
                        let active_kinds = stack.kinds.iter().filter(|count| **count > 0).count();
                        let dominant = dominant_kind(stack.kinds);

                        let glyph = match stack.total {
                            1 => "•",
                            2..=3 => "●",
                            4..=7 => "O",
                            8..=15 => "@",
                            _ => "#",
                        };

                        let color = if active_kinds >= 3 {
                            Color::Rgb(255, 255, 255)
                        } else if active_kinds == 2 {
                            Color::Rgb(180, 255, 255)
                        } else {
                            species_color(dominant)
                        };

                        spans.push(Span::styled(glyph, Style::default().fg(color)));
                    } else {
                        let ghost = match world.cell_at(x, y) {
                            Cell::Alive if world.tick % 40 == 0 => Some("."),
                            Cell::Born if world.tick % 50 == 0 => Some(","),
                            _ => None,
                        };

                        if let Some(glyph) = ghost {
                            spans.push(Span::styled(glyph, Style::default().fg(Color::DarkGray)));
                        } else {
                            spans.push(Span::raw(" "));
                        }
                    }
                }

                lines.push(Line::from(spans));
            }

            let field = Paragraph::new(lines).block(
                Block::default()
                    .title(" RUSTICLES PARTICLE FIELD ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray))
                    .title_style(Style::default().fg(Color::Cyan)),
            );

            frame.render_widget(field, field_area);

            let status_color = if paused {
                Color::Rgb(255, 220, 40)
            } else {
                Color::Green
            };

            let hud_lines = vec![
                Line::from(vec![
                    Span::styled(" RUSTICLES ", Style::default().fg(Color::Cyan)),
                    Span::raw(" seed "),
                    Span::styled(world.seed.to_string(), Style::default().fg(Color::Green)),
                    Span::raw(" | tick "),
                    Span::styled(world.tick.to_string(), Style::default().fg(Color::Green)),
                    Span::raw(" | particles "),
                    Span::styled(
                        world.particles.len().to_string(),
                        Style::default().fg(Color::Rgb(255, 220, 40)),
                    ),
                    Span::raw(" | visible "),
                    Span::styled(
                        visible_cells.to_string(),
                        Style::default().fg(Color::Rgb(40, 180, 255)),
                    ),
                    Span::raw(" | compressed "),
                    Span::styled(
                        compressed_particles.to_string(),
                        Style::default().fg(Color::Rgb(255, 80, 255)),
                    ),
                    Span::raw(" | mixed "),
                    Span::styled(
                        mixed_cells.to_string(),
                        Style::default().fg(Color::Rgb(180, 255, 255)),
                    ),
                    Span::raw(" | "),
                    Span::styled(
                        if paused { "PAUSED" } else { "ALIVE" },
                        Style::default().fg(status_color),
                    ),
                ]),
                Line::from(vec![
                    Span::styled(" amber ", Style::default().fg(species_color(0))),
                    Span::raw(world.kind_count(0).to_string()),
                    Span::styled(" | magenta ", Style::default().fg(species_color(1))),
                    Span::raw(world.kind_count(1).to_string()),
                    Span::styled(" | cyan ", Style::default().fg(species_color(2))),
                    Span::raw(world.kind_count(2).to_string()),
                    Span::styled(" | red ", Style::default().fg(species_color(3))),
                    Span::raw(world.kind_count(3).to_string()),
                    Span::styled(" | green ", Style::default().fg(species_color(4))),
                    Span::raw(world.kind_count(4).to_string()),
                    Span::styled(" | violet ", Style::default().fg(species_color(5))),
                    Span::raw(world.kind_count(5).to_string()),
                    Span::styled(" | orange ", Style::default().fg(species_color(6))),
                    Span::raw(world.kind_count(6).to_string()),
                    Span::styled(" | white ", Style::default().fg(species_color(7))),
                    Span::raw(world.kind_count(7).to_string()),
                ]),
                Line::from(vec![
                    Span::raw(" density: • ● O @ # | q save+quit | s save | n new | r reroll | space/p pause "),
                ]),
            ];

            let hud = Paragraph::new(hud_lines).block(
                Block::default()
                    .title(" CORE ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray))
                    .title_style(Style::default().fg(Color::Cyan)),
            );

            frame.render_widget(hud, hud_area);
        })?;

        Ok(())
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
        let _ = execute!(self.inner.backend_mut(), cursor::Show, LeaveAlternateScreen);
        let _ = self.inner.show_cursor();
    }
}

fn dominant_kind(kinds: [usize; 8]) -> usize {
    let mut best_kind = 0;
    let mut best_count = 0;

    for (kind, count) in kinds.iter().enumerate() {
        if *count > best_count {
            best_kind = kind;
            best_count = *count;
        }
    }

    best_kind
}

fn species_color(kind: usize) -> Color {
    match kind {
        0 => Color::Rgb(255, 220, 40),
        1 => Color::Rgb(255, 80, 255),
        2 => Color::Rgb(40, 180, 255),
        3 => Color::Rgb(255, 55, 55),
        4 => Color::Rgb(80, 255, 90),
        5 => Color::Rgb(170, 80, 255),
        6 => Color::Rgb(255, 140, 30),
        _ => Color::Rgb(230, 255, 255),
    }
}
