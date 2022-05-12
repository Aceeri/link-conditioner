use std::{
    io,
    net::{SocketAddr, ToSocketAddrs},
    ops::Add,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use time_queue::TimeQueue;

pub mod time_queue;

pub fn instant(config: &ConditionerConfig) -> Instant {
    let mut instant = Instant::now().add(config.latency);

    let jitter_percent = rand::random::<f32>(); // 0.0 .. 1.0 range
    let jitter = config.jitter.mul_f32(jitter_percent);
    let positive_jitter = rand::random::<bool>(); // true -> positive, false -> negative
    if positive_jitter {
        instant = instant.checked_add(jitter).unwrap_or(instant);
    } else {
        instant = instant.checked_sub(jitter).unwrap_or(instant);
    };
    //println!("positive: {:?}, jitter: {:?}", positive_jitter, jitter);

    instant
}

pub fn keep_packet(config: &ConditionerConfig) -> bool {
    let n = rand::random::<f32>();
    println!("{} < {}", n, config.packet_loss);
    !(n < config.packet_loss)
}

/// Thin wrapper around a `SocketLike` to provide mock testing of packet loss/latency.
pub struct Conditioner<S> {
    pub config: ConditionerConfig,
    socket: S,
    queue: Arc<Mutex<TimeQueue<RecvFrom>>>,
}

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct RecvFrom {
    pub addr: SocketAddr,
    pub data: Vec<u8>,
}

pub struct ConditionerConfig {
    pub latency: Duration,
    pub jitter: Duration,
    pub packet_loss: f32,
}

impl Default for ConditionerConfig {
    fn default() -> Self {
        Self {
            latency: Duration::ZERO,
            jitter: Duration::ZERO,
            packet_loss: 0.0,
        }
    }
}

pub trait SocketLike {
    fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()>;
    fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
        match self.recv_from(buf) {
            Ok((n, _)) => Ok(n),
            Err(err) => Err(err),
        }
    }
    fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)>;
    fn send(&self, buf: &[u8]) -> io::Result<usize>;
    fn send_to<A: ToSocketAddrs>(&self, buf: &[u8], addr: A) -> io::Result<usize>;
}

impl SocketLike for std::net::UdpSocket {
    fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.set_nonblocking(nonblocking)
    }
    fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.recv(buf)
    }
    fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.recv_from(buf)
    }
    fn send(&self, buf: &[u8]) -> io::Result<usize> {
        self.send(buf)
    }
    fn send_to<A: ToSocketAddrs>(&self, buf: &[u8], addr: A) -> io::Result<usize> {
        self.send_to(buf, addr)
    }
}

impl<S> SocketLike for Conditioner<S>
where
    S: SocketLike,
{
    fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.socket.set_nonblocking(nonblocking)
    }

    fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        if let Ok(mut queue) = self.queue.try_lock() {
            if let Ok((_received, addr)) = self.socket.recv_from(buf) {
                let instant = instant(&self.config);
                if keep_packet(&self.config) {
                    queue.add_item(
                        instant,
                        RecvFrom {
                            addr: addr,
                            data: buf.to_vec(),
                        },
                    );
                }
            }

            if queue.has_item() {
                if let Some(item) = queue.pop_item() {
                    for (index, byte) in item.data.iter().enumerate() {
                        if buf.len() > index {
                            buf[index] = *byte;
                        } else {
                            return Ok((buf.len(), item.addr));
                        }
                    }

                    return Ok((item.data.len(), item.addr));
                }
            }
        }

        Err(std::io::Error::from(std::io::ErrorKind::WouldBlock))
    }

    fn send(&self, buf: &[u8]) -> io::Result<usize> {
        self.socket.send(buf)
    }

    fn send_to<A: ToSocketAddrs>(&self, buf: &[u8], addr: A) -> io::Result<usize> {
        self.socket.send_to(buf, addr)
    }
}

impl<S> Conditioner<S>
where
    S: SocketLike,
{
    pub fn new(config: ConditionerConfig, socket: S) -> Conditioner<S> {
        let queue = Arc::new(Mutex::new(TimeQueue::new()));
        Conditioner {
            socket,
            queue,
            config,
        }
    }
}
