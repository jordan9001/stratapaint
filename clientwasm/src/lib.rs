use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use std::cell::RefCell;
use std::f64;
use std::f32;
use rand_xorshift::XorShiftRng;
use rand::SeedableRng;
use rand::Rng;
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
    team: i32,
    cur_target: u32,
}

// bot constants
const BOTRAD: f64 = 1.2;
const BOTRANDMAXACC: f32 = 15.0;
const BOTMAXVEL: f32 = 42.0;
const BOTBOUNCEAMT: f32 = 0.42;
const BOTCOLSZ: f32 = (BOTRAD as f32) * 0.15;

#[derive(Clone)]
struct LocationGroups {
    groupw: u32,
    grouph: u32,
    vecs: Vec<Vec<u32>>,
}

// LocationGroup constants
const GROUPSHIFT: u32 = 5;

impl LocationGroups {
    fn new(mapw: u32, maph: u32) -> LocationGroups {
        let groupw = (mapw >> GROUPSHIFT)+1;
        let grouph = (maph >> GROUPSHIFT)+1;
        let mut vecs: Vec<Vec<u32>> = Vec::new();
        for _ in 0..(groupw * grouph) {
            vecs.push(Vec::new());
        }
        LocationGroups {
            groupw,
            grouph,
            vecs,
        }
    }

    fn add_bot(&mut self, id: u32, x: u32, y: u32) {
        let xgroup = x >> GROUPSHIFT;
        let ygroup = y >> GROUPSHIFT;

        self.vecs[(xgroup + (ygroup * self.groupw)) as usize].push(id);
    }

    fn rm_bot(&mut self, id: u32, x: u32, y: u32) {
        let xgroup = x >> GROUPSHIFT;
        let ygroup = y >> GROUPSHIFT;

        let v = &mut self.vecs[(xgroup + (ygroup * self.groupw)) as usize];

        for i in 0..v.len() {
            if v[i] == id {
                v.remove(i);
                return;
            }
        }

        panic!("Didn't find bot where it was supposed to be in LocationGroups!");
    }

