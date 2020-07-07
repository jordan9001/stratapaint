'use strict';

var prevts = 0;
var avdt = [];
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

    draw(dt);

    requestAnimationFrame(dodraw);
}

var prevtick = 0;
var avtick = [];
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

    var tick_step = 150;

    init_game("canvas", 900, 900, 0x100, tick_step, 0);

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
import init, { adj_dis, init_game, tick, draw } from './clientwasm.js';
(async function() {
	await init();

	main();
})();


//DEBUG
window.adj_dis = adj_dis;
