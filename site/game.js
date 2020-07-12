'use strict';
const canid = "canvas";

var prevts = 0;
var avdt = [];
var prevtick = 0;
var avtick = [];
var mapimg = undefined;
var can = document.getElementById(canid);
var ctx = can.getContext("2d");
ctx.imageSmoothingEnabled = false;
var can2 = undefined;
var ctx2 = undefined;

var dispscale=1.0;
var dispxoff = 0;
var dispyoff = 0;

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
        //console.log("" + dt + "\navg draw: " + avdt.reduce(function(a,b) { return a + b; }) / avdt.length + "\navg tick: ", avtick.reduce(function(a,b) { return a + b; }) / avtick.length);
    }

    // clear the canvas
    ctx.save();
    ctx.setTransform(1,0,0,1,0,0);
    ctx.clearRect(0.0, 0.0, can.width, can.height);
    ctx.restore();

    // put the image data on our separate canvas
    ctx2.putImageData(mapimg, 0, 0);
    ctx.drawImage(can2, 0, 0);

    // let the engine update it's stuff
    draw(dt);

    // user painting
    //TODO

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

function main(mem) {
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
    var buf = get_buf(-1);
    if (buf == 0) {
        console.log("Got null Map Buffer");
        return;
    }

    // set up needed image buffers
    var len = width * height * 4;
    var mapbuf = new Uint8ClampedArray(mem.buffer, buf, len);
    mapimg = new ImageData(mapbuf, width, height);

    can2 = document.createElement('canvas');
    can2.id = "can2";
    can2.width = width;
    can2.height = height;
    ctx2 = can2.getContext("2d");

    // set up camera transform callbacks
    //TODO
    can.onwheel = function(evt) {
        var y = evt.deltaY;
        dispscale += y * 0.001;
        ctx.setTransform(dispscale, 0, 0, dispscale, dispxoff, dispyoff);
        return false;
    }
    window.onkeydown = function(evt) {
        console.log(evt);
        var camchanged = false;
        switch (evt.key) {
            case "ArrowDown":
                dispyoff -= 15;
                camchanged = true;
                break;
            case "ArrowUp":
                dispyoff += 15;
                camchanged = true;
                break;
            case "ArrowRight":
                dispxoff -= 15;
                camchanged = true;
                break;
            case "ArrowLeft":
                dispxoff += 15;
                camchanged = true;
                break;
        }

        if (camchanged) {
            ctx.setTransform(dispscale, 0, 0, dispscale, dispxoff, dispyoff);
            return false;
        } else {
            return true;
        }
        
    }

    // set up user painting callbacks
    //TODO

    // start drawing
    requestAnimationFrame(dodraw);

    // start ticks
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
import init, { adj_dis, init_game, tick, draw, get_buf } from './clientwasm.js';
(async function() {
    var wasm = await init();
    //console.log(wasm);
    //console.log(typeof(wasm));

	main(wasm.memory);
})();


//DEBUG
window.adj_dis = adj_dis;
window.dispxoff = dispxoff;
window.dispyoff = dispyoff;
window.dispscale = dispscale;
