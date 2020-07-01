use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use std::cell::RefCell;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

// import in javascript symbols we want to use
#[wasm_bindgen]
extern {
    fn alert(s: &str);
}

#[wasm_bindgen]
pub fn greet() {
    alert("Hello, clientwasm!");
}

// the webassebmly doc says I can rely on floating point determanism, which is nice

// bot state
struct BotState {
    health: f32,
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    team: u16,
}

// base state
struct BaseState {
    health: f32,
    nextspawn: f32, // in game time seconds (based on ticks and tick_rate)
    x: f32,
    y: f32,
}

enum Terrain {
    Nothing,

}

struct GameMap {
    w: u32,
    h: u32,
    bytes: Box<[u8]>, // can be color, or for base map it is terrain info
}

impl GameMap {
    fn new(w: u32, h: u32) -> GameMap {
        GameMap{
            w,
            h,
            bytes: (vec![0; (w * h) as usize]).into_boxed_slice(),
        }
    }
}

// game tick structure
// has to contain the current paint layers, and the current bot states, the base states
struct GameTick {
    tick: u32,
    bots: Vec<BotState>,
    teambotcount: Vec<u32>,
    bases: Vec<BaseState>,
    paints: Vec<GameMap>, // Maybe we want to have a DeltaMap option, instead of a full Map here?
}

// game structure
// has to contain a vec of GameTicks we have processed or are working on
// also contains static game info, including the board layout
struct Game {
    states: Vec<GameTick>,
    tickratio: u32, // # of ticks before a netstep tick, doesn't change
    tickstep: f32, // game seconds per tick, doesn't change (try to keep real seconds per tick similar to this)
    dispratio: f32, // current game seconds to go per real second for drawing to the screen, smooths lag and ticks
    map: GameMap, // the static map (walls and cover) below the changing paint layers
    usermap: GameMap, // a buffer that is passed to the javascript layer to take in paint changes
    ctx: web_sys::CanvasRenderingContext2d, // the canvas ctx
}

thread_local!(
    static GAME: RefCell<Option<Game>> = RefCell::new(None);
);

#[wasm_bindgen]
pub fn init_game(can_id: &str, mapw: u32, maph: u32, tick_ratio: u32, tick_step: f32) {
    // do init stuff
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();

    // drop any previous game?
    //TODO

    // get canvas
    let document = web_sys::window().unwrap().document().unwrap();
    let canvas = document.get_element_by_id(can_id).unwrap();
    let canvas: web_sys::HtmlCanvasElement = canvas
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .map_err(|_| ())
        .unwrap();

    let ctx = canvas
        .get_context("2d")
        .unwrap()
        .unwrap()
        .dyn_into::<web_sys::CanvasRenderingContext2d>()
        .unwrap();

    GAME.with(|g| {
        *g.borrow_mut() = Some(Game {
            states: Vec::new(),
            tickratio: tick_ratio,
            tickstep: tick_step,
            dispratio: 1.0,
            map: GameMap::new(mapw, maph),
            usermap: GameMap::new(mapw, maph),
            ctx,
        });
    });

}

//API

// init_game(canvas id, ticks per netstep, mapx, mapy, target tick rate per second)
// target tick rate helps us lerp for the draw call

// get_paint_buf()

// setpaint(netstep #) (indicates shared memory area given by get_paint_buf is updated in the given bounds)
// see https://rustwasm.github.io/book/game-of-life/implementing.html for how to access wasm memory from js

// tick()

// draw(dt)
// using web_sys we can actually use the canvas API from rust
// drawing is smoothed over the actual ticks
// how?
// netsteps will come in with paint updates from the "future"
// We don't want to lag behind the latest updates more than a second

// so our requestanimationframe needs to know where to draw in the tick stream
// it will have it's own trailing float for where it is in the tick stream
// and will lerp between ticks
// when it gets closer to running out of buffer, it will speed up it's rate
// when it get farther, it will slow down it's rate
// basically a small p controller, upgrade to pid if we bounce

// we will move the size of that buffer larger if we have slowdown of producing ticks because of network issues
// okay, that keeps a good separation between ticks and rendering
// while allowing us to have determanistic sim multiplayer
