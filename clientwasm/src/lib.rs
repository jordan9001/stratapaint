use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use std::cell::RefCell;
use std::f64;
use std::f32;
use rand_xorshift::XorShiftRng;
use rand::SeedableRng;
use rand::Rng;
use std::cmp::Ordering;
use std::collections::HashMap;

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
    cur_target: u32,
}

// bot constants
const BOTRAD: f64 = 2.1;
const BOTRANDMAXACC: f32 = 10.0;
const BOTMAXVEL: f32 = 27.0;
const BOTBOUNCEAMT: f32 = 0.42;

#[derive(Clone)]
enum QuadBranch {
    Branch(Box<(QuadNode, QuadNode, QuadNode, QuadNode)>), // nw,ne,se,sw
    BotList(Vec<(u32, u32, u32)>),
    Nothing,
}

#[derive(Clone)]
struct QuadNode {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    brn: QuadBranch,
}

// QuadNode constants
const QUADSPLITMIN: u32 = 64;
const QUADBALANCE: usize = 90;

impl QuadNode {
    fn add_bot(&mut self, bt: (u32, u32, u32)) { // (id, x, y)
        let midx = self.x + (self.w / 2);
        let midy = self.y + (self.h / 2);
        match &mut self.brn {
            QuadBranch::Nothing => {
                let lst: Vec<(u32, u32, u32)> = vec![bt];
                self.brn = QuadBranch::BotList(lst);
            },
            QuadBranch::Branch(b) => {
                if bt.1 < midx {
                    if bt.2 < midy {
                        b.0.add_bot(bt);
                    } else {
                        b.3.add_bot(bt);
                    }
                } else {
                    if bt.2 < midy {
                        b.1.add_bot(bt);
                    } else {
                        b.2.add_bot(bt);
                    }
                }
            },
            QuadBranch::BotList(bl) => {
                if (bl.len() >= QUADBALANCE) && (self.w > QUADSPLITMIN && self.h > QUADSPLITMIN) {
                    // split
                    let mut bx = Box::new((
                        QuadNode{ // nw
                            x: self.x,
                            y: self.y,
                            w: (self.w / 2),
                            h: (self.h / 2),
                            brn: QuadBranch::Nothing,
                        },
                        QuadNode{ // ne
                            x: midx,
                            y: self.y,
                            w: self.w - (self.w / 2),
                            h: (self.h / 2),
                            brn: QuadBranch::Nothing,
                        },
                        QuadNode{ // se
                            x: midx,
                            y: midy,
                            w: self.w - (self.w / 2),
                            h: self.h - (self.h / 2),
                            brn: QuadBranch::Nothing,
                        },
                        QuadNode{ // sw
                            x: self.x,
                            y: midy,
                            w: (self.w / 2),
                            h: self.h - (self.h / 2),
                            brn: QuadBranch::Nothing,
                        },
                    ));

                    // add old ones
                    for binf in bl {
                        if (binf.1 as u32) < midx {
                            if (binf.2 as u32) < midy {
                                bx.0.add_bot(bt);
                            } else {
                                bx.3.add_bot(bt);
                            }
                        } else {
                            if (binf.2 as u32) < midy {
                                bx.1.add_bot(bt);
                            } else {
                                bx.2.add_bot(bt);
                            }
                        }
                    }

                    // add new one (don't keep splitting right now, just go past max if all ended up same spot)
                    if bt.1 < midx {
                        if bt.2 < midy {
                            bx.0.add_bot(bt);
                        } else {
                            bx.3.add_bot(bt);
                        }
                    } else {
                        if bt.2 < midy {
                            bx.1.add_bot(bt);
                        } else {
                            bx.2.add_bot(bt);
                        }
                    }

                    self.brn = QuadBranch::Branch(bx);
                } else {
                    bl.push(bt);
                }
            }
        }
    }
    fn move_bot(&mut self, bt: (u32, u32, u32), prevx: u32, prevy: u32) -> bool {
        //TODO
        // find
        // if we can update, do that
        // else remove, add if possible (might be from upper layer)

        let midx = self.x + (self.w / 2);
        let midy = self.y + (self.h / 2);
        return match &mut self.brn {
            QuadBranch::Nothing => {
                panic!("Looking to move bot in an empty node");
            },
            QuadBranch::Branch(b) => {
                let under: &mut QuadNode = if prevx < midx {
                    if prevy < midy {
                        &mut b.0
                    } else {
                        &mut b.3
                    }
                } else {
                    if prevy < midy {
                        &mut b.1
                    } else {
                        &mut b.2
                    }
                };

                if !under.move_bot(bt, prevx, prevy) {
                    // removed bot, but could not add again, because was out of range
                    // add if we can, or pass false up
                    if (bt.1 < self.x) ||
                        (bt.1 >= (self.x + self.w)) ||
                        (bt.2 < self.y) ||
                        (bt.2 >= (self.y + self.h))
                    {
                        false // out of our bounds, can not move it locally
                    } else {
                        // add it below
                        if bt.1 < midx {
                            if bt.2 < midy {
                                b.0.add_bot(bt);
                            } else {
                                b.3.add_bot(bt);
                            }
                        } else {
                            if bt.2 < midy {
                                b.1.add_bot(bt);
                            } else {
                                b.2.add_bot(bt);
                            }
                        }
                        true // we moved it to another of our unders
                    }
                } else {
                    true // under us moved it just fine
                }
            },
            QuadBranch::BotList(bl) => {
                let mut found = false;
                let mut moved = false;
                for i in 0..bl.len() {
                    if bl[i].0 == bt.0 {
                        found = true;
                        if (bt.1 < self.x) ||
                            (bt.1 >= (self.x + self.w)) ||
                            (bt.2 < self.y) ||
                            (bt.2 >= (self.y + self.h))
                        {
                            // can't move, just remove it
                            bl.remove(i);
                        } else {
                            moved = true;
                            bl[i].1 = bt.1;
                            bl[i].2 = bt.2;
                        }
                        break;
                    }
                }
                if !found {
                    panic!("Unable to find bot to remove");
                }

                moved // return if we moved it or not
            }
        }
    }

