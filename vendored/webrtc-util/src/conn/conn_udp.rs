use tokio::net::UdpSocket;
use std::os::unix::io::AsRawFd;

use super::*;

// UDP socket options support (added for wifi-verify)
#[cfg(target_os = "linux")]
use libc::{
    c_void, iovec, msghdr, sendmsg, IPPROTO_IP, IPPROTO_IPV6,
    IP_TTL, IP_TOS, IPV6_HOPLIMIT, IPV6_TCLASS,
};

#[async_trait]
impl Conn for UdpSocket {
    async fn connect(&self, addr: SocketAddr) -> Result<()> {
        Ok(self.connect(addr).await?)
    }

    async fn recv(&self, buf: &mut [u8]) -> Result<usize> {
        Ok(self.recv(buf).await?)
    }

    async fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr)> {
        Ok(self.recv_from(buf).await?)
    }

    async fn send(&self, buf: &[u8]) -> Result<usize> {
        Ok(self.send(buf).await?)
    }

    async fn send_to(&self, buf: &[u8], target: SocketAddr) -> Result<usize> {
        Ok(self.send_to(buf, target).await?)
    }

    fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.local_addr()?)
    }

    fn remote_addr(&self) -> Option<SocketAddr> {
        self.peer_addr().ok()
    }

    async fn close(&self) -> Result<()> {
        Ok(())
    }

    fn as_any(&self) -> &(dyn std::any::Any + Send + Sync) {
        self
    }
    
    #[cfg(target_os = "linux")]
    async fn send_with_options(
        &self,
        buf: &[u8],
        options: &UdpSendOptions,
    ) -> Result<usize> {
        log::debug!("UdpSocket::send_with_options called with TTL={:?}, TOS={:?}, DF={:?}", 
            options.ttl, options.tos, options.df_bit);
        // For connected sockets, we need to get the remote address
        if let Some(remote_addr) = self.peer_addr().ok() {
            log::debug!("UdpSocket: Forwarding to send_to_with_options_impl for addr={}", remote_addr);
            send_to_with_options_impl(self, buf, remote_addr, options).await
        } else {
            // If not connected, fall back to regular send
            log::warn!("⚠️  UdpSocket: No peer address, falling back to regular send (options will be LOST)");
            Ok(self.send(buf).await?)
        }
    }
    
    #[cfg(target_os = "linux")]
    async fn send_to_with_options(
        &self,
        buf: &[u8],
        target: SocketAddr,
        options: &UdpSendOptions,
    ) -> Result<usize> {
        log::debug!("UdpSocket::send_to_with_options called with TTL={:?}, TOS={:?}, DF={:?}, target={}", 
            options.ttl, options.tos, options.df_bit, target);
        send_to_with_options_impl(self, buf, target, options).await
    }
}

// ============================================================================
// UDP Socket Options Support (added for wifi-verify project)
// ============================================================================

/// UDP send options for per-message configuration
#[derive(Debug, Clone, Default)]
pub struct UdpSendOptions {
    pub ttl: Option<u8>,
    pub tos: Option<u8>,
    pub df_bit: Option<bool>,
    /// Connection ID for ICMP correlation (passed through to packet tracker)
    /// Defaults to empty string for backward compatibility
    pub conn_id: String,
}

#[cfg(target_os = "linux")]
async fn send_to_with_options_impl(
    socket: &UdpSocket,
    buf: &[u8],
    target: SocketAddr,
    options: &UdpSendOptions,
) -> Result<usize> {
    use tokio::task;
    
    log::debug!("send_to_with_options_impl: buf_len={}, TTL={:?}, target={}", 
        buf.len(), options.ttl, target);
    
    let fd = socket.as_raw_fd();
    let buf = buf.to_vec();
    let options = options.clone();
    
    // Run blocking sendmsg in a blocking task
    let result = task::spawn_blocking(move || {
        sendmsg_with_options(fd, &buf, target, &options)
    }).await.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))??;
    
    log::debug!("send_to_with_options_impl: Successfully sent {} bytes", result);
    
    Ok(result)
}

