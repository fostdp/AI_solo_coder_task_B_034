#!/usr/bin/env python3
import socket
import struct
import time
import random
import argparse
import threading
import json
import zlib
from http.server import HTTPServer, BaseHTTPRequestHandler
from typing import Dict, List, Optional, Tuple
from dataclasses import dataclass, field
from datetime import datetime

MAGIC_NUMBER = 0x50524F4E
PACKET_VERSION = 1

SENSOR_TYPES = {
    'voltage': 1,
    'current_density': 2,
    'hydrogen_flow': 3,
    'oxygen_flow': 4,
    'water_temp': 5,
    'membrane_conductivity': 6,
    'hydrogen_purity': 7,
    'cell_voltage': 8,
}

SENSOR_LOCATIONS = {
    'anode': 1,
    'cathode': 2,
    'membrane': 3,
}

@dataclass
class FaultInjection:
    efficiency_drop_electrolyzers: List[int] = field(default_factory=list)
    efficiency_drop_factor: float = 0.85
    membrane_degradation_electrolyzers: List[int] = field(default_factory=list)
    membrane_degradation_rate: float = 0.01
    high_voltage_electrolyzers: List[int] = field(default_factory=list)
    low_purity_electrolyzers: List[int] = field(default_factory=list)

@dataclass
class SensorReading:
    sensor_id: int
    sensor_type: int
    location: int
    value: float
    rated_value: float

class ElectrolyzerSimulator:
    def __init__(self, electrolyzer_id: int, sensors_count: int = 50):
        self.electrolyzer_id = electrolyzer_id
        self.sensors_count = sensors_count
        self.sensors: List[Dict] = []
        self.base_conductivity = 0.12
        self._initialize_sensors()
    
    def _initialize_sensors(self):
        sensor_configs = [
            ('cell_voltage', 'anode', 1.85, 1.85, 15),
            ('cell_voltage', 'cathode', 1.85, 1.85, 15),
            ('cell_voltage', 'membrane', 1.85, 1.85, 10),
            ('current_density', 'anode', 2.0, 2.0, 2),
            ('current_density', 'membrane', 2.0, 2.0, 1),
            ('water_temp', 'anode', 60.0, 60.0, 2),
            ('water_temp', 'cathode', 60.0, 60.0, 2),
            ('water_temp', 'membrane', 60.0, 60.0, 1),
            ('hydrogen_flow', 'cathode', 45.0, 45.0, 3),
            ('oxygen_flow', 'anode', 22.5, 22.5, 3),
            ('membrane_conductivity', 'membrane', self.base_conductivity, self.base_conductivity, 3),
            ('hydrogen_purity', 'cathode', 99.97, 99.97, 3),
        ]
        
        sensor_id = 1
        for sensor_type, location, value, rated, count in sensor_configs:
            for _ in range(count):
                self.sensors.append({
                    'sensor_id': sensor_id,
                    'sensor_type': sensor_type,
                    'location': location,
                    'base_value': value,
                    'rated_value': rated,
                    'noise': value * 0.005,
                })
                sensor_id += 1
        
        while len(self.sensors) < self.sensors_count:
            self.sensors.append({
                'sensor_id': sensor_id,
                'sensor_type': 'voltage',
                'location': random.choice(['anode', 'cathode', 'membrane']),
                'base_value': 1.85,
                'rated_value': 1.85,
                'noise': 0.01,
            })
            sensor_id += 1
    
    def generate_readings(self, faults: FaultInjection, timestamp: int) -> List[SensorReading]:
        readings = []
        
        efficiency_drop = self.electrolyzer_id in faults.efficiency_drop_electrolyzers
        membrane_deg = self.electrolyzer_id in faults.membrane_degradation_electrolyzers
        high_voltage = self.electrolyzer_id in faults.high_voltage_electrolyzers
        low_purity = self.electrolyzer_id in faults.low_purity_electrolyzers
        
        for sensor in self.sensors:
            value = sensor['base_value'] + random.gauss(0, sensor['noise'])
            
            if sensor['sensor_type'] == 'cell_voltage':
                if high_voltage:
                    value = 2.05 + random.gauss(0, 0.02)
                elif efficiency_drop:
                    value = 2.0 + random.gauss(0, 0.02)
            
            elif sensor['sensor_type'] == 'current_density':
                if efficiency_drop:
                    value = value * faults.efficiency_drop_factor
            
            elif sensor['sensor_type'] == 'membrane_conductivity':
                if membrane_deg:
                    self.base_conductivity = max(0.05, self.base_conductivity - faults.membrane_degradation_rate)
                    value = self.base_conductivity
                else:
                    self.base_conductivity = min(0.12, self.base_conductivity + faults.membrane_degradation_rate * 0.5)
            
            elif sensor['sensor_type'] == 'hydrogen_purity':
                if low_purity:
                    value = 99.85 + random.gauss(0, 0.02)
                elif efficiency_drop:
                    value = 99.92 + random.gauss(0, 0.02)
            
            elif sensor['sensor_type'] in ['hydrogen_flow', 'oxygen_flow']:
                if efficiency_drop:
                    value = value * faults.efficiency_drop_factor
            
            elif sensor['sensor_type'] == 'water_temp':
                if efficiency_drop:
                    value = value + 5.0
            
            readings.append(SensorReading(
                sensor_id=sensor['sensor_id'],
                sensor_type=SENSOR_TYPES[sensor['sensor_type']],
                location=SENSOR_LOCATIONS[sensor['location']],
                value=max(0, value),
                rated_value=sensor['rated_value'],
            ))
        
        return readings