    fn rm_bot(&mut self, bt: (u32, u32, u32)) {
        let midx = self.x + (self.w / 2);
        let midy = self.y + (self.h / 2);
        match &mut self.brn {
            QuadBranch::Nothing => {
                panic!("Looking to remove bot in an empty node");
            },
            QuadBranch::Branch(b) => {
                if bt.1 < midx {
                    if bt.2 < midy {
                        b.0.rm_bot(bt);
                    } else {
                        b.3.rm_bot(bt);
                    }
                } else {
                    if bt.2 < midy {
                        b.1.rm_bot(bt);
                    } else {
                        b.2.rm_bot(bt);
                    }
                }
            },
            QuadBranch::BotList(bl) => {
                let mut found = false;
                for i in 0..bl.len() {
                    if bl[i].0 == bt.0 {
                        found = true;
                        bl.remove(i);
                        break;
                    }
                }
                if !found {
                    panic!("Unable to find bot to remove");
                }
            }
        }
    }
}

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
    bots: HashMap<u32, BotState>, // Vector must always have lower bot id's ordered with indexes
    //bottree: QuadNode, // QuadTree of bots, used for avoiding and targeting nearby bots //TODO see if this is more performant than occasional search
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
    //dk: f32,
    //ik: f32,
    ratio: f32, // current game seconds to go per real second for drawing to the screen, smooths lag and ticks
    tick: f32,   // where we are displaying in ticks
    targetlag: f32, // how far behind we want to be in ticks
    preverr: f32,
    avgerr: Vec<f32>,
    //avgderr: Vec<f32>,
    //ierr: f32,
}

// display constants and initial values
const DIS_PK: f32 = 0.001;
//const DIS_DK: f32 = 1.2;
//const DIS_IK: f32 = 0.000001;
const DIS_RMIN: f32 = 0.000001;
const DIS_LAG: f32 = 3.9;
const DIS_RMAX: f32 = 0.06;
const DIS_EHIST: usize = 64;

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

