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

    pub fn draw(&mut self, world: &World, paused: bool, tick_delay: Duration) -> Result<()> {
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

            let mut grid = vec![(' ', Color::Reset); draw_w.saturating_mul(draw_h)];

            for y in 0..draw_h {
                for x in 0..draw_w {
                    let idx = y * draw_w + x;

                    grid[idx] = match world.cell_at(x, y) {
                        Cell::Alive if world.tick % 30 == 0 => ('.', Color::DarkGray),
                        Cell::Born if world.tick % 36 == 0 => (',', Color::DarkGray),
                        _ => (' ', Color::Reset),
                    };
                }
            }

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

                grid[idx] = match p.kind {
                    0 => ('.', Color::Yellow),
                    1 => ('.', Color::Magenta),
                    2 => ('.', Color::Blue),
                    _ => ('.', Color::Red),
                };
            }

            let mut lines = Vec::with_capacity(draw_h);

            for y in 0..draw_h {
                let mut spans = Vec::with_capacity(draw_w);

                for x in 0..draw_w {
                    let (glyph, color) = grid[y * draw_w + x];

                    spans.push(Span::styled(glyph.to_string(), Style::default().fg(color)));
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

            let status_color = if paused { Color::Yellow } else { Color::Green };

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
                        Style::default().fg(Color::Yellow),
                    ),
                    Span::raw(" | cells "),
                    Span::styled(
                        world.live_cell_count().to_string(),
                        Style::default().fg(Color::Magenta),
                    ),
                    Span::raw(" | "),
                    Span::styled(
                        if paused { "PAUSED" } else { "ALIVE" },
                        Style::default().fg(status_color),
                    ),
                    Span::raw(" | "),
                    Span::styled(
                        format!("{}ms", tick_delay.as_millis()),
                        Style::default().fg(Color::Blue),
                    ),
                ]),
                Line::from(vec![
                    Span::styled(" amber ", Style::default().fg(Color::Yellow)),
                    Span::raw(world.kind_count(0).to_string()),
                    Span::styled(" | violet ", Style::default().fg(Color::Magenta)),
                    Span::raw(world.kind_count(1).to_string()),
                    Span::styled(" | blue ", Style::default().fg(Color::Blue)),
                    Span::raw(world.kind_count(2).to_string()),
                    Span::styled(" | red ", Style::default().fg(Color::Red)),
                    Span::raw(world.kind_count(3).to_string()),
                    Span::raw(
                        " | q save+quit | s save | n new | r reroll | space pause | +/- speed",
                    ),
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
