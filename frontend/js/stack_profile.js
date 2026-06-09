class StackProfile {
    constructor(canvasId) {
        this.canvas = document.getElementById(canvasId);
        this.ctx = this.canvas.getContext('2d');
        this.sensors = [];
        this.electrolyzerData = null;
        this.selectedSensor = null;
        this.hoveredSensor = null;
        this.onSensorClick = null;
        this.onSensorHover = null;
        
        this.setupCanvas();
        this.setupEventListeners();
    }

    setupCanvas() {
        const rect = this.canvas.getBoundingClientRect();
        const dpr = window.devicePixelRatio || 1;
        
        this.canvas.width = rect.width * dpr;
        this.canvas.height = rect.height * dpr;
        this.ctx.scale(dpr, dpr);
        
        this.displayWidth = rect.width;
        this.displayHeight = rect.height;
    }

    setupEventListeners() {
        this.canvas.addEventListener('click', (e) => this.handleClick(e));
        this.canvas.addEventListener('mousemove', (e) => this.handleMouseMove(e));
        this.canvas.addEventListener('mouseleave', () => this.handleMouseLeave());
        
        window.addEventListener('resize', () => {
            this.setupCanvas();
            this.render();
        });
    }

    setData(electrolyzerData) {
        this.electrolyzerData = electrolyzerData;
        this.sensors = electrolyzerData?.sensors || [];
        this.render();
    }

    getCanvasCoords(e) {
        const rect = this.canvas.getBoundingClientRect();
        return {
            x: e.clientX - rect.left,
            y: e.clientY - rect.top
        };
    }

    handleClick(e) {
        const coords = this.getCanvasCoords(e);
        const sensor = this.findSensorAtPosition(coords.x, coords.y);
        
        if (sensor && this.onSensorClick) {
            this.selectedSensor = sensor.sensor_id;
            this.onSensorClick(sensor);
            this.render();
        }
    }

    handleMouseMove(e) {
        const coords = this.getCanvasCoords(e);
        const sensor = this.findSensorAtPosition(coords.x, coords.y);
        
        if (sensor) {
            this.canvas.style.cursor = 'pointer';
            this.hoveredSensor = sensor.sensor_id;
            
            if (this.onSensorHover) {
                this.onSensorHover(sensor);
            }
        } else {
            this.canvas.style.cursor = 'default';
            this.hoveredSensor = null;
            
            if (this.onSensorHover) {
                this.onSensorHover(null);
            }
        }
        
        this.render();
    }

    handleMouseLeave() {
        this.hoveredSensor = null;
        this.canvas.style.cursor = 'default';
        
        if (this.onSensorHover) {
            this.onSensorHover(null);
        }
        
        this.render();
    }

    findSensorAtPosition(x, y) {
        const sensorRadius = 8;
        
        for (const sensor of this.sensors) {
            const sensorX = sensor.x * this.displayWidth;
            const sensorY = sensor.y * this.displayHeight;
            
            const distance = Math.sqrt(
                Math.pow(x - sensorX, 2) + Math.pow(y - sensorY, 2)
            );
            
            if (distance <= sensorRadius) {
                return sensor;
            }
        }
        
        return null;
    }

    getDeviationColor(deviationPercent) {
        const absDeviation = Math.abs(deviationPercent);
        
        if (absDeviation < 3) {
            return '#4CAF50';
        } else if (absDeviation < 7) {
            return '#FFC107';
        } else {
            return '#F44336';
        }
    }

    render() {
        this.ctx.clearRect(0, 0, this.displayWidth, this.displayHeight);
        
        this.drawBackground();
        this.drawMembraneElectrodeAssembly();
        this.drawLabels();
        this.drawSensors();
    }

    drawBackground() {
        const gradient = this.ctx.createLinearGradient(0, 0, 0, this.displayHeight);
        gradient.addColorStop(0, '#1a1a2e');
        gradient.addColorStop(1, '#16213e');
        
        this.ctx.fillStyle = gradient;
        this.ctx.fillRect(0, 0, this.displayWidth, this.displayHeight);
        
        this.ctx.strokeStyle = 'rgba(255, 255, 255, 0.03)';
        this.ctx.lineWidth = 1;
        
        const gridSize = 40;
        for (let x = 0; x < this.displayWidth; x += gridSize) {
            this.ctx.beginPath();
            this.ctx.moveTo(x, 0);
            this.ctx.lineTo(x, this.displayHeight);
            this.ctx.stroke();
        }
        
        for (let y = 0; y < this.displayHeight; y += gridSize) {
            this.ctx.beginPath();
            this.ctx.moveTo(0, y);
            this.ctx.lineTo(this.displayWidth, y);
            this.ctx.stroke();
        }
    }

    drawMembraneElectrodeAssembly() {
        const centerX = this.displayWidth / 2;
        const centerY = this.displayHeight / 2;
        
        const totalWidth = this.displayWidth * 0.8;
        const totalHeight = this.displayHeight * 0.7;
        
        const leftX = centerX - totalWidth / 2;
        const topY = centerY - totalHeight / 2;
        
        const layerWidth = totalWidth / 7;
        
        const layers = [
            { name: '阳极集流体', color: '#8B4513', width: layerWidth * 0.6 },
            { name: '阳极扩散层', color: '#2F4F4F', width: layerWidth * 0.8 },
            { name: '阳极催化剂层', color: '#4A90D9', width: layerWidth * 0.4 },
            { name: '质子交换膜', color: '#E74C3C', width: layerWidth * 0.5 },
            { name: '阴极催化剂层', color: '#9B59B6', width: layerWidth * 0.4 },
            { name: '阴极扩散层', color: '#2F4F4F', width: layerWidth * 0.8 },
            { name: '阴极集流体', color: '#8B4513', width: layerWidth * 0.6 },
        ];
        
        let currentX = leftX + (totalWidth - layers.reduce((sum, l) => sum + l.width, 0)) / 2;
        
        for (const layer of layers) {
            const layerX = currentX;
            const layerY = topY;
            const layerH = totalHeight;
            
            const gradient = this.ctx.createLinearGradient(layerX, layerY, layerX + layer.width, layerY);
            gradient.addColorStop(0, this.adjustColor(layer.color, -30));
            gradient.addColorStop(0.5, layer.color);
            gradient.addColorStop(1, this.adjustColor(layer.color, -30));
            
            this.ctx.fillStyle = gradient;
            this.ctx.fillRect(layerX, layerY, layer.width, layerH);
            
            this.ctx.strokeStyle = 'rgba(255, 255, 255, 0.2)';
            this.ctx.lineWidth = 1;
            this.ctx.strokeRect(layerX, layerY, layer.width, layerH);
            
            this.ctx.save();
            this.ctx.translate(layerX + layer.width / 2, centerY);
            this.ctx.rotate(-Math.PI / 2);
            this.ctx.fillStyle = 'rgba(255, 255, 255, 0.9)';
            this.ctx.font = '12px "Segoe UI", sans-serif';
            this.ctx.textAlign = 'center';
            this.ctx.textBaseline = 'middle';
            this.ctx.fillText(layer.name, 0, 0);
            this.ctx.restore();
            
            currentX += layer.width;
        }
        
        this.ctx.fillStyle = 'rgba(0, 212, 255, 0.3)';
        this.ctx.fillRect(leftX, topY - 20, layerWidth * 0.6 + layerWidth * 0.8 + layerWidth * 0.4, 15);
        this.ctx.fillStyle = '#00d4ff';
        this.ctx.font = '11px "Segoe UI", sans-serif';
        this.ctx.textAlign = 'center';
        this.ctx.fillText('H₂O → O₂ + 4H⁺ + 4e⁻', leftX + totalWidth * 0.25, topY - 8);
        
        this.ctx.fillStyle = 'rgba(0, 255, 136, 0.3)';
        this.ctx.fillRect(leftX + totalWidth - (layerWidth * 0.6 + layerWidth * 0.8 + layerWidth * 0.4), topY - 20, layerWidth * 0.6 + layerWidth * 0.8 + layerWidth * 0.4, 15);
        this.ctx.fillStyle = '#00ff88';
        this.ctx.fillText('2H⁺ + 2e⁻ → H₂', leftX + totalWidth * 0.75, topY - 8);
    }

    drawLabels() {
        const centerX = this.displayWidth / 2;
        const centerY = this.displayHeight / 2;
        const totalWidth = this.displayWidth * 0.8;
        const totalHeight = this.displayHeight * 0.7;
        const leftX = centerX - totalWidth / 2;
        const topY = centerY - totalHeight / 2;
        
        this.ctx.fillStyle = '#FF6B6B';
        this.ctx.font = 'bold 14px "Segoe UI", sans-serif';
        this.ctx.textAlign = 'center';
        this.ctx.fillText('阳极侧 (Anode)', leftX + totalWidth * 0.15, topY + totalHeight + 30);
        
        this.ctx.fillStyle = '#4ECDC4';
        this.ctx.fillText('阴极侧 (Cathode)', leftX + totalWidth * 0.85, topY + totalHeight + 30);
        
        this.ctx.fillStyle = '#E74C3C';
        this.ctx.fillText('膜电极组件 (MEA)', centerX, topY + totalHeight + 50);
        
        this.ctx.beginPath();
        this.ctx.moveTo(leftX + totalWidth * 0.05, topY + totalHeight + 35);
        this.ctx.lineTo(leftX + totalWidth * 0.05, topY + totalHeight + 45);
        this.ctx.lineTo(leftX + totalWidth * 0.35, topY + totalHeight + 45);
        this.ctx.strokeStyle = '#FF6B6B';
        this.ctx.lineWidth = 2;
        this.ctx.stroke();
        
        this.ctx.beginPath();
        this.ctx.moveTo(leftX + totalWidth * 0.65, topY + totalHeight + 45);
        this.ctx.lineTo(leftX + totalWidth * 0.95, topY + totalHeight + 45);
        this.ctx.lineTo(leftX + totalWidth * 0.95, topY + totalHeight + 35);
        this.ctx.strokeStyle = '#4ECDC4';
        this.ctx.stroke();
    }

    drawSensors() {
        for (const sensor of this.sensors) {
            const x = sensor.x * this.displayWidth;
            const y = sensor.y * this.displayHeight;
            const color = this.getDeviationColor(sensor.deviation_percent);
            
            const isSelected = this.selectedSensor === sensor.sensor_id;
            const isHovered = this.hoveredSensor === sensor.sensor_id;
            
            const baseRadius = 6;
            const radius = isSelected ? baseRadius + 4 : isHovered ? baseRadius + 2 : baseRadius;
            
            this.ctx.beginPath();
            this.ctx.arc(x, y, radius + 4, 0, Math.PI * 2);
            this.ctx.fillStyle = this.adjustColor(color, -50) + '40';
            this.ctx.fill();
            
            this.ctx.beginPath();
            this.ctx.arc(x, y, radius, 0, Math.PI * 2);
            
            const gradient = this.ctx.createRadialGradient(x - 2, y - 2, 0, x, y, radius);
            gradient.addColorStop(0, this.adjustColor(color, 30));
            gradient.addColorStop(1, color);
            this.ctx.fillStyle = gradient;
            this.ctx.fill();
            
            this.ctx.strokeStyle = isSelected ? '#ffffff' : 'rgba(255, 255, 255, 0.5)';
            this.ctx.lineWidth = isSelected ? 2 : 1;
            this.ctx.stroke();
            
            if (isHovered || isSelected) {
                this.drawSensorTooltip(sensor, x, y);
            }
        }
    }

    drawSensorTooltip(sensor, x, y) {
        const sensorTypeNames = {
            'voltage': '电压',
            'current_density': '电流密度',
            'hydrogen_flow': '氢气流量',
            'oxygen_flow': '氧气流量',
            'water_temp': '水温',
            'membrane_conductivity': '膜电导率',
            'hydrogen_purity': '氢气纯度',
            'cell_voltage': '小室电压'
        };
        
        const locationNames = {
            'anode': '阳极侧',
            'cathode': '阴极侧',
            'membrane': '膜电极'
        };
        
        const typeName = sensorTypeNames[sensor.sensor_type] || sensor.sensor_type;
        const locName = locationNames[sensor.location] || sensor.location;
        
        const unit = this.getUnitForSensorType(sensor.sensor_type);
        
        const lines = [
            `${typeName} #${sensor.sensor_id}`,
            `位置: ${locName}`,
            `当前值: ${sensor.current_value.toFixed(4)} ${unit}`,
            `额定值: ${sensor.rated_value.toFixed(4)} ${unit}`,
            `偏差: ${sensor.deviation_percent >= 0 ? '+' : ''}${sensor.deviation_percent.toFixed(2)}%`
        ];
        
        this.ctx.font = '12px "Segoe UI", sans-serif';
        const padding = 10;
        const lineHeight = 18;
        const maxWidth = Math.max(...lines.map(l => this.ctx.measureText(l).width));
        const tooltipWidth = maxWidth + padding * 2;
        const tooltipHeight = lines.length * lineHeight + padding * 2;
        
        let tooltipX = x + 15;
        let tooltipY = y - tooltipHeight / 2;
        
        if (tooltipX + tooltipWidth > this.displayWidth) {
            tooltipX = x - tooltipWidth - 15;
        }
        if (tooltipY < 10) {
            tooltipY = 10;
        }
        if (tooltipY + tooltipHeight > this.displayHeight - 10) {
            tooltipY = this.displayHeight - tooltipHeight - 10;
        }
        
        this.ctx.fillStyle = 'rgba(20, 20, 40, 0.95)';
        this.ctx.beginPath();
        this.roundRect(tooltipX, tooltipY, tooltipWidth, tooltipHeight, 8);
        this.ctx.fill();
        
        this.ctx.strokeStyle = this.getDeviationColor(sensor.deviation_percent);
        this.ctx.lineWidth = 2;
        this.ctx.stroke();
        
        this.ctx.textAlign = 'left';
        this.ctx.textBaseline = 'top';
        
        lines.forEach((line, i) => {
            if (i === 0) {
                this.ctx.fillStyle = '#00d4ff';
                this.ctx.font = 'bold 12px "Segoe UI", sans-serif';
            } else {
                this.ctx.fillStyle = '#e0e0e0';
                this.ctx.font = '12px "Segoe UI", sans-serif';
            }
            this.ctx.fillText(line, tooltipX + padding, tooltipY + padding + i * lineHeight);
        });
    }

    getUnitForSensorType(type) {
        const units = {
            'voltage': 'V',
            'current_density': 'A/cm²',
            'hydrogen_flow': 'm³/h',
            'oxygen_flow': 'm³/h',
            'water_temp': '°C',
            'membrane_conductivity': 'S/cm',
            'hydrogen_purity': '%',
            'cell_voltage': 'V'
        };
        return units[type] || '';
    }

    adjustColor(color, amount) {
        const clamp = (num) => Math.min(255, Math.max(0, num));
        
        if (color.startsWith('#')) {
            const num = parseInt(color.slice(1), 16);
            const r = clamp((num >> 16) + amount);
            const g = clamp(((num >> 8) & 0x00FF) + amount);
            const b = clamp((num & 0x0000FF) + amount);
            return `#${(r << 16 | g << 8 | b).toString(16).padStart(6, '0')}`;
        }
        return color;
    }

    roundRect(x, y, width, height, radius) {
        this.ctx.moveTo(x + radius, y);
        this.ctx.lineTo(x + width - radius, y);
        this.ctx.quadraticCurveTo(x + width, y, x + width, y + radius);
        this.ctx.lineTo(x + width, y + height - radius);
        this.ctx.quadraticCurveTo(x + width, y + height, x + width - radius, y + height);
        this.ctx.lineTo(x + radius, y + height);
        this.ctx.quadraticCurveTo(x, y + height, x, y + height - radius);
        this.ctx.lineTo(x, y + radius);
        this.ctx.quadraticCurveTo(x, y, x + radius, y);
        this.ctx.closePath();
    }
}