// Game constants
const STARTID: u32 = 1;

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
            bots: HashMap::new(),
            /*bottree: QuadNode {
                x: 0,
                y: 0,
                w: self.map.w,
                h: self.map.h,
                brn: QuadBranch::Nothing,
            },*/
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
                tk.bots.insert(self.objidcntr, BotState {
                    id: self.objidcntr,
                    health: 100.0,
                    x: (x * stride) as f32,
                    y: (y * stride) as f32,
                    vx: 0.0,
                    vy: 0.0,
                    cur_target: 0,
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
        
        for (_, bt) in newtk.bots.iter_mut() {
            // add random accel to each bot
            let amt: f32 = rng.gen_range(0.0, BOTRANDMAXACC) * self.tickstep;
            let ang: f32 = rng.gen_range(0.0, f32::consts::PI * 2.0);

            let xpart: f32 = ang.cos() * amt;
            let ypart: f32 = ang.sin() * amt;
            bt.vx += xpart;
            bt.vy += ypart;

            // query paint for force
            //TODO

            // move away from other bots?
            //TODO
            // +1 too a lower rez 2d map that can be used to "roll downhill" away from groups of bots
        }

        for (_, bt) in newtk.bots.iter_mut() {
            // apply separation forces
            //TODO

            // max out vel
            // simpler check than actual speed, but whatever
            if bt.vx > BOTMAXVEL {
                bt.vx = BOTMAXVEL;
            } else if bt.vx < -BOTMAXVEL {
                bt.vx = -BOTMAXVEL;
            }
            if bt.vy > BOTMAXVEL {
                bt.vy = BOTMAXVEL;
            } else if bt.vy < -BOTMAXVEL {
                bt.vy = -BOTMAXVEL;
            }

            // step vel for each bot
            let mut newx = bt.x + (bt.vx * self.tickstep);
            let mut newy = bt.y + (bt.vy * self.tickstep);

            // bounce off walls
            // TODO

            // bounce off edge
            // don't have to be as complicated as walls
            // just don't move, and bounce velocity
            if newx <= 0.0  || newx >= (self.map.w as f32) {
                newx = bt.x;
                bt.vx *= -BOTBOUNCEAMT;
            }
            if newy <= 0.0  || newy >= (self.map.h as f32) {
                newy = bt.y;
                bt.vy *= -BOTBOUNCEAMT;
            }

            bt.x = newx;
            bt.y = newy;
        }

        // clean old tick info not needed for drawing        
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

        // step through both ticks for bots
        for (id, bt1) in tk1.bots.iter() {
            if let Some(bt2) = tk2.bots.get(id) {
                // lerp
                let x: f32 = (lerpfac * (bt2.x - bt1.x)) + bt1.x;
                let y: f32 = (lerpfac * (bt2.y - bt1.y)) + bt1.y;

                self.ctx.begin_path();
                self.ctx.arc(x as f64, y as f64, BOTRAD, 0.0, f64::consts::PI * 2.0).expect("Unable to draw bot");
                self.ctx.fill();
            } // else must be removed by next tick
            //TODO explosion or something?
        }

        // draw bases
        //TODO

        // draw map features
        //TODO

        // adjust the ratio to even out
        let err = ((self.curtick as f32) - self.dis.targetlag) - self.dis.tick;
        //self.dis.ierr += err;
        //let derr = (err - self.dis.preverr) / dt;
        self.dis.preverr = err;
        self.dis.avgerr.push(err);
        //self.dis.avgderr.push(derr);
        if self.dis.avgerr.len() > DIS_EHIST {
            self.dis.avgerr.remove(0);
            //self.dis.avgderr.remove(0);
        }

        let err = self.dis.avgerr.iter().sum::<f32>() / (self.dis.avgerr.len() as f32);
        //let derr = self.dis.avgderr.iter().sum::<f32>() / (self.dis.avgderr.len() as f32);
        
        self.dis.ratio = self.dis.pk * err;
            //+ (self.dis.dk * derr)
            //+ (self.dis.ik * self.dis.ierr);
        if self.dis.ratio <= DIS_RMIN {
            self.dis.ratio = DIS_RMIN;
        } else if self.dis.ratio >= DIS_RMAX {
            self.dis.ratio = DIS_RMAX;
        }

        
        //DEBUG
        log(&format!(
            "cur: {}\ndisp: {}\nerr: {}\ndt: {}\nratio: {}",
            self.curtick,
            self.dis.tick,
            err,
            dt,
            self.dis.ratio,
        )[..]);
        

        ////DEBUG
        //if disp1 == self.curtick {
        //    log(&format!("WARNING, RAILED {}", disp1)[..])
        //}

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
                //dk: DIS_DK,
                //ik: DIS_IK,
                ratio: dispratio,
                tick: 0.0,
                targetlag: DIS_LAG,
                preverr: 0.0,
                avgerr: Vec::new(),
                //avgderr: Vec::new(),
                //ierr: 0.0,
            },
            map: GameMap::new(mapw, maph),
            usermap: GameMap::new(mapw, maph),
            ctx,
            canvas,
            baseseed: seed,
            objidcntr: STARTID,
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
pub fn adj_dis(pk: f32, targetlag: f32) {
    GAME.with(|g| {
        if let Some(game) = &mut *g.borrow_mut() {
            let di = &mut game.dis;
            di.pk = pk;
            //di.dk = dk;
            //di.ik = ik;
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
