




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
import init, { greet } from './clientwasm.js';
(async function() {
	await init();

	main();
})();
