use std::{
    io,
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs, UdpSocket},
    ops::Add,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use time_queue::TimeQueue;

pub mod time_queue;

/// Thin wrapper around a `UdpSocket` to provide mock testing of packet loss/latency.
pub enum UdpConditioner {
    Conditioned {
        socket: UdpSocket,
        config: ConditionerConfig,
        queue: Arc<Mutex<TimeQueue<RecvFrom>>>,
    },
    Raw(UdpSocket),
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

impl UdpConditioner {
    pub fn bind<A: ToSocketAddrs>(addr: A) -> io::Result<UdpConditioner> {
        let socket = UdpSocket::bind(addr)?;
        Ok(UdpConditioner::Raw(socket))
    }

    pub fn bind_conditioned<A: ToSocketAddrs>(
        config: ConditionerConfig,
        addr: A,
    ) -> io::Result<UdpConditioner> {
        let socket = UdpSocket::bind(addr)?;
        let queue = Arc::new(Mutex::new(TimeQueue::new()));
        Ok(UdpConditioner::Conditioned {
            socket: socket,
            config: config,
            queue: queue,
        })
    }

    pub fn udp_socket(&self) -> &UdpSocket {
        match self {
            UdpConditioner::Conditioned { socket, .. } => &socket,
            UdpConditioner::Raw(socket) => &socket,
        }
    }

    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.udp_socket().set_nonblocking(nonblocking)
    }

    // We tamper with these and leave the rest to be forwarded to the underlying UdpSocket.
    // ---
    pub fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            UdpConditioner::Conditioned {
                socket,
                queue,
                config,
            } => {
                if let Ok(mut queue) = queue.try_lock() {
                    if let Ok((_received, addr)) = socket.recv_from(buf) {
                        let instant = Instant::now().add(config.latency);
                        queue.add_item(
                            instant,
                            RecvFrom {
                                addr: addr,
                                data: buf.to_vec(),
                            },
                        );

                        if queue.has_item() {
                            if let Some(item) = queue.pop_item() {
                                for (index, byte) in item.data.iter().enumerate() {
                                    if buf.len() > index {
                                        buf[index] = *byte;
                                    } else {
                                        return Ok(buf.len());
                                    }
                                }

                                return Ok(item.data.len());
                            }
                        }
                    }
                }

                Err(std::io::Error::from(std::io::ErrorKind::WouldBlock))
            }
            UdpConditioner::Raw(socket) => socket.recv(buf),
        }
    }

    pub fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        match self {
            UdpConditioner::Conditioned {
                socket,
                queue,
                config,
            } => {
                if let Ok(mut queue) = queue.try_lock() {
                    if let Ok((_received, addr)) = socket.recv_from(buf) {
                        let instant = Instant::now().add(config.latency);
                        queue.add_item(
                            instant,
                            RecvFrom {
                                addr: addr,
                                data: buf.to_vec(),
                            },
                        );
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
            UdpConditioner::Raw(socket) => socket.recv_from(buf),
        }
    }

    pub fn peek_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        match self {
            UdpConditioner::Conditioned {
                socket,
                queue,
                config,
            } => socket.peek_from(buf),
            UdpConditioner::Raw(socket) => socket.peek_from(buf),
        }
    }

    pub fn send(&self, buf: &[u8]) -> io::Result<usize> {
        self.udp_socket().send(buf)
    }

    pub fn send_to<A: ToSocketAddrs>(&self, buf: &[u8], addr: A) -> io::Result<usize> {
        self.udp_socket().send_to(buf, addr)
    }

    // ---

    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.udp_socket().peer_addr()
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.udp_socket().local_addr()
    }

    pub fn try_clone(&self) -> io::Result<UdpSocket> {
        self.udp_socket().try_clone()
    }

    pub fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.udp_socket().set_read_timeout(dur)
    }

    pub fn set_write_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.udp_socket().set_write_timeout(dur)
    }

    pub fn read_timeout(&self) -> io::Result<Option<Duration>> {
        self.udp_socket().read_timeout()
    }

    pub fn write_timeout(&self) -> io::Result<Option<Duration>> {
        self.udp_socket().write_timeout()
    }

    pub fn set_broadcast(&self, broadcast: bool) -> io::Result<()> {
        self.udp_socket().set_broadcast(broadcast)
    }

    pub fn broadcast(&self) -> io::Result<bool> {
        self.udp_socket().broadcast()
    }

    pub fn set_multicast_loop_v4(&self, multicast_loop_v4: bool) -> io::Result<()> {
        self.udp_socket().set_multicast_loop_v4(multicast_loop_v4)
    }

    pub fn multicast_loop_v4(&self) -> io::Result<bool> {
        self.udp_socket().multicast_loop_v4()
    }

    pub fn set_multicast_ttl_v4(&self, multicast_ttl_v4: u32) -> io::Result<()> {
        self.udp_socket().set_multicast_ttl_v4(multicast_ttl_v4)
    }

    pub fn multicast_ttl_v4(&self) -> io::Result<u32> {
        self.udp_socket().multicast_ttl_v4()
    }

    pub fn set_multicast_loop_v6(&self, multicast_loop_v6: bool) -> io::Result<()> {
        self.udp_socket().set_multicast_loop_v6(multicast_loop_v6)
    }

    pub fn multicast_loop_v6(&self) -> io::Result<bool> {
        self.udp_socket().multicast_loop_v6()
    }

    pub fn ttl(&self) -> io::Result<u32> {
        self.udp_socket().ttl()
    }

    pub fn join_multicast_v4(&self, multiaddr: &Ipv4Addr, interface: &Ipv4Addr) -> io::Result<()> {
        self.udp_socket().join_multicast_v4(multiaddr, interface)
    }

    pub fn join_multicast_v6(&self, multiaddr: &Ipv6Addr, interface: u32) -> io::Result<()> {
        self.udp_socket().join_multicast_v6(multiaddr, interface)
    }

    pub fn leave_multicast_v4(&self, multiaddr: &Ipv4Addr, interface: &Ipv4Addr) -> io::Result<()> {
        self.udp_socket().leave_multicast_v4(multiaddr, interface)
    }

    pub fn leave_multicast_v6(&self, multiaddr: &Ipv6Addr, interface: u32) -> io::Result<()> {
        self.udp_socket().leave_multicast_v6(multiaddr, interface)
    }

    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.udp_socket().take_error()
    }

    pub fn connect<A: ToSocketAddrs>(&self, addr: A) -> io::Result<()> {
        self.udp_socket().connect(addr)
    }
}
