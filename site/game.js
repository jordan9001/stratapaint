'use strict';

var prevts = 0;
function dodraw(ts) {
    // draw the game
    var dt = ts - prevts;
    prevts = ts;
    if (dt <= 0) {
        dt = 0;
    }
    //console.log("draw : ", dt);
    
    //dotick();
    draw(dt);

    requestAnimationFrame(dodraw);
}

var prevtick = 0;
function dotick() {
    /*
    var ts = performance.now();
    var dt = ts - prevtick;
    prevtick = ts;
    if (dt <= 0) {
        dt = 0;
    }
    console.log("tick : ", dt);
    */
    tick();
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