#[cfg(target_os = "linux")]
/// Queries the address family of a socket using getsockname()
/// 
/// This is critical for determining which control message protocol level to use
/// when setting socket options like TTL. An IPv6 socket must use IPPROTO_IPV6
/// control messages even when sending to IPv4 addresses via IPv4-mapped IPv6.
/// 
/// # Arguments
/// * `fd` - The raw file descriptor of the socket
/// 
/// # Returns
/// * `Ok(sa_family_t)` - The address family (AF_INET or AF_INET6)
/// * `Err(Error)` - If getsockname() fails
fn get_socket_family(fd: std::os::unix::io::RawFd) -> Result<libc::sa_family_t> {
    unsafe {
        let mut addr: libc::sockaddr_storage = std::mem::zeroed();
        let mut addr_len: libc::socklen_t = std::mem::size_of::<libc::sockaddr_storage>() as libc::socklen_t;
        
        let result = libc::getsockname(
            fd,
            &mut addr as *mut libc::sockaddr_storage as *mut libc::sockaddr,
            &mut addr_len,
        );
        
        if result < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        
        Ok(addr.ss_family)
    }
}

#[cfg(target_os = "linux")]
fn sendmsg_with_options(
    fd: std::os::unix::io::RawFd,
    buf: &[u8],
    dest: SocketAddr,
    options: &UdpSendOptions,
) -> Result<usize> {
    log::debug!("sendmsg_with_options: fd={}, buf_len={}, dest={}, TTL={:?}, TOS={:?}, DF={:?}", 
        fd, buf.len(), dest, options.ttl, options.tos, options.df_bit);
    
    unsafe {
        // Determine the socket's address family (not the destination's)
        // This is crucial: an IPv6 socket can send to IPv4 addresses via IPv4-mapped IPv6,
        // but must use IPv6 control messages (IPPROTO_IPV6) not IPv4 ones (IPPROTO_IP)
        let socket_family = get_socket_family(fd)?;
        let is_ipv6_socket = socket_family == libc::AF_INET6 as libc::sa_family_t;
        
        log::debug!("sendmsg_with_options: Socket family={}, is_ipv6={}", socket_family, is_ipv6_socket);
        
        // Prepare the data buffer
        let mut iov = iovec {
            iov_base: buf.as_ptr() as *mut c_void,
            iov_len: buf.len(),
        };
        
        // Prepare control message buffer
        let mut cmsg_buf = vec![0u8; 256];
        let mut cmsg_len = 0usize;
        
        // Build the msghdr structure
        // CRITICAL FIX: We must keep the address storage alive for the entire duration
        // of the sendmsg call. Previously, we were returning references to local variables
        // in the match arms, which created dangling pointers causing EAFNOSUPPORT (97)
        // and EINVAL (22) errors.
        let mut addr_storage_v4: libc::sockaddr_in = std::mem::zeroed();
        let mut addr_storage_v6: libc::sockaddr_in6 = std::mem::zeroed();
        
        let (addr_ptr, addr_len) = match dest {
            SocketAddr::V4(addr) => {
                addr_storage_v4.sin_family = libc::AF_INET as libc::sa_family_t;
                addr_storage_v4.sin_port = addr.port().to_be();
                addr_storage_v4.sin_addr = libc::in_addr {
                    s_addr: u32::from_ne_bytes(addr.ip().octets()),
                };
                (
                    &addr_storage_v4 as *const libc::sockaddr_in as *const libc::sockaddr,
                    std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
                )
            }
            SocketAddr::V6(addr) => {
                addr_storage_v6.sin6_family = libc::AF_INET6 as libc::sa_family_t;
                addr_storage_v6.sin6_port = addr.port().to_be();
                addr_storage_v6.sin6_addr = libc::in6_addr {
                    s6_addr: addr.ip().octets(),
                };
                (
                    &addr_storage_v6 as *const libc::sockaddr_in6 as *const libc::sockaddr,
                    std::mem::size_of::<libc::sockaddr_in6>() as libc::socklen_t,
                )
            }
        };
        
        let mut msg: msghdr = std::mem::zeroed();
        msg.msg_name = addr_ptr as *mut c_void;
        msg.msg_namelen = addr_len;
        msg.msg_iov = &mut iov;
        msg.msg_iovlen = 1;
        msg.msg_control = cmsg_buf.as_mut_ptr() as *mut c_void;
        msg.msg_controllen = cmsg_buf.len();
        
        // Add control messages based on SOCKET family (not destination family)
        // An IPv6 socket must use IPv6 control messages even when sending to IPv4 addresses
        if is_ipv6_socket {
            // IPv6 socket: use IPPROTO_IPV6 control messages
            if let Some(ttl) = options.ttl {
                log::debug!("sendmsg: Adding IPv6 hop limit control message: TTL={}", ttl);
                let cmsg = libc::CMSG_FIRSTHDR(&msg);
                if !cmsg.is_null() {
                    (*cmsg).cmsg_level = IPPROTO_IPV6;
                    (*cmsg).cmsg_type = IPV6_HOPLIMIT;
                    (*cmsg).cmsg_len = libc::CMSG_LEN(std::mem::size_of::<i32>() as u32) as usize;
                    
                    let data_ptr = libc::CMSG_DATA(cmsg);
                    *(data_ptr as *mut i32) = ttl as i32;
                    
                    cmsg_len = (*cmsg).cmsg_len;
                }
            }
            
            if let Some(tos) = options.tos {
                log::debug!("sendmsg: Adding IPv6 traffic class control message: TOS={}", tos);
                let cmsg = if cmsg_len > 0 {
                    let first = libc::CMSG_FIRSTHDR(&msg);
                    libc::CMSG_NXTHDR(&msg, first)
                } else {
                    libc::CMSG_FIRSTHDR(&msg)
                };
                
                if !cmsg.is_null() {
                    (*cmsg).cmsg_level = IPPROTO_IPV6;
                    (*cmsg).cmsg_type = IPV6_TCLASS;
                    (*cmsg).cmsg_len = libc::CMSG_LEN(std::mem::size_of::<i32>() as u32) as usize;
                    
                    let data_ptr = libc::CMSG_DATA(cmsg);
                    *(data_ptr as *mut i32) = tos as i32;
                    
                    cmsg_len += (*cmsg).cmsg_len;
                }
            }
        } else {
            // IPv4 socket: use IPPROTO_IP control messages
            if let Some(ttl) = options.ttl {
                log::debug!("sendmsg: Adding IPv4 TTL control message: TTL={}", ttl);
                let cmsg = libc::CMSG_FIRSTHDR(&msg);
                if !cmsg.is_null() {
                    (*cmsg).cmsg_level = IPPROTO_IP;
                    (*cmsg).cmsg_type = IP_TTL;
                    // CRITICAL FIX: IP_TTL expects int (i32), not u8
                    // See: ip(7) man page - IP_TTL takes an integer argument
                    (*cmsg).cmsg_len = libc::CMSG_LEN(std::mem::size_of::<i32>() as u32) as usize;
                    
                    let data_ptr = libc::CMSG_DATA(cmsg);
                    *(data_ptr as *mut i32) = ttl as i32;
                    
                    cmsg_len = (*cmsg).cmsg_len;
                    log::debug!("sendmsg: Set IPv4 TTL={} in control message, cmsg_len={}", ttl, cmsg_len);
                }
            }
            
            if let Some(tos) = options.tos {
                log::debug!("sendmsg: Adding IPv4 TOS control message: TOS={}", tos);
                let cmsg = if cmsg_len > 0 {
                    let first = libc::CMSG_FIRSTHDR(&msg);
                    libc::CMSG_NXTHDR(&msg, first)
                } else {
                    libc::CMSG_FIRSTHDR(&msg)
                };
                
                if !cmsg.is_null() {
                    (*cmsg).cmsg_level = IPPROTO_IP;
                    (*cmsg).cmsg_type = IP_TOS;
                    // IP_TOS also expects int (i32), not u8
                    // See: ip(7) man page - IP_TOS takes an integer argument
                    (*cmsg).cmsg_len = libc::CMSG_LEN(std::mem::size_of::<i32>() as u32) as usize;
                    
                    let data_ptr = libc::CMSG_DATA(cmsg);
                    *(data_ptr as *mut i32) = tos as i32;
                    
                    cmsg_len += (*cmsg).cmsg_len;
                }
            }
        }
        
        // Update control message length
        msg.msg_controllen = cmsg_len;
        
        log::debug!("sendmsg: Calling sendmsg with msg_controllen={}", cmsg_len);
        
        // Send the message
        let result = sendmsg(fd, &msg, 0);
        
        if result < 0 {
            let err = std::io::Error::last_os_error();
            log::error!("❌ sendmsg FAILED with error: {} (errno={})", err, err.raw_os_error().unwrap_or(-1));
            return Err(err.into());
        }
        
        log::debug!("sendmsg SUCCEEDED: sent {} bytes", result);
        
        // Track this packet for ICMP/ICMPv6 correlation if TTL/Hop Limit is set
        // Call the extern function from wifi-verify-server
        if let Some(ttl_value) = options.ttl {
            // Declare extern functions once
            extern "C" {
                fn wifi_verify_track_udp_packet(
                    dest_ip_v4: u32,
                    dest_port: u16,
                    udp_length: u16,
                    ttl: u8,
                    buf_ptr: *const u8,
                    buf_len: usize,
                    conn_id_ptr: *const u8,
                    conn_id_len: usize,
                );
                
                fn wifi_verify_track_udp_packet_v6(
                    dest_ip_v6_ptr: *const u8,
                    dest_port: u16,
                    udp_length: u16,
                    hop_limit: u8,
                    buf_ptr: *const u8,
                    buf_len: usize,
                    conn_id_ptr: *const u8,
                    conn_id_len: usize,
                );
            }
            
            match dest {
                SocketAddr::V4(addr_v4) => {
                    // Track IPv4 packet
                    let dest_ip = u32::from_be_bytes(addr_v4.ip().octets());
                    let udp_length = (8 + buf.len()) as u16; // UDP header (8 bytes) + payload
                    
                    log::debug!("Calling wifi_verify_track_udp_packet (IPv4): dest={}:{}, udp_length={}, ttl={}, conn_id={}",
                        addr_v4.ip(), addr_v4.port(), udp_length, ttl_value, options.conn_id);
                    
                    unsafe {
                        wifi_verify_track_udp_packet(
                            dest_ip,
                            addr_v4.port(),
                            udp_length,
                            ttl_value,
                            buf.as_ptr(),
                            buf.len(),
                            options.conn_id.as_ptr(),
                            options.conn_id.len(),
                        );
                    }
                }
                SocketAddr::V6(addr_v6) => {
                    // Track IPv6 packet
                    let dest_ip = addr_v6.ip().octets();
                    let udp_length = (8 + buf.len()) as u16; // UDP header (8 bytes) + payload
                    
                    log::debug!("Calling wifi_verify_track_udp_packet_v6 (IPv6): dest=[{}]:{}, udp_length={}, hop_limit={}, conn_id={}",
                        addr_v6.ip(), addr_v6.port(), udp_length, ttl_value, options.conn_id);
                    
                    unsafe {
                        wifi_verify_track_udp_packet_v6(
                            dest_ip.as_ptr(),
                            addr_v6.port(),
                            udp_length,
                            ttl_value,
                            buf.as_ptr(),
                            buf.len(),
                            options.conn_id.as_ptr(),
                            options.conn_id.len(),
                        );
                    }
                }
            }
        }
        
        Ok(result as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::UdpSocket;
    use std::os::unix::io::AsRawFd;

    #[tokio::test]
    #[cfg(target_os = "linux")]
    async fn test_ipv4_socket_family() {
        // Create an IPv4 socket
        let socket = UdpSocket::bind("0.0.0.0:0").await.unwrap();
        let fd = socket.as_raw_fd();
        
        let family = get_socket_family(fd).unwrap();
        assert_eq!(family, libc::AF_INET as libc::sa_family_t, "IPv4 socket should have AF_INET family");
        
        println!("✓ IPv4 socket correctly identified with family: {}", family);
    }

    #[tokio::test]
    #[cfg(target_os = "linux")]
    async fn test_ipv6_socket_family() {
        // Create an IPv6 socket (dual-stack)
        let socket = UdpSocket::bind("[::]:0").await.unwrap();
        let fd = socket.as_raw_fd();
        
        let family = get_socket_family(fd).unwrap();
        assert_eq!(family, libc::AF_INET6 as libc::sa_family_t, "IPv6 socket should have AF_INET6 family");
        
        println!("✓ IPv6 socket correctly identified with family: {}", family);
    }
}
