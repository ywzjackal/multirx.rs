#![feature(mpsc_select)]
#![allow(dead_code)]
use std::sync::mpsc::{channel, Sender, Receiver, SendError};
use std::thread;
use std::io;

pub enum Event<T>
    where T: std::marker::Send + std::marker::Copy
{
    Join(Sender<T>),
    Exit,
}

#[derive(Debug)]
pub struct MultiRx<T>
    where T: std::marker::Send + std::marker::Copy + 'static
{
    event: Sender<Event<T>>,
    user_tx: Sender<T>,
}

impl<T> MultiRx<T>
    where T: std::marker::Send + std::marker::Copy
{
    /// Create Empty MultiRx Struct
    pub fn new() -> io::Result<MultiRx<T>> {
        let (event_tx, event_rx) = channel();
        let (user_tx, user_rx) = channel();
        let rt = MultiRx {
            event: event_tx,
            user_tx: user_tx,
        };
        try!(thread::Builder::new()
            .name("MultiRx Runtime".to_string())
            .spawn(move || runtime(event_rx, user_rx)));
        Ok(rt)
    }
    /// Clone an TX handle.
    pub fn clone_tx(&self) -> Sender<T> {
        self.user_tx.clone()
    }
    /// Join Channel RX Group.
    pub fn join_rx(&self) -> Result<Receiver<T>, SendError<Event<T>>> {
        let (tx, rx) = channel();
        try!(self.event.send(Event::Join(tx)));
        Ok(rx)
    }
}

fn runtime<T>(rx: Receiver<Event<T>>, user_rx: Receiver<T>)
    where T: std::marker::Send + std::marker::Copy
{
    let mut children = Vec::new();
    loop {
        select!{
            rt = rx.recv() => {
                match rt {
                    Ok(evt) => {
                        match evt {
                            Event::Exit => {
                                println!("runtime exit, got exit event.");
                                return;
                            },
                            Event::Join(tx) => {
                                children.push(tx);
                            }
                        }
                    },
                    Err(_) => {
                        //println!("fail to recv runtime event, {:?}", e);
                        return;
                    }
                }
            },
            rt = user_rx.recv() => {
                match rt {
                    Ok(evt) => {
                        let mut index = 0;
                        let mut remove_index = Vec::new();
                        for tx in children.iter() {
                            match tx.send(evt) {
                                Ok(()) => {index += 1;},
                                Err(e) => {
                                    remove_index.push(index);
                                    println!("fail to send event to user, sender removed, {:?}", e);
                                }
                            }
                        }
                        for id in remove_index {
                            children.remove(id);
                        }
                    },
                    Err(_) => {
                        //println!("fail to recv user event, {:?}", e);
                        return;
                    }
                }
            } 
        }
    }
}

#[test]
fn test() {
    use std::sync::Arc;
    use std::time::Duration;
    use std::sync::atomic::{AtomicUsize, Ordering};
    let mrx = MultiRx::new().unwrap();
    let tx = mrx.clone_tx();
    let counter = Arc::new(AtomicUsize::new(0));
    for i in 0..20 {
        match mrx.join_rx() {
            Ok(rx) => {
                let c = counter.clone();
                thread::spawn(move || {
                    match rx.recv() {
                        Err(e) => {
                            println!("Fail to recv user event! {:?}", e);
                            return;
                        }
                        Ok(_) => {
                            c.fetch_add(1, Ordering::SeqCst);
                            println!("index:{:03}, thread {:03}", c.load(Ordering::SeqCst), i);
                        }
                    }
                    drop(rx);
                });
            }
            Err(e) => {
                println!("Fail to join rx! {:?}", e);
            }
        }
    }
    tx.send(1).unwrap();
    tx.send(1).unwrap();
    thread::sleep(Duration::from_millis(100));
    assert_eq!(counter.load(Ordering::SeqCst), 20);
}