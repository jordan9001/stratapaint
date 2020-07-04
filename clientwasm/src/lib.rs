use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use std::cell::RefCell;
use std::f64;
use std::f32;
use rand_xorshift::XorShiftRng;
use rand::SeedableRng;
use rand::Rng;
use std::cmp::Ordering;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

// import in javascript symbols we want to use
#[wasm_bindgen]
extern {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

// bot state
#[derive(Clone,Copy)]
struct BotState {
    id: u32,
    health: f32,
    x: f32, // these are in units of position on GameMap, not canvas. But we treat the canvas as px == GameMap unit, and just transform the canvas
    y: f32,
    vx: f32,
    vy: f32,
    team: u16,
}

// bot constants
const BOTRAD: f64 = 1.0;
const BOTRANDMAXACC: f32 = 10.0;
const BOTMAXVEL: f32 = 27.0;
const BOTBOUNCEAMT: f32 = 0.42;

// base state
#[derive(Clone,Copy)]
struct BaseState {
    id: u32,
    health: f32,
    nextspawn: f32, // in game time seconds (based on ticks and tick_rate)
    x: f32,
    y: f32,
    team: i32,
}

#[derive(Clone)]
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
#[derive(Clone)]
struct GameTick {
    tick: u32,
    bases: Vec<BaseState>,
    bots: Vec<BotState>, // Vector must always have lower bot id's ordered with indexes
    teambotcount: Vec<u32>,
    paints: Vec<GameMap>, // Maybe we want to have a DeltaMap option?
}

impl GameTick {

    // cleans out things only needed for future tick processing
    // and keeps the things needed for future drawing
    fn cleanup(&mut self) {
        self.paints.clear();
    }
}

struct DisplayInfo {
    pk: f32,
    dk: f32,
    dmax: f32,
    ratio: f32, // current game seconds to go per real second for drawing to the screen, smooths lag and ticks
    tick: f32,   // where we are displaying in ticks
    targetlag: f32, // how far behind we want to be in ticks
    preverr: f32,
}
const DIS_PK: f32 = 0.001;
const DIS_DK: f32 = 1.2;
const DIS_DMAX: f32 = 0.01;
const DIS_RMIN: f32 = 0.00000001;
const DIS_LAG: f32 = 9.6;
const DIS_RMAX: f32 = 0.06;

// game structure
// has to contain a vec of GameTicks we have processed or are working on
// also contains static game info, including the board layout
struct Game {
    states: Vec<GameTick>, // vector must always have the newest ticks (higher number) at lower indexes
    tickratio: u32, // # of ticks before a netstep tick, doesn't change
    tickstep: f32, // game seconds per tick, doesn't change (try to keep real seconds per tick similar to this)
    curtick: u32, // next tick to process
    map: GameMap, // the static map (walls and cover) below the changing paint layers
    usermap: GameMap, // a buffer that is passed to the javascript layer to take in paint changes
    ctx: web_sys::CanvasRenderingContext2d, // the canvas ctx
    canvas: web_sys::HtmlCanvasElement,
    baseseed: u32,
    objidcntr: u32,
    dis: DisplayInfo,
}

// game constants


impl Game {
    fn init_state(&mut self) {
        // create the inital state and game map
        if !self.states.is_empty() {
            panic!("Tried to init state with non-empty states vector");
        }
        if self.curtick != 0 {
            panic!("Tried to init state with nonzero current tick");
        }

        let mut tk = GameTick {
            tick: 0,
            bases: Vec::new(),
            bots: Vec::new(),
            teambotcount: Vec::new(),
            paints: Vec::new(),
        };

        // create per team layers
        // for now only one team
        for _ in 0..1 {
            log("Adding Team");
            tk.teambotcount.push(0);
            tk.paints.push(GameMap::new(self.map.w, self.map.h));
        }

        // for now spawn a bunch of bots all across the map
        let stride = 12;
        for x in 1..((self.map.w/stride)-1) {
            for y in 1..((self.map.h/stride)-1) {
                tk.bots.push(BotState {
                    id: self.objidcntr,
                    health: 100.0,
                    x: (x * stride) as f32,
                    y: (y * stride) as f32,
                    vx: 0.0,
                    vy: 0.0,
                    team: 0,
                });
                self.objidcntr += 1;
                tk.teambotcount[0] += 1;
            }
        }

        log(&format!("starting out with {} bots", tk.teambotcount[0])[..]);

        self.states.push(tk);
    }

    fn tick(&mut self) {
        //TODO
        // move bots with the following priotities
        // 1: don't move off the map
        // 2: don't move into a wall
        // 3: move away from close bots (//TODO have quadtree for map to make nearby lookups easy?)
        // 4: obey local paint 
        // 5: random

        
        //DEBUG
        //let newtk: &mut GameTick = &mut self.states[0];


        let mut newtk: GameTick = self.get_cur_tick().clone();
        let oldtick = newtk.tick; // save old tick so we can clean it at the end
        newtk.tick += 1;

        // get prng for this tick
        let mut rng = XorShiftRng::seed_from_u64((self.baseseed + newtk.tick) as u64);

        let botln = newtk.bots.len();
        
        for i in 0..botln {
            // add random accel to each bot
            let amt: f32 = rng.gen_range(0.0, BOTRANDMAXACC) * self.tickstep;
            let ang: f32 = rng.gen_range(0.0, f32::consts::PI * 2.0);

            let xpart: f32 = ang.cos() * amt;
            let ypart: f32 = ang.sin() * amt;
            newtk.bots[i].vx += xpart;
            newtk.bots[i].vy += ypart;

            // query paint for force
            //TODO

            
            // add forces pushing local bots away
            //TODO
            // +1 too a lower rez 2d map that can be used to "roll downhill" away from groups of bots
        }

        for i in 0..botln {
            // max out vel
            /*
            let x2 = newtk.bots[i].vx * newtk.bots[i].vx;
            let y2 = newtk.bots[i].vy * newtk.bots[i].vy;
            
            let vel2 = x2 + y2;
            if vel2 > BOTMAXVEL2 {
                // scale value for x and y = BOTMAXVEL / sqrt(x^2 + y^2)
                let s = BOTMAXVEL / (vel2.sqrt());
                newtk.bots[i].vx *= s;
                newtk.bots[i].vy *= s;
            }
            */
            // simpler check
            if newtk.bots[i].vx > BOTMAXVEL {
                newtk.bots[i].vx = BOTMAXVEL;
            } else if newtk.bots[i].vx < -BOTMAXVEL {
                newtk.bots[i].vx = -BOTMAXVEL;
            }
            if newtk.bots[i].vy > BOTMAXVEL {
                newtk.bots[i].vy = BOTMAXVEL;
            } else if newtk.bots[i].vy < -BOTMAXVEL {
                newtk.bots[i].vy = -BOTMAXVEL;
            }

            // step vel for each bot
            let mut newx = newtk.bots[i].x + (newtk.bots[i].vx * self.tickstep);
            let mut newy = newtk.bots[i].y + (newtk.bots[i].vy * self.tickstep);

            // bounce off wall
            // TODO

            // bounce off edge
            // don't have to be as complicated as walls
            // just don't move, and bounce velocity
            if newx <= 0.0  || newx >= (self.map.w as f32) {
                newx = newtk.bots[i].x;
                newtk.bots[i].vx *= -BOTBOUNCEAMT;
            }
            if newy <= 0.0  || newy >= (self.map.h as f32) {
                newy = newtk.bots[i].y;
                newtk.bots[i].vy *= -BOTBOUNCEAMT;
            }

            newtk.bots[i].x = newx;
            newtk.bots[i].y = newy;
        }

        // clean old tick info not needed for drawing
        //DEBUG
        
        let mut i = 0;
        loop {
            if i >= self.states.len() {
                panic!("Could not find oldtick in states vector");
            }

            let t = &mut self.states[i];
            if t.tick == oldtick {
                t.cleanup();
                break;
            }

            i += 1;
        };
        
        self.states.insert(0, newtk);
        self.curtick += 1;
    }

    fn draw(&mut self, dt: f32) {
        // First get it working with no smoothing, just display latest TICK
        
        // get target ticks to lerp between
        let mut disp2 = self.dis.tick.ceil() as u32;
        if disp2 > self.curtick {
            self.dis.tick = self.curtick as f32;
            disp2 = self.curtick;
        }
        let disp1 = self.dis.tick.floor() as u32;
        let lerpfac = self.dis.tick.fract();

        let mut i = 0;
        let tk2 = loop {
            if i >= self.states.len() {
                panic!("Could not find tk2 states vector");
            }

            let t = &self.states[i];
            if t.tick == disp2 {
                break t;
            }

            i += 1;
        };
        
        let tk1 = loop {
            if i >= self.states.len() {
                panic!("Could not find tk2 states vector");
            }

            let t = &self.states[i];
            if t.tick == disp1 {
                break t;
            }

            i += 1;
        };
        
        // clear canvas
        // save transform
        self.ctx.save();
        // reset transform
        self.ctx.set_transform(1.0, 0.0, 0.0, 1.0, 0.0, 0.0).expect("Unable to set transform for clearing");
        // clear everything
        self.ctx.clear_rect(0.0, 0.0, self.canvas.width() as f64, self.canvas.height() as f64);
        // restore transform
        self.ctx.restore();

        // draw bots
        self.ctx.set_fill_style(&JsValue::from_str("#fa110e"));

        let mut i1 = 0;
        let mut i2 = 0;
        let tk1len = tk1.bots.len();
        let tk2len = tk2.bots.len();
        // step through both ticks, bots are always ordered
        while (i1 < tk1len) && (i2 < tk2len) {
            let id1 = tk1.bots[i1].id;
            let id2 = tk2.bots[i2].id;
            match id1.cmp(&id2) {
                Ordering::Equal => {
                    let x1 = tk1.bots[i1].x;
                    let y1 = tk1.bots[i1].y;
                    let x2 = tk2.bots[i2].x;
                    let y2 = tk2.bots[i2].y;
    
                    // lerp
                    let x: f32 = (lerpfac * (x2 - x1)) + x1;
                    let y: f32 = (lerpfac * (y2 - y1)) + y1;
    
                    self.ctx.begin_path();
                    self.ctx.arc(x as f64, y as f64, BOTRAD, 0.0, f64::consts::PI * 2.0).expect("Unable to draw bot");
                    self.ctx.fill();
    
                    i1 += 1;
                    i2 += 1;
                },
                Ordering::Less => {
                    i1 += 1;
                }
                Ordering::Greater => {
                    i2 += 1;
                }
            }
        }

        // draw bases
        //TODO

        // draw map features
        //TODO

        // adjust the ratio to even out
        let err = ((self.curtick as f32) - self.dis.targetlag) - self.dis.tick;
        let derr = (err - self.dis.preverr) / dt;
        self.dis.preverr = err;
        let mut dratio = (self.dis.pk * err) + (self.dis.dk * derr);

        
        if dratio > self.dis.dmax {
            dratio = self.dis.dmax;
        } else if dratio < -self.dis.dmax {
            dratio = -self.dis.dmax;
        }
        self.dis.ratio += dratio;
        if self.dis.ratio <= DIS_RMIN {
            self.dis.ratio = DIS_RMIN;
        } else if self.dis.ratio >= DIS_RMAX {
            self.dis.ratio = DIS_RMAX;
        }

        
        //DEBUG
        log(&format!(
            "cur: {}\ndisp: {}\nerr: {}\ndt: {}\nratio: {}\ndratio: {}",
            self.curtick,
            self.dis.tick,
            err,
            dt,
            self.dis.ratio,
            dratio,
        )[..]);
        

        // move our displayed tick
        self.dis.tick += dt * self.dis.ratio;

        // clean up old ticks not needed anymore
        let mut disp1 = self.dis.tick.floor() as u32;
        if disp1 > self.curtick {
            disp1 = self.curtick;
        }

        let mut i = 0;
        while i < self.states.len() {
            let mut isold: bool = false;
            {
                let t = &self.states[i];
                if t.tick < disp1 {
                    isold = true;
                }
            }
            if isold {
                self.states.remove(i);
            } else {
                i += 1;
            }
        }
        
    }

    fn get_cur_tick(&self) -> &GameTick {
        let mut i = 0;
        loop {
            if i >= self.states.len() {
                panic!("Could not find curtick in states vector");
            }

            let t = &self.states[i];
            if t.tick == self.curtick {
                return t;
            }

            i += 1;
        };
    }
}

thread_local!(
    static GAME: RefCell<Option<Game>> = RefCell::new(None);
);

#[wasm_bindgen]
pub fn init_game(can_id: &str,
    mapw: u32,
    maph: u32,
    tick_ratio: u32,    // number of ticks per netstep
    tick_step: f32,     // milliseconds per tick (ish, can be affected by netstep lag or computation lag)
    seed: u32,
) {
    // do init stuff
    // setup console panics
    //#[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();

    // drop any previous game
    GAME.with(|g| {
        *g.borrow_mut() = None;
    });

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

    let tickstep = tick_step / 1000.0; // milliseconds to seconds
    // tick_step (ms / tick)
    // dispratio (ticks / ms) = 1/tick_step 
    let dispratio = 1.0 / tick_step;

    GAME.with(|g| {
        let g = &mut *g.borrow_mut();
        *g = Some(Game {
            states: Vec::new(),
            tickratio: tick_ratio,
            tickstep,
            curtick: 0,
            dis: DisplayInfo {
                pk: DIS_PK,
                dk: DIS_DK,
                dmax: DIS_DMAX,
                ratio: dispratio,
                tick: 0.0,
                targetlag: DIS_LAG,
                preverr: 0.0,
            },
            map: GameMap::new(mapw, maph),
            usermap: GameMap::new(mapw, maph),
            ctx,
            canvas,
            baseseed: seed,
            objidcntr: 0,
        });

        if let Some(game) = &mut *g {
            game.init_state();
        }
    });
}

#[wasm_bindgen]
pub fn draw(dt: f32) {
    GAME.with(|g| {
        if let Some(game) = &mut *g.borrow_mut() {
            game.draw(dt);
        }
    });
}

#[wasm_bindgen]
pub fn tick() {
    GAME.with(|g| {
        if let Some(game) = &mut *g.borrow_mut() {
            game.tick();
        }
    });
}

#[wasm_bindgen]
pub fn adj_dis(pk: f32, dk: f32, dmax: f32, targetlag: f32) {
    GAME.with(|g| {
        if let Some(game) = &mut *g.borrow_mut() {
            let di = &mut game.dis;
            di.pk = pk;
            di.dk = dk;
            di.dmax = pk;
            di.targetlag = targetlag;
        }
    });
}

//API

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
