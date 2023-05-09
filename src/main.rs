use std::{os::unix::prelude::PermissionsExt, process::Output, time::{Instant, Duration}, thread::{Thread, self}, fmt::Display, any::TypeId};
use futures::stream::{self, StreamExt};

use actix::prelude::*;
use anyhow;
use futures_core::{future::BoxFuture, Future};
use futures_util::{SinkExt, io::Close};

#[derive(Debug, MessageResponse)]
struct MyReturn {
    message: String,
}

#[derive(Debug, Message, Clone)]
#[rtype(result = "MyReturn")]
struct MyMessage {
    count: usize,
    message: String,
}

#[derive(Debug, Message)]
#[rtype(result = "anyhow::Result<()>")]
struct Subscribe(Recipient<MyMessage>); 

#[derive(Debug, Message)]
#[rtype(result = "()")]
struct NotifyAll(MyMessage); 



#[derive(Debug)]
struct MyActor {
    age: usize,
    name: String,
}

impl Actor for MyActor {
    type Context = Context<Self>;
}

impl Handler<MyMessage> for MyActor {
    type Result = MyReturn;

    fn handle(&mut self, msg: MyMessage, _ctx: &mut Self::Context) -> Self::Result {
        let result = format!("age = {}, name = {}, count = {}, message = {}", self.age, self.name, msg.count, msg.message);
        println!("{}", result);
        self.age += 1;
        MyReturn {
            message: result,
        }
    }
}

#[derive(Debug)]
struct MyActor2 {
    age: usize,
    name: String,
}

impl Actor for MyActor2 {
    type Context = Context<Self>;
}

impl Handler<MyMessage> for MyActor2 {
    type Result = MyReturn;

    fn handle(&mut self, msg: MyMessage, _ctx: &mut Self::Context) -> Self::Result {
        let result = format!("actor2, 2age = {}, name = {}, count = {}, message = {}", self.age, self.name, msg.count, msg.message);
        println!("{}", result);
        self.age += 1;
        MyReturn {
            message: result,
        }
    }
}

struct MyEvent  {
    subscribers: Vec<Recipient<MyMessage>>,
}

impl Actor for MyEvent {
    type Context = Context<Self>;
}

impl Handler<Subscribe> for MyEvent {
    type Result = anyhow::Result<()>;

    fn handle(&mut self, msg: Subscribe, _ctx: &mut Self::Context) -> Self::Result {
        self.subscribers.push(msg.0);
        Ok(())
    }
}

impl Handler<NotifyAll> for MyEvent {
    type Result = ();

    fn handle(&mut self, msg: NotifyAll, _ctx: &mut Self::Context) -> Self::Result {
        self.notify(msg.0.clone());
        ()
    }
}

impl MyEvent {
    fn notify(&self, message: MyMessage) {
        self.subscribers.iter().for_each(|suber| {
            let result = suber.try_send(message.clone());
            println!("send result: {:?}", result);
            println!("notify, thread id = {:?}", std::thread::current().id());
        })
    }    
}


fn test_actix() {
    let system = actix::prelude::System::new();

    let fut = async {
        let addr = Subscribe(MyActor {
            age: 40,
            name: String::from("jack"),
        }.start().recipient());

        let addr2 = Subscribe(MyActor2 {
            age: 38,
            name: String::from("rose"),
        }.start().recipient());

        let dispatcher = MyEvent{
            subscribers: vec![],
        }.start();

        dispatcher.do_send(addr);
        dispatcher.do_send(addr2);
        dispatcher.do_send(NotifyAll(MyMessage { count: 12, message: String::from("jack"), }));
    };

    let arbiter = Arbiter::new();
    arbiter.spawn(fut);

    let arbiter2 = Arbiter::new();
    arbiter2.spawn(async {
        println!("when will it be printed?, thread id = {:?}", std::thread::current().id());
    });

    system.run().unwrap();
}

struct CountingTask {
    nunmber: u64,
    when: Instant,
}

impl CountingTask {
    pub fn new(n: u64) -> Self {
        CountingTask { nunmber: n, when: Instant::now() }
    }    
}

impl futures::Future for CountingTask {
    type Output = u64;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        if self.as_ref().nunmber <= (Instant::now() - self.when).as_secs() {
            return std::task::Poll::Ready(self.as_ref().nunmber);
        }
        let timeout = self.as_ref().nunmber;
        let waker = cx.waker().clone();
        tokio::spawn(async move {
            thread::sleep(Duration::from_secs(timeout));
            waker.wake();
        });
        std::task::Poll::Pending
    }
}

struct TaskStream {
    number: u64,
    current: u64,
}

impl TaskStream {
    pub fn new(n: u64) -> Self {
        TaskStream { number: n, current: 0 }
    } 
}

impl futures::Stream for TaskStream {
    type Item = CountingTask;

    fn poll_next(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Self::Item>> {
        let current = self.as_ref().current;
        if  current < self.as_ref().number {
            let task = CountingTask::new(current);
            let s = self.get_mut();
            s.current += 1;
            return std::task::Poll::Ready(Some(task));
        }
        std::task::Poll::Ready(None)
    }
}

#[derive(Debug)]
struct MySink;

impl MySink {
    pub fn new() -> Self {
        MySink
    } 
}

impl<Item> futures::Sink<Item> for MySink where Item: std::fmt::Debug {
    type Error = anyhow::Error;

    fn poll_ready(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), Self::Error>> {
        println!("poll ready");
        std::task::Poll::Ready(Ok(()))
    }

    fn start_send(self: std::pin::Pin<&mut Self>, item: Item) -> Result<(), Self::Error> {
        println!("start send, {:?}",  item);
        Ok(())
    }

    fn poll_flush(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn poll_close(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), Self::Error>> {
        println!("poll close");
        std::task::Poll::Ready(Ok(()))
    }
}

async fn test_stream() {
    let stream = TaskStream::new(3);
    let result = stream.buffered(1024).collect::<Vec<u64>>();
    let result = result.await;

    println!("result = {:?}", result);
}

async fn test_stream_vec() {
    let myvec = vec![101, 102, 103];
    let mut v = futures::stream::iter(myvec.into_iter()).collect::<Vec<i32>>();
    println!("v = {:?}", v.await);
}

async fn test_sink() {
    let stream = TaskStream::new(3);
    let mut mysink = MySink::new();
    let mut iter = stream.buffered(1024).map(|item|Ok(item));
    let result = mysink.send_all(&mut iter).await;
    println!("{:?}", result);
}

#[tokio::main]
async fn main() {
    // test_stream().await;
    // test_stream_vec().await;
    // test_sink().await;
}