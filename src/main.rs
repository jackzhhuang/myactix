use std::os::unix::prelude::PermissionsExt;

use actix::prelude::*;
use anyhow;

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


fn main() {
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
