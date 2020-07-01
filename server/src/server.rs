use warp::Filter;
use warp::filters::ws::{WebSocket, Message};
use futures::StreamExt;
use futures::SinkExt;

macro_rules! SITEPATH { () => { "./site/" }; }
const PORT: u16 = 8910;

async fn new_user(wsock: WebSocket) {
    println!("Got New User!");
    // get the ping, send back a pong, close connection

    let (mut wtx, mut wrx) = wsock.split();
    
    if let Some(res) = wrx.next().await {
        match res {
            Ok(msg) => {
                if let Ok(s) = msg.to_str() {
                    println!("Got message from websocket: {}", s);
                } else {
                    println!("Got a binary message");
                }
            },
            Err(e) => {
                println!("Got Error from websocket: {}", e);
            }
        }
    }


    // another option here is to 
    if let Err(e) = wtx.send(Message::text("PONG")).await {
        println!("Could not send message, disconnected: {}", e);
    }

    //TODO lots of stuff
    // example session
    //  client : server
    //  -> join game
    //  <- game info (map inital layout, # players ready, x Steps to 1 NetStep)
    //  -> ready
    //  <- NetStep 0 updates (probably empty)
    //  -> Ack NetStep0
    //  -> queue Update
    //  <- NetStep 1 updates (should contain queued updates)
    //  <- NetStep 2 updates

    // so I need a task per client handling recving data, and send it into a chan to join or create a game
    // and I will have a main task that can will broker out to games
    // and I will have a task per game that will pass handle player input and send out updates
}

#[tokio::main]
async fn main() {
    let wspath = warp::path("con")
        .and(warp::ws())
        .map(|ws: warp::ws::Ws| {
            ws.on_upgrade(new_user)
        });
    let site = warp::fs::dir(SITEPATH!());

    let routes = wspath.or(site);

    //TODO start task for game broker
    // (which will start tasks for each game)
    // pass chan reference to each wspath client

    println!("Serving on {}", PORT);
    warp::serve(routes).run(([0,0,0,0], PORT)).await;
}
