




function main() {
    console.log("Game loading...");

    let ws = new WebSocket("ws://" + location.host + "/con");
    ws.onopen = function() {
        ws.send("PING");
    };
    ws.onmessage = function(msg) {
        console.log(msg);
    }

    greet();

    init_game("canvas", 1000, 1000, 0x100, 0.2);

    draw(0.0);

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
import init, { greet, init_game, draw } from './clientwasm.js';
(async function() {
	await init();

	main();
})();
