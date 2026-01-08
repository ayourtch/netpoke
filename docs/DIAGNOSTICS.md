# Server Diagnostics Endpoint

## Overview

The NetPoke server provides a comprehensive diagnostics endpoint at `/api/diagnostics` that helps troubleshoot WebRTC connection issues. This endpoint is particularly useful when the server gets into a state where WebRTC connections cannot establish, and you see the error:

```
"pingAllCandidates called with no candidate pairs. Connection is not possible yet."
```

## Accessing the Diagnostics Endpoint

The diagnostics endpoint requires authentication and is accessible at:

```
GET /api/diagnostics
```

### Authentication

- If authentication is enabled in the server configuration, you must be logged in to access this endpoint
- The endpoint follows the same authentication rules as other dashboard endpoints
- If authentication is disabled, the endpoint is publicly accessible

### Example Request

```bash
# With authentication (using session cookie)
curl -X GET https://your-server.com/api/diagnostics \
  -H "Cookie: session_id=YOUR_SESSION_ID" \
  -H "Accept: application/json"

# Without authentication (if auth is disabled)
curl -X GET http://localhost:3000/api/diagnostics
```

## Response Format

The endpoint returns a JSON object with comprehensive server and session diagnostics:

```json
{
  "server_uptime_secs": 3600,
  "total_sessions": 5,
  "connected_sessions": 3,
  "disconnected_sessions": 1,
  "failed_sessions": 1,
  "sessions": [
    {
      "session_id": "client-123",
      "parent_id": null,
      "ip_version": "ipv4",
      "mode": "measurement",
      "conn_id": "uuid-string",
      "connected_at_secs": 120,
      "connection_state": "Connected",
      "ice_connection_state": "Connected",
      "ice_gathering_state": "Complete",
      "peer_address": "192.168.1.100",
      "peer_port": 54321,
      "candidate_pairs": [
        {
          "local_candidate_type": "Host",
          "local_address": "10.0.0.5:12345",
          "remote_candidate_type": "Host",
          "remote_address": "192.168.1.100:54321",
          "state": "Succeeded",
          "nominated": true,
          "bytes_sent": 1048576,
          "bytes_received": 524288
        }
      ],
      "data_channels": {
        "probe": "Open",
        "bulk": "Open",
        "control": "Open",
        "testprobe": null
      },
      "icmp_error_count": 0,
      "last_icmp_error_secs_ago": null
    }
  ]
}
```

## Response Fields

### Server-Level Fields

| Field | Type | Description |
|-------|------|-------------|
| `server_uptime_secs` | integer | Time in seconds since the server started |
| `total_sessions` | integer | Total number of active sessions |
| `connected_sessions` | integer | Number of sessions in Connected state |
| `disconnected_sessions` | integer | Number of sessions in Disconnected state |
| `failed_sessions` | integer | Number of sessions in Failed state |

### Session-Level Fields

| Field | Type | Description |
|-------|------|-------------|
| `session_id` | string | Unique identifier for this session |
| `parent_id` | string? | Parent session ID (for grouped sessions) |
| `ip_version` | string? | IP version used ("ipv4" or "ipv6") |
| `mode` | string? | Session mode ("measurement" or "traceroute") |
| `conn_id` | string | Connection UUID for ECMP testing |
| `connected_at_secs` | integer | Time in seconds since session started |
| `connection_state` | string | WebRTC peer connection state |
| `ice_connection_state` | string | ICE connection state |
| `ice_gathering_state` | string | ICE gathering state |
| `peer_address` | string? | Client IP address |
| `peer_port` | integer? | Client port number |
| `candidate_pairs` | array | List of ICE candidate pairs (see below) |
| `data_channels` | object | Status of data channels (see below) |
| `icmp_error_count` | integer | Number of recent ICMP errors |
| `last_icmp_error_secs_ago` | integer? | Seconds since last ICMP error |

### Candidate Pair Fields

Each candidate pair in the `candidate_pairs` array contains:

| Field | Type | Description |
|-------|------|-------------|
| `local_candidate_type` | string | Type of local candidate (Host, Srflx, Relay, etc.) |
| `local_address` | string | Local IP:port |
| `remote_candidate_type` | string | Type of remote candidate |
| `remote_address` | string | Remote IP:port |
| `state` | string | Pair state (Waiting, InProgress, Succeeded, Failed) |
| `nominated` | boolean | Whether this pair is nominated |
| `bytes_sent` | integer | Bytes sent on this pair |
| `bytes_received` | integer | Bytes received on this pair |

