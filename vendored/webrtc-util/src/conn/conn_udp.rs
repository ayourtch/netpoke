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
        // Check if we have UDP options to apply (passed via thread-local storage)
        #[cfg(target_os = "linux")]
        {
            if let Some(options) = get_current_send_options() {
                println!("DEBUG: Found send options, calling send_to_with_options with TTL={:?}", options.ttl);
                return send_to_with_options(self, buf, target, &options).await;
            }
        }
        
        // Default: regular send_to
        Ok(self.send_to(buf, target).await?)
    }

    fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.local_addr()?)
    }

    fn remote_addr(&self) -> Option<SocketAddr> {
        None
    }

    async fn close(&self) -> Result<()> {
        Ok(())
    }

    fn as_any(&self) -> &(dyn std::any::Any + Send + Sync) {
        self
    }
}

// ============================================================================
// UDP Socket Options Support (added for wifi-verify project)
// ============================================================================

use std::cell::RefCell;

thread_local! {
    static SEND_OPTIONS: RefCell<Option<UdpSendOptions>> = RefCell::new(None);
}

/// UDP send options for per-message configuration
#[derive(Debug, Clone, Copy)]
pub struct UdpSendOptions {
    pub ttl: Option<u8>,
    pub tos: Option<u8>,
    pub df_bit: Option<bool>,
}

/// Set send options for the current thread (will be used by next send_to call)
pub fn set_send_options(options: Option<UdpSendOptions>) {
    SEND_OPTIONS.with(|opts| {
        *opts.borrow_mut() = options;
    });
    
    #[cfg(target_os = "linux")]
    if let Some(opts) = options {
        println!("DEBUG: set_send_options called with TTL={:?}, TOS={:?}, DF={:?}", 
            opts.ttl, opts.tos, opts.df_bit);
    } else {
        println!("DEBUG: set_send_options called with None (clearing options)");
    }
}

/// Get and clear current send options
fn get_current_send_options() -> Option<UdpSendOptions> {
    // CRITICAL FIX: Use clone() instead of take() to preserve the value
    // The value should persist until explicitly cleared with set_send_options(None)
    // Using take() caused the options to disappear after first check
    let result = SEND_OPTIONS.with(|opts| opts.borrow().clone());
    
    #[cfg(target_os = "linux")]
    if let Some(ref opts) = result {
        println!("DEBUG: get_current_send_options retrieved TTL={:?}, TOS={:?}, DF={:?}", 
            opts.ttl, opts.tos, opts.df_bit);
    }
    
    result
}

#[cfg(target_os = "linux")]
async fn send_to_with_options(
    socket: &UdpSocket,
    buf: &[u8],
    target: SocketAddr,
    options: &UdpSendOptions,
) -> Result<usize> {
    use tokio::task;
    
    println!("DEBUG: send_to_with_options called with TTL={:?}, target={}", options.ttl, target);
    
    let fd = socket.as_raw_fd();
    let buf = buf.to_vec();
    let options = *options;
    
    // Run blocking sendmsg in a blocking task
    let result = task::spawn_blocking(move || {
        sendmsg_with_options(fd, &buf, target, &options)
    }).await.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))??;
    
    println!("DEBUG: send_to_with_options sent {} bytes", result);
    
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
    println!("DEBUG: sendmsg_with_options called with fd={}, buf_len={}, dest={}, TTL={:?}", 
        fd, buf.len(), dest, options.ttl);
    
    unsafe {
        // Determine the socket's address family (not the destination's)
        // This is crucial: an IPv6 socket can send to IPv4 addresses via IPv4-mapped IPv6,
        // but must use IPv6 control messages (IPPROTO_IPV6) not IPv4 ones (IPPROTO_IP)
        let socket_family = get_socket_family(fd)?;
        let is_ipv6_socket = socket_family == libc::AF_INET6 as libc::sa_family_t;
        
        println!("DEBUG: Socket family: {}, is_ipv6_socket: {}", socket_family, is_ipv6_socket);
        
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
                println!("DEBUG: Adding IPv6 hop limit control message: {}", ttl);
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
                println!("DEBUG: Adding IPv6 traffic class control message: {}", tos);
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
                println!("DEBUG: Adding IPv4 TTL control message: {}", ttl);
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
                }
            }
            
            if let Some(tos) = options.tos {
                println!("DEBUG: Adding IPv4 TOS control message: {}", tos);
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
        
        println!("DEBUG: Calling sendmsg with msg_controllen={}", cmsg_len);
        
        // Send the message
        let result = sendmsg(fd, &msg, 0);
        
        if result < 0 {
            let err = std::io::Error::last_os_error();
            println!("DEBUG: sendmsg failed with error: {}", err);
            return Err(err.into());
        }
        
        println!("DEBUG: sendmsg succeeded, sent {} bytes", result);
        
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

    #[tokio::test]
    #[cfg(target_os = "linux")]
    async fn test_send_with_ttl_ipv4_socket() {
        // Create an IPv4 socket
        let socket = UdpSocket::bind("0.0.0.0:0").await.unwrap();
        
        // Set send options with low TTL
        let options = UdpSendOptions {
            ttl: Some(1),
            tos: None,
            df_bit: Some(true),
        };
        
        set_send_options(Some(options));
        
        // Try to send a packet to a public DNS server (won't actually reach it with TTL=1)
        let dest: SocketAddr = "8.8.8.8:53".parse().unwrap();
        let result = socket.send_to(b"test", dest).await;
        
        // Should either succeed or fail with PermissionDenied (not "Address family not supported")
        match result {
            Ok(_) => println!("✓ IPv4 socket successfully sent packet with TTL=1"),
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                println!("✓ IPv4 socket received expected PermissionDenied (needs CAP_NET_RAW)")
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
        
        set_send_options(None);
    }

    #[tokio::test]
    #[cfg(target_os = "linux")]
    async fn test_send_with_ttl_ipv6_socket() {
        // Create an IPv6 socket (dual-stack)
        let socket = UdpSocket::bind("[::]:0").await.unwrap();
        
        // Set send options with low TTL
        let options = UdpSendOptions {
            ttl: Some(1),
            tos: None,
            df_bit: Some(true),
        };
        
        set_send_options(Some(options));
        
        // Try to send to IPv4 address via dual-stack socket
        // This is the scenario that was failing before the fix with
        // "Address family not supported by protocol" error
        let dest: SocketAddr = "8.8.8.8:53".parse().unwrap();
        let result = socket.send_to(b"test", dest).await;
        
        // Should either succeed or fail with PermissionDenied (not "Address family not supported")
        match result {
            Ok(_) => println!("✓ IPv6 socket successfully sent packet to IPv4 address with TTL=1"),
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                println!("✓ IPv6 socket received expected PermissionDenied (needs CAP_NET_RAW)")
            }
            Err(e) => panic!("Unexpected error (expected success or PermissionDenied, not Address family error): {:?}", e),
        }
        
        set_send_options(None);
    }
}
