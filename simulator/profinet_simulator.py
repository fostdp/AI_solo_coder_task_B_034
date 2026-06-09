#!/usr/bin/env python3
import json
import time
import random
import socket
import struct
import zlib
from datetime import datetime
from dataclasses import dataclass, field
from typing import List, Dict, Optional

MAGIC_NUMBER = 0x50524F4E  # "PRON" in ASCII
PACKET_HEADER_SIZE = 8
CRC_SIZE = 4
CRC_POLYNOMIAL = 0xEDB88320  # CRC-32 (IEEE)


SENSOR_TYPES = [
    'voltage',
    'current_density',
    'hydrogen_flow',
    'oxygen_flow',
    'water_temp',
    'membrane_conductivity',
    'hydrogen_purity',
    'cell_voltage'
]

LOCATIONS = ['anode', 'cathode', 'membrane']

RATED_VALUES = {
    'voltage': 1.85,
    'current_density': 2.0,
    'hydrogen_flow': 500.0,
    'oxygen_flow': 250.0,
    'water_temp': 60.0,
    'membrane_conductivity': 0.1,
    'hydrogen_purity': 99.97,
    'cell_voltage': 1.85
}

SENSORS_PER_ELECTROLYZER = 50
ELECTROLYZER_COUNT = 10

@dataclass
class SensorConfig:
    sensor_id: int
    sensor_type: str
    location: str
    rated_value: float
    x: float
    y: float

@dataclass
class Electrolyzer:
    id: int
    sensors: List[SensorConfig] = field(default_factory=list)
    base_current_density: float = 2.0
    base_water_temp: float = 60.0
    degradation_factor: float = 1.0
    anomaly_mode: bool = False

def generate_sensor_configs(electrolyzer_id: int) -> List[SensorConfig]:
    sensors = []
    for i in range(SENSORS_PER_ELECTROLYZER):
        sensor_type = SENSOR_TYPES[i % len(SENSOR_TYPES)]
        location = LOCATIONS[i % len(LOCATIONS)]
        
        if location == 'anode':
            x = random.uniform(0.05, 0.25)
            y = random.uniform(0.1, 0.9)
        elif location == 'cathode':
            x = random.uniform(0.75, 0.95)
            y = random.uniform(0.1, 0.9)
        else:
            x = random.uniform(0.4, 0.6)
            y = random.uniform(0.1, 0.9)
        
        sensors.append(SensorConfig(
            sensor_id=electrolyzer_id * 1000 + i,
            sensor_type=sensor_type,
            location=location,
            rated_value=RATED_VALUES[sensor_type],
            x=x,
            y=y
        ))
    return sensors

def generate_sensor_value(sensor: SensorConfig, electrolyzer: Electrolyzer) -> float:
    base = sensor.rated_value
    noise = random.gauss(0, base * 0.01)
    drift = base * (electrolyzer.degradation_factor - 1) * 0.1
    
    if electrolyzer.anomaly_mode and random.random() < 0.3:
        anomaly_factor = random.uniform(1.1, 1.3)
        value = base * anomaly_factor + noise + drift
    else:
        if sensor.sensor_type == 'current_density':
            value = electrolyzer.base_current_density + noise * 0.05
        elif sensor.sensor_type == 'water_temp':
            value = electrolyzer.base_water_temp + noise * 0.5
        elif sensor.sensor_type == 'hydrogen_purity':
            value = min(99.999, base + noise * 0.01 - drift * 0.5)
        elif sensor.sensor_type == 'membrane_conductivity':
            value = max(0.05, base + noise * 0.005 - drift * 0.02)
        else:
            value = base + noise + drift
    
    return round(value, 6)

def calculate_crc32(data: bytes) -> int:
    """
    Calculate CRC-32 (IEEE Std 802.3) checksum for data.
    Matches Rust's crc::CRC_32_ISCSI (0x1EDC6F41 polynomial, reversed).
    """
    crc = zlib.crc32(data)
    return crc & 0xFFFFFFFF

def create_profinet_packet(timestamp: float, electrolyzer_id: int, sensors_data: List[Dict], 
                           inject_crc_error: bool = False) -> bytes:
    """
    Create a Profinet packet with proper frame format:
    [magic:4][payload_len:4][payload:N][crc:4]
    
    Args:
        timestamp: Unix timestamp with fractional seconds
        electrolyzer_id: Electrolyzer ID (1-10)
        sensors_data: List of sensor reading dictionaries
        inject_crc_error: If True, inject an invalid CRC for testing
    """
    payload = json.dumps({
        'timestamp': timestamp,
        'electrolyzer_id': electrolyzer_id,
        'sensors': sensors_data
    }).encode('utf-8')
    
    header = struct.pack('!II', MAGIC_NUMBER, len(payload))
    
    crc = calculate_crc32(payload)
    if inject_crc_error:
        crc = (crc ^ 0xFFFFFFFF) & 0xFFFFFFFF
    
    crc_bytes = struct.pack('!I', crc)
    
    return header + payload + crc_bytes

def create_invalid_packet(reason: str = "random") -> bytes:
    """
    Create various invalid packets for testing error handling.
    """
    if reason == "too_short":
        return b'\x00\x00\x00'
    elif reason == "bad_magic":
        return struct.pack('!II', 0xDEADBEEF, 4) + b'test' + struct.pack('!I', 0)
    elif reason == "wrong_length":
        payload = b'{"test":1}'
        header = struct.pack('!II', MAGIC_NUMBER, 1000)
        crc = struct.pack('!I', calculate_crc32(payload))
        return header + payload + crc
    elif reason == "bad_crc":
        payload = b'{"test":1}'
        header = struct.pack('!II', MAGIC_NUMBER, len(payload))
        crc = struct.pack('!I', 0xDEADBEEF)
        return header + payload + crc
    else:
        return b'\x00' * 64