def build_profinet_packet(electrolyzer_id: int, readings: List[SensorReading], timestamp_ms: int) -> bytes:
    header = struct.pack('!IBBH', MAGIC_NUMBER, timestamp_ms, electrolyzer_id, len(readings))
    
    payload = b''
    for reading in readings:
        payload += struct.pack('!HB B dd',
            reading.sensor_id,
            reading.sensor_type,
            reading.location,
            reading.value,
            reading.rated_value,
        )
    
    crc = zlib.crc32(header + payload) & 0xFFFFFFFF
    packet = header + payload + struct.pack('!I', crc)
    
    return packet

class FaultAPIHandler(BaseHTTPRequestHandler):
    faults: FaultInjection = None
    simulators: Dict[int, ElectrolyzerSimulator] = None
    
    def log_message(self, format, *args):
        pass
    
    def do_GET(self):
        if self.path == '/status':
            status = {
                'faults': {
                    'efficiency_drop': list(self.faults.efficiency_drop_electrolyzers),
                    'efficiency_drop_factor': self.faults.efficiency_drop_factor,
                    'membrane_degradation': list(self.faults.membrane_degradation_electrolyzers),
                    'membrane_degradation_rate': self.faults.membrane_degradation_rate,
                    'high_voltage': list(self.faults.high_voltage_electrolyzers),
                    'low_purity': list(self.faults.low_purity_electrolyzers),
                },
                'electrolyzers': list(self.simulators.keys()),
            }
            self.send_json(status)
        elif self.path == '/':
            help_text = {
                'endpoints': {
                    'GET /status': 'Get current fault injection status',
                    'POST /inject/efficiency_drop': 'Inject efficiency drop {\"electrolyzer_ids\": [1,2,3], \"factor\": 0.85}',
                    'POST /inject/membrane_degradation': 'Inject membrane degradation {\"electrolyzer_ids\": [1,2], \"rate\": 0.01}',
                    'POST /inject/high_voltage': 'Inject high voltage {\"electrolyzer_ids\": [1]}',
                    'POST /inject/low_purity': 'Inject low purity {\"electrolyzer_ids\": [2]}',
                    'POST /clear': 'Clear all faults {\"electrolyzer_ids\": [1,2]} or clear all',
                    'POST /reset_efficiency_drop': 'Reset efficiency drop for electrolyzers {\"electrolyzer_ids\": [1,2]}',
                    'POST /reset_membrane_degradation': 'Reset membrane degradation {\"electrolyzer_ids\": [1,2]}',
                }
            }
            self.send_json(help_text)
        else:
            self.send_error(404)
    
    def do_POST(self):
        content_length = int(self.headers.get('Content-Length', 0))
        body = self.rfile.read(content_length).decode('utf-8')
        data = json.loads(body) if body else {}
        
        try:
            if self.path == '/inject/efficiency_drop':
                ids = data.get('electrolyzer_ids', [])
                factor = data.get('factor', self.faults.efficiency_drop_factor)
                for id in ids:
                    if id not in self.faults.efficiency_drop_electrolyzers:
                        self.faults.efficiency_drop_electrolyzers.append(id)
                self.faults.efficiency_drop_factor = factor
                self.send_json({'success': True, 'message': f'Efficiency drop injected for {ids}, factor={factor}'})
            
            elif self.path == '/inject/membrane_degradation':
                ids = data.get('electrolyzer_ids', [])
                rate = data.get('rate', self.faults.membrane_degradation_rate)
                for id in ids:
                    if id not in self.faults.membrane_degradation_electrolyzers:
                        self.faults.membrane_degradation_electrolyzers.append(id)
                self.faults.membrane_degradation_rate = rate
                self.send_json({'success': True, 'message': f'Membrane degradation injected for {ids}, rate={rate}/s'})
            
            elif self.path == '/inject/high_voltage':
                ids = data.get('electrolyzer_ids', [])
                for id in ids:
                    if id not in self.faults.high_voltage_electrolyzers:
                        self.faults.high_voltage_electrolyzers.append(id)
                self.send_json({'success': True, 'message': f'High voltage injected for {ids}'})
            
            elif self.path == '/inject/low_purity':
                ids = data.get('electrolyzer_ids', [])
                for id in ids:
                    if id not in self.faults.low_purity_electrolyzers:
                        self.faults.low_purity_electrolyzers.append(id)
                self.send_json({'success': True, 'message': f'Low purity injected for {ids}'})
            
            elif self.path == '/reset_efficiency_drop':
                ids = data.get('electrolyzer_ids', [])
                if ids:
                    for id in ids:
                        if id in self.faults.efficiency_drop_electrolyzers:
                            self.faults.efficiency_drop_electrolyzers.remove(id)
                    self.send_json({'success': True, 'message': f'Efficiency drop reset for {ids}'})
                else:
                    self.faults.efficiency_drop_electrolyzers.clear()
                    self.send_json({'success': True, 'message': 'All efficiency drop faults reset'})
            
            elif self.path == '/reset_membrane_degradation':
                ids = data.get('electrolyzer_ids', [])
                if ids:
                    for id in ids:
                        if id in self.faults.membrane_degradation_electrolyzers:
                            self.faults.membrane_degradation_electrolyzers.remove(id)
                        if id in self.simulators:
                            self.simulators[id].base_conductivity = 0.12
                    self.send_json({'success': True, 'message': f'Membrane degradation reset for {ids}'})
                else:
                    self.faults.membrane_degradation_electrolyzers.clear()
                    for sim in self.simulators.values():
                        sim.base_conductivity = 0.12
                    self.send_json({'success': True, 'message': 'All membrane degradation faults reset'})
            
            elif self.path == '/clear':
                ids = data.get('electrolyzer_ids', [])
                if ids:
                    for id in ids:
                        for lst in [self.faults.efficiency_drop_electrolyzers,
                                   self.faults.membrane_degradation_electrolyzers,
                                   self.faults.high_voltage_electrolyzers,
                                   self.faults.low_purity_electrolyzers]:
                            if id in lst:
                                lst.remove(id)
                        if id in self.simulators:
                            self.simulators[id].base_conductivity = 0.12
                    self.send_json({'success': True, 'message': f'All faults cleared for {ids}'})
                else:
                    self.faults.efficiency_drop_electrolyzers.clear()
                    self.faults.membrane_degradation_electrolyzers.clear()
                    self.faults.high_voltage_electrolyzers.clear()
                    self.faults.low_purity_electrolyzers.clear()
                    for sim in self.simulators.values():
                        sim.base_conductivity = 0.12
                    self.send_json({'success': True, 'message': 'All faults cleared'})
            
            else:
                self.send_error(404)
        
        except Exception as e:
            self.send_json({'success': False, 'error': str(e)}, status=500)
    
    def send_json(self, data, status=200):
        self.send_response(status)
        self.send_header('Content-Type', 'application/json')
        self.end_headers()
        self.wfile.write(json.dumps(data, indent=2).encode('utf-8'))

