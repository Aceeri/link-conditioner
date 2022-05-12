use std::{
    io,
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs, UdpSocket},
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

/// Thin wrapper around a `UdpSocket` to provide mock testing of packet loss/latency.
pub struct ConditionedUdpSocket {
    pub config: ConditionerConfig,
    socket: UdpSocket,
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

pub trait UdpSocketLike {
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

impl UdpSocketLike for ConditionedUdpSocket {
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

impl ConditionedUdpSocket {
    pub fn bind<A: ToSocketAddrs>(
        config: ConditionerConfig,
        addr: A,
    ) -> io::Result<ConditionedUdpSocket> {
        let socket = UdpSocket::bind(addr)?;
        println!("binding to {:?}", socket);
        let queue = Arc::new(Mutex::new(TimeQueue::new()));
        Ok(ConditionedUdpSocket {
            socket,
            queue,
            config,
        })
    }

    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.socket.peer_addr()
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.socket.local_addr()
    }

    pub fn try_clone(&self) -> io::Result<UdpSocket> {
        self.socket.try_clone()
    }

    pub fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.socket.set_read_timeout(dur)
    }

    pub fn set_write_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.socket.set_write_timeout(dur)
    }

    pub fn read_timeout(&self) -> io::Result<Option<Duration>> {
        self.socket.read_timeout()
    }

    pub fn write_timeout(&self) -> io::Result<Option<Duration>> {
        self.socket.write_timeout()
    }

    pub fn set_broadcast(&self, broadcast: bool) -> io::Result<()> {
        self.socket.set_broadcast(broadcast)
    }

    pub fn broadcast(&self) -> io::Result<bool> {
        self.socket.broadcast()
    }

    pub fn set_multicast_loop_v4(&self, multicast_loop_v4: bool) -> io::Result<()> {
        self.socket.set_multicast_loop_v4(multicast_loop_v4)
    }

    pub fn multicast_loop_v4(&self) -> io::Result<bool> {
        self.socket.multicast_loop_v4()
    }

    pub fn set_multicast_ttl_v4(&self, multicast_ttl_v4: u32) -> io::Result<()> {
        self.socket.set_multicast_ttl_v4(multicast_ttl_v4)
    }

    pub fn multicast_ttl_v4(&self) -> io::Result<u32> {
        self.socket.multicast_ttl_v4()
    }

    pub fn set_multicast_loop_v6(&self, multicast_loop_v6: bool) -> io::Result<()> {
        self.socket.set_multicast_loop_v6(multicast_loop_v6)
    }

    pub fn multicast_loop_v6(&self) -> io::Result<bool> {
        self.socket.multicast_loop_v6()
    }

    pub fn ttl(&self) -> io::Result<u32> {
        self.socket.ttl()
    }

    pub fn join_multicast_v4(&self, multiaddr: &Ipv4Addr, interface: &Ipv4Addr) -> io::Result<()> {
        self.socket.join_multicast_v4(multiaddr, interface)
    }

    pub fn join_multicast_v6(&self, multiaddr: &Ipv6Addr, interface: u32) -> io::Result<()> {
        self.socket.join_multicast_v6(multiaddr, interface)
    }

    pub fn leave_multicast_v4(&self, multiaddr: &Ipv4Addr, interface: &Ipv4Addr) -> io::Result<()> {
        self.socket.leave_multicast_v4(multiaddr, interface)
    }

    pub fn leave_multicast_v6(&self, multiaddr: &Ipv6Addr, interface: u32) -> io::Result<()> {
        self.socket.leave_multicast_v6(multiaddr, interface)
    }

    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.socket.take_error()
    }

    pub fn connect<A: ToSocketAddrs>(&self, addr: A) -> io::Result<()> {
        self.socket.connect(addr)
    }
}