def send_profinet_data(host: str, port: int, electrolyzers: List[Electrolyzer],
                        inject_invalid_packets: bool = True):
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    packet_count = 0
    invalid_packet_count = 0
    
    try:
        while True:
            timestamp = time.time()
            
            for electrolyzer in electrolyzers:
                sensors_data = []
                
                for sensor in electrolyzer.sensors:
                    value = generate_sensor_value(sensor, electrolyzer)
                    sensors_data.append({
                        'sensor_id': sensor.sensor_id,
                        'sensor_type': sensor.sensor_type,
                        'location': sensor.location,
                        'value': value,
                        'rated_value': sensor.rated_value,
                        'x': sensor.x,
                        'y': sensor.y
                    })
                
                inject_error = False
                if inject_invalid_packets and random.random() < 0.005:
                    inject_error = True
                    error_type = random.choice(['bad_crc', 'bad_magic', 'wrong_length', 'too_short'])
                    if error_type == 'bad_crc':
                        packet = create_profinet_packet(timestamp, electrolyzer.id, sensors_data, inject_crc_error=True)
                    else:
                        packet = create_invalid_packet(error_type)
                    
                    sock.sendto(packet, (host, port))
                    invalid_packet_count += 1
                    print(f"[{datetime.now().strftime('%H:%M:%S')}] ⚠️  Injected invalid packet ({error_type}) "
                          f"for electrolyzer {electrolyzer.id} (total invalid: {invalid_packet_count})")
                
                packet = create_profinet_packet(timestamp, electrolyzer.id, sensors_data)
                sock.sendto(packet, (host, port))
                
                packet_count += 1
                if packet_count % 50 == 0:
                    print(f"[{datetime.now().strftime('%H:%M:%S')}] Sent {packet_count} valid packets, "
                          f"{invalid_packet_count} invalid packets, "
                          f"Electrolyzer {electrolyzer.id}: "
                          f"Current={electrolyzer.base_current_density:.2f} A/cm², "
                          f"Temp={electrolyzer.base_water_temp:.1f}°C")
            
            if random.random() < 0.002:
                target = random.choice(electrolyzers)
                target.anomaly_mode = True
                target.base_current_density = random.uniform(0.8, 1.2)
                target.base_water_temp = random.uniform(50, 75)
                print(f"⚠️  Anomaly triggered on electrolyzer {target.id}!")
                
                time.sleep(random.randint(30, 120))
                target.anomaly_mode = False
                target.base_current_density = 2.0
                target.base_water_temp = 60.0
                print(f"✅ Anomaly cleared on electrolyzer {target.id}")
            
            if random.random() < 0.0005:
                target = random.choice(electrolyzers)
                target.degradation_factor += random.uniform(0.005, 0.02)
                print(f"📉 Degradation updated on electrolyzer {target.id}: "
                      f"factor={target.degradation_factor:.4f}")
            
            time.sleep(2)
            
    except KeyboardInterrupt:
        print("\nSimulator stopped.")
    finally:
        sock.close()

def main():
    import argparse
    
    parser = argparse.ArgumentParser(description='Profinet Simulator for PEM Electrolyzer System')
    parser.add_argument('--host', default='127.0.0.1', help='Target host IP address')
    parser.add_argument('--port', type=int, default=34567, help='Target UDP port')
    parser.add_argument('--no-inject-errors', action='store_true', 
                        help='Disable injection of invalid packets for testing')
    parser.add_argument('--seed', type=int, help='Random seed for reproducible testing')
    
    args = parser.parse_args()
    
    if args.seed is not None:
        random.seed(args.seed)
    
    host = args.host
    port = args.port
    inject_errors = not args.no_inject_errors
    
    print("=" * 60)
    print("Profinet Simulator for PEM Electrolyzer System")
    print("=" * 60)
    print(f"Electrolyzers: {ELECTROLYZER_COUNT}")
    print(f"Sensors per electrolyzer: {SENSORS_PER_ELECTROLYZER}")
    print(f"Total sensors: {ELECTROLYZER_COUNT * SENSORS_PER_ELECTROLYZER}")
    print(f"Report interval: 2 seconds")
    print(f"Target: {host}:{port}")
    print(f"Packet format: [magic:4][len:4][payload:N][crc:4]")
    print(f"Magic number: 0x{MAGIC_NUMBER:08X} ('PRON')")
    print(f"CRC polynomial: CRC-32 IEEE (0x{CRC_POLYNOMIAL:08X})")
    print(f"Inject invalid packets: {'YES (0.5% probability)' if inject_errors else 'NO'}")
    print("=" * 60)
    
    electrolyzers = []
    for i in range(ELECTROLYZER_COUNT):
        electrolyzer = Electrolyzer(
            id=i + 1,
            sensors=generate_sensor_configs(i + 1),
            base_current_density=2.0 + random.gauss(0, 0.1),
            base_water_temp=60.0 + random.gauss(0, 2),
            degradation_factor=1.0 + random.uniform(-0.02, 0.05)
        )
        electrolyzers.append(electrolyzer)
    
    print("Starting simulator... Press Ctrl+C to stop.")
    send_profinet_data(host, port, electrolyzers, inject_invalid_packets=inject_errors)

if __name__ == '__main__':
    main()