def run_api_server(port: int, faults: FaultInjection, simulators: Dict[int, ElectrolyzerSimulator]):
    FaultAPIHandler.faults = faults
    FaultAPIHandler.simulators = simulators
    server = HTTPServer(('0.0.0.0', port), FaultAPIHandler)
    print(f"[API] Fault injection API running on http://0.0.0.0:{port}")
    server.serve_forever()

def main():
    parser = argparse.ArgumentParser(description='Profinet PEM Electrolyzer Simulator')
    parser.add_argument('--host', default='127.0.0.1', help='Target host')
    parser.add_argument('--port', type=int, default=34567, help='Target port')
    parser.add_argument('--electrolyzers', type=int, default=10, help='Number of electrolyzers')
    parser.add_argument('--sensors', type=int, default=50, help='Sensors per electrolyzer')
    parser.add_argument('--interval', type=float, default=2.0, help='Send interval in seconds')
    parser.add_argument('--api-port', type=int, default=8081, help='Fault injection API port')
    parser.add_argument('--jitter', type=float, default=0.1, help='Packet send jitter (0-1)')
    parser.add_argument('--initial-faults', action='store_true', help='Start with demo faults')
    args = parser.parse_args()
    
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    
    faults = FaultInjection()
    simulators = {}
    
    for i in range(1, args.electrolyzers + 1):
        simulators[i] = ElectrolyzerSimulator(i, args.sensors)
    
    if args.initial_faults:
        faults.efficiency_drop_electrolyzers = [1, 2, 3]
        faults.membrane_degradation_electrolyzers = [5]
    
    api_thread = threading.Thread(target=run_api_server, args=(args.api_port, faults, simulators), daemon=True)
    api_thread.start()
    
    print(f"{'='*60}")
    print(f"Profinet PEM Electrolyzer Simulator")
    print(f"{'='*60}")
    print(f"Target: {args.host}:{args.port}")
    print(f"Electrolyzers: {args.electrolyzers}")
    print(f"Sensors per electrolyzer: {args.sensors}")
    print(f"Interval: {args.interval}s")
    print(f"Total sensors: {args.electrolyzers * args.sensors}")
    print(f"Data rate: ~{args.electrolyzers * args.sensors / args.interval:.0f} points/sec")
    print(f"{'='*60}")
    print(f"Fault Injection API: http://localhost:{args.api_port}")
    print(f"  GET  /status - Current status")
    print(f"  POST /inject/efficiency_drop '{{\"electrolyzer_ids\":[1,2], \"factor\":0.85}}'")
    print(f"  POST /inject/membrane_degradation '{{\"electrolyzer_ids\":[5], \"rate\":0.01}}'")
    print(f"  POST /inject/high_voltage '{{\"electrolyzer_ids\":[1]}}'")
    print(f"  POST /inject/low_purity '{{\"electrolyzer_ids\":[2]}}'")
    print(f"  POST /clear '{{\"electrolyzer_ids\":[1,2]}}' (or empty to clear all)")
    print(f"  POST /reset_efficiency_drop '{{\"electrolyzer_ids\":[1]}}'")
    print(f"  POST /reset_membrane_degradation '{{\"electrolyzer_ids\":[5]}}'")
    print(f"{'='*60}")
    
    if args.initial_faults:
        print(f"[DEMO] Initial faults:")
        print(f"  - Efficiency drop: electrolyzers {faults.efficiency_drop_electrolyzers}")
        print(f"  - Membrane degradation: electrolyzers {faults.membrane_degradation_electrolyzers}")
        print(f"{'='*60}")
    
    packet_count = 0
    start_time = time.time()
    
    try:
        while True:
            cycle_start = time.time()
            
            for electrolyzer_id, simulator in simulators.items():
                timestamp_ms = int(time.time() * 1000)
                readings = simulator.generate_readings(faults, timestamp_ms)
                packet = build_profinet_packet(electrolyzer_id, readings, timestamp_ms)
                sock.sendto(packet, (args.host, args.port))
                packet_count += 1
                
                if packet_count % (args.electrolyzers * 10) == 0:
                    elapsed = time.time() - start_time
                    print(f"[{datetime.now().strftime('%H:%M:%S')}] "
                          f"Sent {packet_count} packets, "
                          f"{packet_count/elapsed:.1f} pkt/s, "
                          f"Faults: eff_drop={len(faults.efficiency_drop_electrolyzers)} "
                          f"mem_deg={len(faults.membrane_degradation_electrolyzers)} "
                          f"high_v={len(faults.high_voltage_electrolyzers)} "
                          f"low_pur={len(faults.low_purity_electrolyzers)}")
            
            cycle_time = time.time() - cycle_start
            sleep_time = args.interval - cycle_time
            if sleep_time > 0:
                if args.jitter > 0:
                    sleep_time *= (1 - args.jitter / 2 + random.random() * args.jitter)
                time.sleep(max(0, sleep_time))
    
    except KeyboardInterrupt:
        elapsed = time.time() - start_time
        print(f"\n{'='*60}")
        print(f"Simulator stopped.")
        print(f"Total packets: {packet_count}")
        print(f"Total time: {elapsed:.1f}s")
        print(f"Average rate: {packet_count/elapsed:.1f} pkt/s")
        print(f"{'='*60}")

if __name__ == '__main__':
    main()
