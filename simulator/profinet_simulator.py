#!/usr/bin/env python3
import json
import time
import random
import socket
import struct
from datetime import datetime
from dataclasses import dataclass, field
from typing import List, Dict

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

def create_profinet_packet(timestamp: float, electrolyzer_id: int, sensors_data: List[Dict]) -> bytes:
    payload = json.dumps({
        'timestamp': timestamp,
        'electrolyzer_id': electrolyzer_id,
        'sensors': sensors_data
    }).encode('utf-8')
    
    header = struct.pack('!II', 0x0001, len(payload))
    return header + payload

def send_profinet_data(host: str, port: int, electrolyzers: List[Electrolyzer]):
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    packet_count = 0
    
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
                
                packet = create_profinet_packet(timestamp, electrolyzer.id, sensors_data)
                sock.sendto(packet, (host, port))
                
                packet_count += 1
                if packet_count % 50 == 0:
                    print(f"[{datetime.now().strftime('%H:%M:%S')}] Sent {packet_count} packets, "
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
    host = '127.0.0.1'
    port = 34567
    
    print("=" * 60)
    print("Profinet Simulator for PEM Electrolyzer System")
    print("=" * 60)
    print(f"Electrolyzers: {ELECTROLYZER_COUNT}")
    print(f"Sensors per electrolyzer: {SENSORS_PER_ELECTROLYZER}")
    print(f"Total sensors: {ELECTROLYZER_COUNT * SENSORS_PER_ELECTROLYZER}")
    print(f"Report interval: 2 seconds")
    print(f"Target: {host}:{port}")
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
    send_profinet_data(host, port, electrolyzers)

if __name__ == '__main__':
    main()
