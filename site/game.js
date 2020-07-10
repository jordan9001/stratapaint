'use strict';
const canid = "canvas";

var prevts = 0;
var avdt = [];
var prevtick = 0;
var avtick = [];
var mapimg = undefined;
var can = document.getElementById(canid);
var ctx = can.getContext("2d");

function dodraw(ts) {
    // draw the game
    var dt = ts - prevts;
    prevts = ts;
    if (dt <= 0) {
        dt = 0;
    }

    avdt.push(dt);
    if (avdt.length > 64) {
        avdt.shift();
    }

    if (avtick.length != 0) {
        console.log("" + dt + "\navg draw: " + avdt.reduce(function(a,b) { return a + b; }) / avdt.length + "\navg tick: ", avtick.reduce(function(a,b) { return a + b; }) / avtick.length);
    }

    // clear the canvas
    ctx.save();
    ctx.setTransform(1,0,0,1,0,0);
    ctx.clear_rect(0.0, 0.0, can.width, can.height);
    ctx.restore();

    // let the engine update it's stuff
    draw();

    // draw the images from the buffers as needed

    // user painting
    //TODO

    // base map image
    ctx.putImageData(mapimg, 0, 0);

    requestAnimationFrame(dodraw);
}

function dotick() {
    var ts = performance.now();
    
    tick();

    var dt = performance.now() - ts;
    prevtick = dt;
    avtick.push(dt);
    if (avtick.length > 64) {
        avtick.shift();
    }
}

function main() {
    console.log("Game loading...");

    let ws = new WebSocket("ws://" + location.host + "/con");
    ws.onopen = function() {
        ws.send("PING");
    };
    ws.onmessage = function(msg) {
        //console.log(msg);
    };
    ws.onclose = function() {
        //console.log("Connection closed!");
    };

    // set up canvas zooming/ moving
    //TODO

    var tick_step = 100;
    var width = 800;
    var height = 800;

    init_game(canid, width, height, 0x100, tick_step, 0);

    // set up a ImageData for the map
    var buf = getbuf(-1);
    var offset = ;
    var len = width * height * 4;
    var mapbuf = new Uint8ClampedArray(buf, offset, len);
    mapimg = new ImageData(mapbuf, width, height);

    // start drawing
    requestAnimationFrame(dodraw);

    // start ticks
    //TODO move tick stuff this to a separate looping callback unaffected by tab being inactive
    setInterval(dotick, tick_step);

    // wasm jobs:
    // game logic 
    // drawing

    // js jobs:
    // websocket communication
    // user input
    // call wasm tick (probably triggered by interval message from webworker, so we don't miss a tick)
    // call draw from requestAnimationFrame
}

// first init webasm and import the symbols we need
import init, { adj_dis, init_game, tick, draw, getbuf } from './clientwasm.js';
(async function() {
	await init();

	main();
})();


//DEBUG
window.adj_dis = adj_dis;
