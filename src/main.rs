mod memory;
mod terminal;
mod world;

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal as cterm,
};
use std::time::{Duration, Instant};
use terminal::Terminal;
use world::World;

const SAVE_PATH: &str = "saves/rusticles_memory.json";

fn main() -> Result<()> {
    let (world_w, world_h) = viewport_size();

    let mut terminal = Terminal::enter()?;
    let mut world = memory::load(SAVE_PATH)?.unwrap_or_else(|| World::new_random(world_w, world_h));
    world.resize_to(world_w, world_h);

    let mut paused = false;
    let mut tick_delay = Duration::from_millis(45);
    let render_delay = Duration::from_millis(100);

    let mut last_tick = Instant::now();
    let mut last_render = Instant::now() - render_delay;

    loop {
        let (new_w, new_h) = viewport_size();
        if world.width != new_w || world.height != new_h {
            world.resize_to(new_w, new_h);
        }

        while event::poll(Duration::from_millis(2))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

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
                        let (w, h) = viewport_size();
                        world = World::new_random(w, h);
                        memory::save(SAVE_PATH, &world)?;
                    }
                    KeyCode::Char('r') => {
                        let (w, h) = viewport_size();
                        world = World::new_random(w, h);
                    }
                    KeyCode::Char(' ') | KeyCode::Char('p') => {
                        paused = !paused;
                        last_render = Instant::now() - render_delay;
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

fn viewport_size() -> (usize, usize) {
    let (tw, th) = cterm::size().unwrap_or((120, 42));

    let width = tw.saturating_sub(2).max(40) as usize;
    let height = th.saturating_sub(6).max(12) as usize;

    (width, height)
}