    fn move_bot(&mut self, id: u32, old_x: u32, old_y: u32, new_x: u32, new_y: u32) {
        if ((old_x >> GROUPSHIFT) != (new_x >> GROUPSHIFT)) || ((old_y >> GROUPSHIFT) != (new_y >> GROUPSHIFT)) {
            self.rm_bot(id, old_x, old_y);
            self.add_bot(id, new_x, new_y);
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

#[derive(PartialEq,Eq)]
#[derive(Clone,Copy)]
#[repr(packed)]
struct Px {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

impl Px {
    const White: Px = Px{
        r: 0xff,
        g: 0xff,
        b: 0xff,
        a: 0xff,
    };
    const Black: Px = Px{
        r: 0,
        g: 0,
        b: 0,
        a: 0xff,
    };
    const Grey: Px = Px{
        r: 0x80,
        g: 0x80,
        b: 0x80,
        a: 0xff,
    };
    const Clear: Px = Px{
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    };
}

#[derive(Clone)]
struct GameMap {
    w: u32,
    h: u32,
    bytes: Box<[Px]>, // can be color, or for base map it is terrain info
}

impl GameMap {
    fn new(w: u32, h: u32) -> GameMap {
        GameMap{
            w,
            h,
            bytes: (vec![Px::White; (w * h) as usize]).into_boxed_slice(),
        }
    }

    fn set_tile(&mut self, x: u32, y: u32, tile: MapTiles) {
        let p: Px = tile.into();
        self.set(x, y, p);
    }

    fn set(&mut self, x: u32, y: u32, color: Px) {
        let ind = (x + (y * self.w)) as usize;
        self.bytes[ind] = color;
    }
}

enum MapTiles {
    Wall,
    Nothing,
    Unk,
}

impl From<Px> for MapTiles {
    fn from(orig: Px) -> Self {
        match orig {
            Px::White => MapTiles::Nothing,
            Px::Black => MapTiles::Wall,
            _ => MapTiles::Unk,
        }
    }
}

impl From<MapTiles> for Px {
    fn from(orig: MapTiles) -> Self {
        match orig {
            MapTiles::Nothing => Px::White,
            MapTiles::Wall => Px::Black,
            MapTiles::Unk => Px::Clear,
        }
    }
}

// game tick structure
// has to contain the current paint layers, and the current bot states, the base states
// anything needed for the display needs to be here, because it can lag
// and anything that can come from the "future" from the network: other player's paints
#[derive(Clone)]
struct GameTick {
    tick: u32,
    bases: Vec<BaseState>,
    bots: HashMap<u32, RefCell<BotState>>, // Vector must always have lower bot id's ordered with indexes
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
    avgerr: Vec<f32>,
    avgerrsum: f32,
    //preverr: f32,
    //avgderr: Vec<f32>,
    //ierr: f32,
    //dk: f32,
    //ik: f32,
    targetlag: f32, // how far behind we want to be in ticks
    ratio: f32, // current game seconds to go per real second for drawing to the screen, smooths lag and ticks
    tick: f32,   // where we are displaying in ticks
    
    //TODO add in particle effects tracking here for shooting
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
    bottree: LocationGroups, // collection of bots in curtick, used for avoiding and targeting nearby bots
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
const MAXCHECK: u32 = 9;

impl Game {
    fn add_bot(&mut self, tk: &mut GameTick, x: f32, y: f32, id: u32, team: i32) {
        tk.bots.insert(id, RefCell::new(BotState {
            id,
            health: 100.0,
            x,
            y,
            vx: 0.0,
            vy: 0.0,
            cur_target: 0,
            team,
        }));
        tk.teambotcount[team as usize] += 1;

        // add to tree
        self.bottree.add_bot(id, x as u32, y as u32);
    }


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
        let mut rng = XorShiftRng::seed_from_u64((self.baseseed) as u64);

        // add a couple of random walls
        for _ in 0..3 {
            let x: u32 = rng.gen_range(0, self.map.w - 1);
            let ystart: u32 = rng.gen_range(0,self.map.h - 30);
            let yend: u32 = rng.gen_range(ystart+1, self.map.h);
            for y in ystart..yend {
                self.map.set_tile(x, y, MapTiles::Wall);
            }
        }

        for _ in 0..3 {
            let y: u32 = rng.gen_range(0, self.map.w - 1);
            let xstart: u32 = rng.gen_range(0,self.map.w - 30);
            let xend: u32 = rng.gen_range(xstart+1, self.map.w);
            for x in xstart..xend {
                self.map.set_tile(x, y, MapTiles::Wall);
            }
        }

        for _ in 0..1000 {
            self.add_bot(
                &mut tk, ((self.map.w as f32)/2.0) + rng.gen_range(-(self.map.w as f32)/3.0, (self.map.w as f32)/3.0),
                ((self.map.h as f32)/2.0) + rng.gen_range(-(self.map.h as f32)/3.0, (self.map.h as f32)/3.0),
                self.objidcntr,
                0,
            );
            self.objidcntr += 1;
        }

        // spawn a bunch in the middle

        for _ in 0..500 {
            self.add_bot(
                &mut tk,
                ((self.map.w as f32)/2.0) + rng.gen_range(-20.0, 20.0),
                ((self.map.h as f32)/2.0) + rng.gen_range(-20.0, 20.0),
                self.objidcntr,
                0,
            );
            self.objidcntr += 1;
        }

        // play around with starting velocities
        for b in tk.bots.values_mut() {
            let b = &mut*b.borrow_mut();
            b.vx = rng.gen_range(-BOTMAXVEL, BOTMAXVEL);
            b.vy = rng.gen_range(-BOTMAXVEL, BOTMAXVEL);
        }

        log(&format!("starting out with {} bots", self.objidcntr)[..]);

        self.states.push(tk);
    }

    fn tick(&mut self) {
        //DEBUG
        //let newtk: &mut GameTick = &mut self.states[0];


        let mut newtk: GameTick = self.get_cur_tick().clone();
        let oldtick = newtk.tick; // save old tick so we can clean it at the end
        newtk.tick += 1;

        // get prng for this tick
        let mut rng = XorShiftRng::seed_from_u64((self.baseseed + newtk.tick) as u64);
        
        for k in newtk.bots.keys() {
            // add random accel to each bot
            let amt: f32 = rng.gen_range(0.0, BOTRANDMAXACC) * self.tickstep;
            let ang: f32 = rng.gen_range(0.0, f32::consts::PI * 2.0);

            let xpart: f32 = ang.cos() * amt;
            let ypart: f32 = ang.sin() * amt;

            let bt = newtk.bots.get(k).unwrap();
            let bt = &mut*bt.borrow_mut(); //TODO use Cell instead of refcell because it is copy
            bt.vx += xpart;
            bt.vy += ypart;

            // query paint for force
            //TODO

            // loop through local bots
            // just for avoidance for now

            let xmin = bt.x - BOTCOLSZ;
            let ymin = bt.y - BOTCOLSZ;
            let xmax = bt.x + BOTCOLSZ;
            let ymax = bt.y + BOTCOLSZ;

            let xmin = if xmin <= 0.0 {
                0
            } else {
                (xmin as u32) >> GROUPSHIFT
            };
            let ymin = if ymin <= 0.0 {
                0
            } else {
                (ymin as u32) >> GROUPSHIFT
            };
            let xmax = (xmax as u32) >> GROUPSHIFT;
            let xmax = if xmax >= self.bottree.groupw {
                self.bottree.groupw
            } else {
                xmax+1
            };
            let ymax = (ymax as u32) >> GROUPSHIFT;
            let ymax = if ymax >= self.bottree.grouph {
                self.bottree.grouph
            } else {
                ymax+1
            };

            
            // push apart close bots
            for xg in xmin..xmax {
                for yg in ymin..ymax {
                    let mut numcheck = 0;
                    for id2 in &self.bottree.vecs[(xg + (yg * self.bottree.groupw)) as usize] {
                        // don't do the same work twice
                        if id2 <= k {
                            continue;
                        }
                        numcheck += 1;
                        if numcheck > MAXCHECK {
                            break;
                        }

                        let bt2 = newtk.bots.get(id2).unwrap();
                        let bt2 = &mut*bt2.borrow_mut();

                        // test boids

                        
                        let dx = bt.x - (*bt2).x;
                        let dy = bt.y - (*bt2).y;

                        if dx > 0.0 && dx < BOTCOLSZ {
                            if bt.vx < 0.0 {
                                bt.vx *= -BOTBOUNCEAMT;
                            }
                            if bt2.vx > 0.0 {
                                bt2.vx *= -BOTBOUNCEAMT;
                            }
                        } else if dx < 0.0 && dx > -BOTCOLSZ {
                            if bt.vx > 0.0 {
                                bt.vx *= -BOTBOUNCEAMT;
                            }
                            if bt2.vx < 0.0 {
                                bt2.vx *= -BOTBOUNCEAMT;
                            }
                        }

                        if dy > 0.0 && dy < BOTCOLSZ {
                            if bt.vy < 0.0 {
                                bt.vy *= -BOTBOUNCEAMT;
                            }
                            if bt2.vy > 0.0 {
                                bt2.vy *= -BOTBOUNCEAMT;
                            }
                        } else if dy < 0.0 && dy > -BOTCOLSZ {
                            if bt.vy > 0.0 {
                                bt.vy *= -BOTBOUNCEAMT;
                            }
                            if bt2.vy < 0.0 {
                                bt2.vy *= -BOTBOUNCEAMT;
                            }
                        }
                        
                    }
                }
            }

            // max out vel
            // just use a simple P-Inf norm, instead of doing any sqrt for now
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

            // move the bot
            self.bottree.move_bot(bt.id, bt.x as u32, bt.y as u32, newx as u32, newy as u32);
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

        // do big cleanup
        // clean up old ticks not needed anymore
        let mut disp1 = self.dis.tick as u32;
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

    fn draw(&mut self, dt: f32) {
        
        // don't call the canvas API, just fill out the data in a buffer used by a ImageData(buf, width, height) on the js
        //https://www.hellorust.com/demos/canvas/index.html
        
        // get target ticks to lerp between
        let mut disp2 = self.dis.tick.ceil() as u32;
        if disp2 > self.curtick {
            self.dis.tick = self.curtick as f32;
            disp2 = self.curtick;
        }
        let disp1 = self.dis.tick as u32;
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

        //DEBUG
        //let tk1 = self.get_cur_tick();
        //let tk2 = self.get_cur_tick();
        
        // clear canvas
        // save transform
        //self.ctx.save();
        // reset transform
        //self.ctx.set_transform(1.0, 0.0, 0.0, 1.0, 0.0, 0.0).expect("Unable to set transform for clearing");
        // clear everything
        //self.ctx.clear_rect(0.0, 0.0, self.canvas.width() as f64, self.canvas.height() as f64);
        // restore transform
        //self.ctx.restore();

        // draw bots
        self.ctx.set_fill_style(&JsValue::from_str("#fa110e"));

        // step through both ticks for bots
        for (id, bt1) in tk1.bots.iter() {
            if let Some(bt2) = tk2.bots.get(id) {

                let bt1 = bt1.borrow();
                let bt2 = bt2.borrow();
                // lerp
                let x: f32 = (lerpfac * (bt2.x - bt1.x)) + bt1.x;
                let y: f32 = (lerpfac * (bt2.y - bt1.y)) + bt1.y;

                self.ctx.begin_path();
                self.ctx.arc(x as f64, y as f64, BOTRAD, 0.0, f64::consts::PI * 2.0).expect("Unable to draw bot");
                self.ctx.fill();

                //DEBUG
                //self.ctx.begin_path();
                //self.ctx.rect((x - (BOTCOLSZ/2.0)) as f64, (y - (BOTCOLSZ/2.0)) as f64, BOTCOLSZ as f64, BOTCOLSZ as f64);
                //self.ctx.stroke();
            } // else must be removed by next tick
            //TODO explosion or something?
        }

        // draw bases
        //TODO

        // Map get drawn from the buffer directly

        // adjust the ratio to even out
        let err = ((self.curtick as f32) - self.dis.targetlag) - self.dis.tick;
        self.dis.avgerrsum += err;
        self.dis.avgerr.push(err);
        let elen = self.dis.avgerr.len();
        if elen > DIS_EHIST {
            self.dis.avgerrsum -= self.dis.avgerr.remove(0);
        }

        let err = self.dis.avgerrsum / (elen as f32);
        
        self.dis.ratio = self.dis.pk * err;
        if self.dis.ratio <= DIS_RMIN {
            self.dis.ratio = DIS_RMIN;
        } else if self.dis.ratio >= DIS_RMAX {
            self.dis.ratio = DIS_RMAX;
        }

        //DEBUG
        if disp1 == self.curtick {
            log(&format!("WARNING, RAILED {}", disp1)[..])
        }

        // move our displayed tick
        self.dis.tick += dt * self.dis.ratio;
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
            bottree: LocationGroups::new(mapw, maph),
            tickratio: tick_ratio,
            tickstep,
            curtick: 0,
            dis: DisplayInfo {
                pk: DIS_PK,
                //dk: DIS_DK,
                //ik: DIS_IK,
                //preverr: 0.0,
                //avgderr: Vec::new(),
                //ierr: 0.0,
                ratio: dispratio,
                tick: 0.0,
                targetlag: DIS_LAG,
                avgerr: Vec::new(),
                avgerrsum: 0.0,
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
