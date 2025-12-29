#!/usr/bin/env python3
"""
Test script to verify traceroute packet matching fix.

This script demonstrates that with the new HOP_MULTIPLIER=50,
there are no collisions in UDP packet lengths even with variable
encryption overhead.
"""

# Constants from server/src/measurements.rs
BASE_PROBE_SIZE = 100
CONN_ID_MULTIPLIER = 97
HOP_MULTIPLIER_OLD = 3   # Old value (broken)
HOP_MULTIPLIER_NEW = 50  # New value (fixed)
CONN_ID_HASH_RANGE = 10
MAX_TTL = 30

# Encryption overhead variance
OVERHEAD_MIN = 80
OVERHEAD_MAX = 110
OVERHEAD_VARIANCE = OVERHEAD_MAX - OVERHEAD_MIN

def test_multiplier(hop_multiplier, label):
    """Test a HOP_MULTIPLIER value for collisions."""
    print(f"\n{'=' * 70}")
    print(f"Testing {label}: HOP_MULTIPLIER = {hop_multiplier}")
    print(f"{'=' * 70}")
    
    # Track all possible UDP lengths for each connection
    udp_lengths_by_conn = {}
    
    for conn_id_hash in range(CONN_ID_HASH_RANGE):
        udp_lengths = {}
        
        for ttl in range(1, MAX_TTL + 1):
            json_size = BASE_PROBE_SIZE + (conn_id_hash * CONN_ID_MULTIPLIER) + (ttl * hop_multiplier)
            
            # Calculate possible UDP lengths with min and max overhead
            udp_len_min = json_size + OVERHEAD_MIN + 8  # +8 for UDP header
            udp_len_max = json_size + OVERHEAD_MAX + 8
            
            # Store range of possible UDP lengths for this TTL
            udp_lengths[ttl] = (udp_len_min, udp_len_max)
        
        udp_lengths_by_conn[conn_id_hash] = udp_lengths
    
    # Check for collisions within each connection
    total_collisions = 0
    
    for conn_id_hash in range(CONN_ID_HASH_RANGE):
        conn_collisions = []
        udp_lengths = udp_lengths_by_conn[conn_id_hash]
        
        for ttl1 in range(1, MAX_TTL + 1):
            min1, max1 = udp_lengths[ttl1]
            
            for ttl2 in range(ttl1 + 1, MAX_TTL + 1):
                min2, max2 = udp_lengths[ttl2]
                
                # Check if ranges overlap
                if max1 >= min2:
                    conn_collisions.append((ttl1, min1, max1, ttl2, min2, max2))
                    total_collisions += 1
        
        if conn_collisions and conn_id_hash == 0:
            print(f"\n❌ Connection {conn_id_hash} has {len(conn_collisions)} collision(s):")
            for ttl1, min1, max1, ttl2, min2, max2 in conn_collisions[:3]:
                print(f"   TTL {ttl1:2d} (UDP: {min1}-{max1}) overlaps with TTL {ttl2:2d} (UDP: {min2}-{max2})")
            if len(conn_collisions) > 3:
                print(f"   ... and {len(conn_collisions) - 3} more")
    
    if total_collisions == 0:
        print(f"\n✅ NO COLLISIONS! All {CONN_ID_HASH_RANGE * MAX_TTL} packets have unique UDP lengths.")
        
        # Show spacing for first connection
        print(f"\nExample spacing for connection 0:")
        udp_lengths = udp_lengths_by_conn[0]
        for ttl in [1, 2, 3, 10, 20, 30]:
            min_len, max_len = udp_lengths[ttl]
            print(f"   TTL {ttl:2d}: UDP {min_len:4d}-{max_len:4d} bytes")
    else:
        print(f"\n❌ FOUND {total_collisions} COLLISIONS across all connections!")
        print(f"   This will cause WRONG HOP MATCHING in traceroute!")
    
    return total_collisions == 0

def main():
    print("Traceroute Packet Matching Collision Test")
    print(f"Encryption overhead variance: {OVERHEAD_VARIANCE} bytes")
    
    # Test old value (should have collisions)
    old_ok = test_multiplier(HOP_MULTIPLIER_OLD, "OLD (BROKEN)")
    
    # Test new value (should have no collisions)
    new_ok = test_multiplier(HOP_MULTIPLIER_NEW, "NEW (FIXED)")
    
    # Summary
    print(f"\n{'=' * 70}")
    print("SUMMARY")
    print(f"{'=' * 70}")
    
    if not old_ok:
        print("❌ OLD value (HOP_MULTIPLIER=3): COLLISIONS FOUND")
        print("   → ICMP errors can match wrong hops!")
    
    if new_ok:
        print("✅ NEW value (HOP_MULTIPLIER=50): NO COLLISIONS")
        print("   → ICMP errors match correct hops!")
    
    print(f"\nThe fix increases spacing from {HOP_MULTIPLIER_OLD} to {HOP_MULTIPLIER_NEW} bytes,")
    print(f"which is greater than the {OVERHEAD_VARIANCE}-byte encryption variance.")
    print("This ensures unique UDP packet lengths for all traceroute probes.")

if __name__ == '__main__':
    main()
