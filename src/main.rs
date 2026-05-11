mod memory;
mod terminal;
mod world;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode};
use std::time::{Duration, Instant};
use terminal::Terminal;
use world::World;

const SAVE_PATH: &str = "saves/rusticles_memory.json";

fn main() -> Result<()> {
    let mut terminal = Terminal::enter()?;
    let mut world = memory::load(SAVE_PATH)?.unwrap_or_else(World::new_random);

    let mut paused = false;
    let mut tick_delay = Duration::from_millis(45);
    let render_delay = Duration::from_millis(66);

    let mut last_tick = Instant::now();
    let mut last_render = Instant::now() - render_delay;

    loop {
        while event::poll(Duration::from_millis(2))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => {
                        memory::save(SAVE_PATH, &world)?;
                        return Ok(());
                    }
                    KeyCode::Char('s') => {
                        memory::save(SAVE_PATH, &world)?;
                    }
                    KeyCode::Char('n') => {
                        memory::wipe(SAVE_PATH)?;
                        world = World::new_random();
                        memory::save(SAVE_PATH, &world)?;
                    }
                    KeyCode::Char('r') => {
                        world = World::new_random();
                    }
                    KeyCode::Char(' ') => {
                        paused = !paused;
                    }
                    KeyCode::Char('+') | KeyCode::Char('=') => {
                        tick_delay = tick_delay.saturating_sub(Duration::from_millis(5));
                        if tick_delay < Duration::from_millis(10) {
                            tick_delay = Duration::from_millis(10);
                        }
                    }
                    KeyCode::Char('-') => {
                        tick_delay += Duration::from_millis(5);
                        if tick_delay > Duration::from_millis(250) {
                            tick_delay = Duration::from_millis(250);
                        }
                    }
                    _ => {}
                }
            }
        }

        if !paused && last_tick.elapsed() >= tick_delay {
            world.step();
            last_tick = Instant::now();
        }

        if last_render.elapsed() >= render_delay {
            terminal.draw(&world, paused, tick_delay)?;
            last_render = Instant::now();
        }

        std::thread::sleep(Duration::from_millis(3));
    }
}