### Data Channel Status

The `data_channels` object shows the state of each data channel:

| Field | Type | Description |
|-------|------|-------------|
| `probe` | string? | State of probe channel (Open, Connecting, Closing, Closed) |
| `bulk` | string? | State of bulk data channel |
| `control` | string? | State of control channel |
| `testprobe` | string? | State of test probe channel (for traceroute) |

A `null` value indicates the channel has not been created.

## Connection States

### Peer Connection States
- **New**: Initial state
- **Connecting**: Connection attempt in progress
- **Connected**: Connection established successfully
- **Disconnected**: Connection temporarily lost
- **Failed**: Connection permanently failed
- **Closed**: Connection closed

### ICE Connection States
- **New**: ICE agent is gathering candidates
- **Checking**: ICE agent is checking candidate pairs
- **Connected**: At least one candidate pair is connected
- **Completed**: All components are connected
- **Failed**: Connection failed
- **Disconnected**: Connection lost
- **Closed**: ICE agent is shut down

### ICE Gathering States
- **New**: No candidates gathered yet
- **Gathering**: Candidates are being gathered
- **Complete**: All candidates have been gathered

## Troubleshooting Guide

### Problem: "No candidate pairs" Error

**Symptoms:**
- Logs show: "pingAllCandidates called with no candidate pairs"
- Dashboard shows no clients connected
- Restarting clients doesn't help
- Only restarting server helps

**Diagnostics to Check:**

1. Check `ice_gathering_state` for all sessions:
   - Should be "Complete" for healthy connections
   - If stuck in "New" or "Gathering", ICE gathering may have stalled

2. Check `candidate_pairs` array:
   - Empty array indicates no candidate pairs were formed
   - This is the root cause of the error message

3. Check `ice_connection_state`:
   - Should progress from "Checking" to "Connected"
   - Stuck in "Checking" with no candidate pairs indicates the problem

4. Review server uptime and session states:
   - High number of `failed_sessions` may indicate a systemic issue
   - Compare `connected_at_secs` with session states

**Potential Causes:**

1. **Network Configuration Issues:**
   - Firewall blocking ICE candidates
   - NAT traversal problems
   - Missing STUN/TURN server configuration

2. **Server Resource Exhaustion:**
   - High memory usage preventing ICE gathering
   - Socket descriptor limits reached
   - Thread pool exhaustion

3. **WebRTC Stack Issues:**
   - Bug in the ICE agent
   - Race condition in candidate gathering
   - Memory corruption in connection state

**Resolution Steps:**

1. **Immediate:** Restart the server (this is currently the only known fix)

2. **Investigation:** After restart, monitor the diagnostics endpoint to identify patterns:
   - When does the issue occur?
   - Are there common factors among failed sessions?
   - Does it correlate with server load or uptime?

3. **Prevention:** Consider implementing:
   - Automatic health checks that trigger server restart
   - Rate limiting on new connections
   - Better resource monitoring and alerting
   - Periodic cleanup of stale sessions

## Integration with Monitoring

You can integrate this endpoint with monitoring tools:

```bash
# Prometheus-style metrics
curl -s http://localhost:3000/api/diagnostics | jq '{
  server_uptime: .server_uptime_secs,
  total_sessions: .total_sessions,
  connected_sessions: .connected_sessions,
  failed_sessions: .failed_sessions
}'

# Check for unhealthy sessions
curl -s http://localhost:3000/api/diagnostics | jq '.sessions[] | 
  select(.candidate_pairs | length == 0) | 
  {session_id, connection_state, ice_connection_state}'
```

## Future Enhancements

Potential improvements to the diagnostics system:

1. **Automatic Detection:** Server could automatically detect the "no candidate pairs" condition
2. **Self-Healing:** Implement automatic session cleanup or server restart
3. **Historical Data:** Store diagnostics snapshots for trend analysis
4. **Alerting:** Send notifications when problematic conditions are detected
5. **Additional Metrics:** Add more detailed WebRTC statistics (DTLS state, SCTP state, etc.)

## Related Documentation

- [AUTHENTICATION.md](AUTHENTICATION.md) - Authentication configuration
- [README.md](README.md) - General server documentation
- [VERIFICATION_GUIDE.md](VERIFICATION_GUIDE.md) - Client connection testing
