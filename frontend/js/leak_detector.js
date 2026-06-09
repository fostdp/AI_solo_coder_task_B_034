class LeakDetectorComponent {
    constructor(containerId) {
        this.container = document.getElementById(containerId);
        this.data = null;
        this.chart = null;
        this.render();
    }

    render() {
        if (!this.container) return;
        
        this.container.innerHTML = `
            <div class="leak-detector-container">
                <h3 class="component-title">氢气泄漏检测</h3>
                <div class="leak-status">
                    <div class="status-indicator" id="leak-status-indicator">
                        <span class="status-dot"></span>
                        <span class="status-text" id="leak-status-text">无泄漏</span>
                    </div>
                    <div class="detection-time" id="leak-detection-time"></div>
                </div>
                
                <div class="leak-metrics">
                    <div class="metric-card">
                        <span class="metric-label">SNR</span>
                        <span class="metric-value" id="leak-snr">--</span>
                        <span class="metric-unit">dB</span>
                    </div>
                    <div class="metric-card">
                        <span class="metric-label">泄漏浓度</span>
                        <span class="metric-value" id="leak-concentration">--</span>
                        <span class="metric-unit">%</span>
                    </div>
                    <div class="metric-card">
                        <span class="metric-label">泄漏速率</span>
                        <span class="metric-value" id="leak-rate">--</span>
                        <span class="metric-unit">m³/h</span>
                    </div>
                    <div class="metric-card">
                        <span class="metric-label">自适应滤波</span>
                        <span class="metric-value" id="leak-filter-enabled">--</span>
                        <span class="metric-unit">状态</span>
                    </div>
                </div>
                
                <div class="leak-location">
                    <h4>泄漏定位</h4>
                    <div class="location-info">
                        <div class="location-coordinates">
                            <span class="coord-label">X:</span>
                            <span class="coord-value" id="leak-x">--</span>
                            <span class="coord-label">Y:</span>
                            <span class="coord-value" id="leak-y">--</span>
                            <span class="coord-label">Z:</span>
                            <span class="coord-value" id="leak-z">--</span>
                        </div>
                        <div class="location-accuracy">
                            <span class="accuracy-label">定位精度:</span>
                            <span class="accuracy-value" id="leak-accuracy">--</span>
                            <span class="accuracy-unit">m</span>
                        </div>
                    </div>
                    <canvas id="leak-location-canvas"></canvas>
                </div>
                
                <div class="leak-spectrum">
                    <h4>声发射频谱分析</h4>
                    <canvas id="leak-spectrum-canvas"></canvas>
                </div>
                
                <div class="leak-severity">
                    <h4>泄漏等级</h4>
                    <div id="leak-severity" class="severity-indicator severity-normal">无泄漏</div>
                </div>
                
                <div class="leak-sensors">
                    <h4>传感器状态</h4>
                    <div id="leak-sensors" class="sensor-grid"></div>
                </div>
                
                <div class="leak-alert" id="leak-alert" style="display: none;">
                    <div class="alert-icon">⚠️</div>
                    <div class="alert-content">
                        <div class="alert-title" id="leak-alert-title">检测到氢气泄漏</div>
                        <div class="alert-message" id="leak-alert-message">请立即采取措施</div>
                    </div>
                </div>
            </div>
        `;
        
        this.locationChart = new LeakLocationChart('leak-location-canvas');
        this.spectrumChart = new SpectrumChart('leak-spectrum-canvas');
    }

    setData(data) {
        this.data = data;
        this.updateUI();
    }

    updateUI() {
        if (!this.data) return;
        
        const hasLeak = this.data.is_leak_detected || false;
        
        const statusMap = {
            'none': { class: 'status-normal', text: '无泄漏' },
            'minor': { class: 'status-mild', text: '轻微泄漏' },
            'moderate': { class: 'status-moderate', text: '中度泄漏' },
            'severe': { class: 'status-severe', text: '严重泄漏' }
        };
        
        const status = statusMap[this.data.leak_severity] || (hasLeak ? statusMap['minor'] : statusMap['none']);
        const indicator = document.getElementById('leak-status-indicator');
        indicator.className = `status-indicator ${status.class}`;
        document.getElementById('leak-status-text').textContent = status.text;
        
        if (this.data.timestamp) {
            const time = new Date(this.data.timestamp).toLocaleString('zh-CN');
            document.getElementById('leak-detection-time').textContent = `检测时间: ${time}`;
        }
        
        document.getElementById('leak-snr').textContent = this.data.snr?.toFixed(1) || '--';
        document.getElementById('leak-concentration').textContent = this.data.concentration?.toFixed(3) || '--';
        document.getElementById('leak-rate').textContent = this.data.leak_rate?.toFixed(4) || '--';
        document.getElementById('leak-filter-enabled').textContent = this.data.adaptive_filter_enabled ? '启用' : '禁用';
        
        if (this.data.location) {
            document.getElementById('leak-x').textContent = this.data.location.x?.toFixed(2) || '--';
            document.getElementById('leak-y').textContent = this.data.location.y?.toFixed(2) || '--';
            document.getElementById('leak-z').textContent = this.data.location.z?.toFixed(2) || '--';
            document.getElementById('leak-accuracy').textContent = this.data.location_accuracy?.toFixed(2) || '--';
        }
        
        const severityMap = {
            'none': '无泄漏',
            'minor': '轻微',
            'moderate': '中度',
            'severe': '严重'
        };
        const severityEl = document.getElementById('leak-severity');
        const sevClass = this.data.leak_severity || (hasLeak ? 'minor' : 'normal');
        severityEl.className = `severity-indicator severity-${sevClass}`;
        severityEl.textContent = severityMap[this.data.leak_severity] || (hasLeak ? '检测到' : '无泄漏');
        
        if (this.data.sensors) {
            this.updateSensors(this.data.sensors);
        }
        
        if (hasLeak) {
            const alertEl = document.getElementById('leak-alert');
            alertEl.style.display = 'flex';
            document.getElementById('leak-alert-title').textContent = 
                this.data.alert_message || '检测到氢气泄漏';
            document.getElementById('leak-alert-message').textContent = 
                this.data.recommended_action || '请立即采取措施';
        } else {
            document.getElementById('leak-alert').style.display = 'none';
        }
        
        if (this.locationChart && this.data.sensors) {
            this.locationChart.setData({
                sensors: this.data.sensors,
                leakLocation: this.data.location
            });
        }
        
        if (this.spectrumChart && this.data.spectrum_data) {
            this.spectrumChart.setData(this.data.spectrum_data);
        }
    }

    updateSensors(sensors) {
        const sensorsEl = document.getElementById('leak-sensors');
        sensorsEl.innerHTML = sensors.map(sensor => `
            <div class="sensor-item ${sensor.status}">
                <span class="sensor-id">#${sensor.id}</span>
                <span class="sensor-amplitude">${sensor.amplitude?.toFixed(2) || '--'} dB</span>
            </div>
        `).join('');
    }
}

class LeakLocationChart {
    constructor(canvasId) {
        this.canvas = document.getElementById(canvasId);
        this.ctx = this.canvas.getContext('2d');
        this.data = { sensors: [], leakLocation: null };
        this.setupCanvas();
    }

    setupCanvas() {
        const rect = this.canvas.getBoundingClientRect();
        const dpr = window.devicePixelRatio || 1;
        
        this.canvas.width = rect.width * dpr;
        this.canvas.height = rect.height * dpr;
        this.ctx.scale(dpr, dpr);
        
        this.displayWidth = rect.width;
        this.displayHeight = rect.height;
        this.margin = { top: 10, right: 10, bottom: 30, left: 40 };
        this.chartWidth = this.displayWidth - this.margin.left - this.margin.right;
        this.chartHeight = this.displayHeight - this.margin.top - this.margin.bottom;
    }

    setData(data) {
        this.data = data || { sensors: [], leakLocation: null };
        this.render();
    }

    render() {
        this.ctx.clearRect(0, 0, this.displayWidth, this.displayHeight);
        
        this.ctx.fillStyle = 'rgba(0, 0, 0, 0.3)';
        this.ctx.fillRect(0, 0, this.displayWidth, this.displayHeight);
        
        this.ctx.strokeStyle = 'rgba(255, 255, 255, 0.1)';
        this.ctx.lineWidth = 1;
        for (let i = 0; i <= 5; i++) {
            const x = this.margin.left + (i / 5) * this.chartWidth;
            this.ctx.beginPath();
            this.ctx.moveTo(x, this.margin.top);
            this.ctx.lineTo(x, this.displayHeight - this.margin.bottom);
            this.ctx.stroke();
            
            const y = this.margin.top + (i / 5) * this.chartHeight;
            this.ctx.beginPath();
            this.ctx.moveTo(this.margin.left, y);
            this.ctx.lineTo(this.displayWidth - this.margin.right, y);
            this.ctx.stroke();
        }
        
        this.ctx.strokeStyle = '#888';
        this.ctx.lineWidth = 2;
        this.ctx.strokeRect(
            this.margin.left, this.margin.top, 
            this.chartWidth, this.chartHeight
        );
        
        this.ctx.fillStyle = '#ccc';
        this.ctx.font = '10px "Segoe UI", sans-serif';
        this.ctx.textAlign = 'center';
        for (let i = 0; i <= 5; i++) {
            const x = this.margin.left + (i / 5) * this.chartWidth;
            this.ctx.fillText(`${(i / 5 * 10).toFixed(0)}`, x, this.displayHeight - 10);
        }
        
        this.ctx.textAlign = 'right';
        for (let i = 0; i <= 5; i++) {
            const y = this.displayHeight - this.margin.bottom - (i / 5) * this.chartHeight;
            this.ctx.fillText(`${(i / 5 * 10).toFixed(0)}`, this.margin.left - 5, y + 4);
        }
        
        if (this.data.sensors) {
            this.data.sensors.forEach(sensor => {
                const x = this.margin.left + (sensor.x / 10) * this.chartWidth;
                const y = this.displayHeight - this.margin.bottom - (sensor.y / 10) * this.chartHeight;
                
                this.ctx.fillStyle = sensor.status === 'active' ? '#4CAF50' : '#666';
                this.ctx.beginPath();
                this.ctx.arc(x, y, 6, 0, Math.PI * 2);
                this.ctx.fill();
                
                this.ctx.fillStyle = '#fff';
                this.ctx.font = '9px "Segoe UI", sans-serif';
                this.ctx.textAlign = 'center';
                this.ctx.fillText(sensor.id, x, y + 3);
            });
        }
        
        if (this.data.leakLocation) {
            const x = this.margin.left + (this.data.leakLocation.x / 10) * this.chartWidth;
            const y = this.displayHeight - this.margin.bottom - (this.data.leakLocation.y / 10) * this.chartHeight;
            
            this.ctx.strokeStyle = '#F44336';
            this.ctx.lineWidth = 2;
            this.ctx.beginPath();
            this.ctx.arc(x, y, 15, 0, Math.PI * 2);
            this.ctx.stroke();
            
            this.ctx.fillStyle = 'rgba(244, 67, 54, 0.3)';
            this.ctx.beginPath();
            this.ctx.arc(x, y, 12, 0, Math.PI * 2);
            this.ctx.fill();
            
            this.ctx.fillStyle = '#F44336';
            this.ctx.beginPath();
            this.ctx.arc(x, y, 5, 0, Math.PI * 2);
            this.ctx.fill();
            
            this.ctx.fillStyle = '#F44336';
            this.ctx.font = '11px "Segoe UI", sans-serif';
            this.ctx.textAlign = 'center';
            this.ctx.fillText('泄漏点', x, y - 20);
        }
    }
}

class SpectrumChart {
    constructor(canvasId) {
        this.canvas = document.getElementById(canvasId);
        this.ctx = this.canvas.getContext('2d');
        this.data = [];
        this.setupCanvas();
    }

    setupCanvas() {
        const rect = this.canvas.getBoundingClientRect();
        const dpr = window.devicePixelRatio || 1;
        
        this.canvas.width = rect.width * dpr;
        this.canvas.height = rect.height * dpr;
        this.ctx.scale(dpr, dpr);
        
        this.displayWidth = rect.width;
        this.displayHeight = rect.height;
        this.margin = { top: 20, right: 20, bottom: 40, left: 50 };
        this.chartWidth = this.displayWidth - this.margin.left - this.margin.right;
        this.chartHeight = this.displayHeight - this.margin.top - this.margin.bottom;
    }

    setData(data) {
        this.data = data || [];
        this.render();
    }

    render() {
        this.ctx.clearRect(0, 0, this.displayWidth, this.displayHeight);
        
        this.ctx.fillStyle = 'rgba(0, 0, 0, 0.3)';
        this.ctx.fillRect(0, 0, this.displayWidth, this.displayHeight);
        
        if (this.data.length === 0) {
            this.ctx.fillStyle = '#666';
            this.ctx.font = '14px "Segoe UI", sans-serif';
            this.ctx.textAlign = 'center';
            this.ctx.fillText('暂无频谱数据', this.displayWidth / 2, this.displayHeight / 2);
            return;
        }
        
        const frequencies = this.data.map(d => d.frequency);
        const amplitudes = this.data.map(d => d.amplitude);
        
        const xMin = Math.min(...frequencies);
        const xMax = Math.max(...frequencies);
        const yMin = Math.min(...amplitudes) * 0.9;
        const yMax = Math.max(...amplitudes) * 1.1;
        
        this.ctx.strokeStyle = 'rgba(255, 255, 255, 0.1)';
        this.ctx.lineWidth = 1;
        for (let i = 0; i <= 5; i++) {
            const x = this.margin.left + (i / 5) * this.chartWidth;
            this.ctx.beginPath();
            this.ctx.moveTo(x, this.margin.top);
            this.ctx.lineTo(x, this.displayHeight - this.margin.bottom);
            this.ctx.stroke();
            
            const y = this.margin.top + (i / 5) * this.chartHeight;
            this.ctx.beginPath();
            this.ctx.moveTo(this.margin.left, y);
            this.ctx.lineTo(this.displayWidth - this.margin.right, y);
            this.ctx.stroke();
        }
        
        this.ctx.strokeStyle = '#888';
        this.ctx.lineWidth = 2;
        this.ctx.beginPath();
        this.ctx.moveTo(this.margin.left, this.displayHeight - this.margin.bottom);
        this.ctx.lineTo(this.displayWidth - this.margin.right, this.displayHeight - this.margin.bottom);
        this.ctx.stroke();
        
        this.ctx.beginPath();
        this.ctx.moveTo(this.margin.left, this.displayHeight - this.margin.bottom);
        this.ctx.lineTo(this.margin.left, this.margin.top);
        this.ctx.stroke();
        
        this.ctx.fillStyle = '#ccc';
        this.ctx.font = '11px "Segoe UI", sans-serif';
        this.ctx.textAlign = 'center';
        this.ctx.fillText('频率 (kHz)', this.displayWidth / 2, this.displayHeight - 10);
        
        this.ctx.save();
        this.ctx.translate(12, this.displayHeight / 2);
        this.ctx.rotate(-Math.PI / 2);
        this.ctx.fillText('幅值 (dB)', 0, 0);
        this.ctx.restore();
        
        const barWidth = this.chartWidth / this.data.length * 0.8;
        this.data.forEach((d, i) => {
            const x = this.margin.left + (i / this.data.length) * this.chartWidth + barWidth * 0.1;
            const barHeight = ((d.amplitude - yMin) / (yMax - yMin)) * this.chartHeight;
            const y = this.displayHeight - this.margin.bottom - barHeight;
            
            const gradient = this.ctx.createLinearGradient(x, y, x, this.displayHeight - this.margin.bottom);
            gradient.addColorStop(0, '#00d4ff');
            gradient.addColorStop(1, 'rgba(0, 212, 255, 0.2)');
            
            this.ctx.fillStyle = gradient;
            this.ctx.fillRect(x, y, barWidth, barHeight);
        });
    }
}

const leakDetectorStyle = document.createElement('style');
leakDetectorStyle.textContent = `
    .leak-detector-container {
        padding: 1rem;
        background: rgba(0, 0, 0, 0.3);
        border-radius: 8px;
    }
    .component-title {
        color: #00d4ff;
        margin: 0 0 1rem 0;
        font-size: 1.2rem;
    }
    .leak-status {
        display: flex;
        justify-content: space-between;
        align-items: center;
        margin-bottom: 1rem;
        padding: 0.75rem;
        background: rgba(0, 0, 0, 0.3);
        border-radius: 6px;
    }
    .status-indicator {
        display: flex;
        align-items: center;
        gap: 0.5rem;
    }
    .status-dot {
        width: 12px;
        height: 12px;
        border-radius: 50%;
        background: #666;
    }
    .status-normal .status-dot { background: #4CAF50; }
    .status-mild .status-dot { background: #FFC107; }
    .status-moderate .status-dot { background: #FF9800; }
    .status-severe .status-dot { background: #F44336; }
    .leak-metrics {
        display: grid;
        grid-template-columns: repeat(4, 1fr);
        gap: 0.75rem;
        margin-bottom: 1rem;
    }
    .metric-card {
        background: rgba(0, 0, 0, 0.3);
        padding: 0.75rem;
        border-radius: 6px;
        text-align: center;
    }
    .metric-label {
        display: block;
        color: #888;
        font-size: 0.8rem;
        margin-bottom: 0.25rem;
    }
    .metric-value {
        display: block;
        color: #00d4ff;
        font-size: 1.2rem;
        font-weight: bold;
    }
    .metric-unit {
        display: block;
        color: #666;
        font-size: 0.75rem;
    }
    .location-info {
        display: flex;
        justify-content: space-between;
        margin-bottom: 0.5rem;
    }
    .coord-label, .accuracy-label {
        color: #888;
        margin-right: 0.25rem;
    }
    .coord-value, .accuracy-value {
        color: #00d4ff;
        font-weight: bold;
        margin-right: 0.75rem;
    }
    .accuracy-unit {
        color: #666;
    }
    #leak-location-canvas, #leak-spectrum-canvas {
        width: 100%;
        height: 200px;
        border-radius: 6px;
        margin-top: 0.5rem;
    }
    .severity-indicator {
        display: inline-block;
        padding: 0.5rem 1rem;
        border-radius: 4px;
        font-weight: bold;
    }
    .severity-normal { background: rgba(76, 175, 80, 0.2); color: #4CAF50; }
    .severity-mild { background: rgba(255, 193, 7, 0.2); color: #FFC107; }
    .severity-moderate { background: rgba(255, 152, 0, 0.2); color: #FF9800; }
    .severity-severe { background: rgba(244, 67, 54, 0.2); color: #F44336; }
    .sensor-grid {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(80px, 1fr));
        gap: 0.5rem;
    }
    .sensor-item {
        background: rgba(0, 0, 0, 0.3);
        padding: 0.5rem;
        border-radius: 4px;
        text-align: center;
    }
    .sensor-item.active {
        border: 1px solid #4CAF50;
    }
    .sensor-item.alert {
        border: 1px solid #F44336;
    }
    .sensor-id {
        display: block;
        color: #888;
        font-size: 0.75rem;
    }
    .sensor-amplitude {
        display: block;
        color: #00d4ff;
        font-weight: bold;
        font-size: 0.9rem;
    }
    .leak-alert {
        display: flex;
        align-items: center;
        gap: 1rem;
        padding: 1rem;
        background: rgba(244, 67, 54, 0.1);
        border: 1px solid #F44336;
        border-radius: 6px;
        margin-top: 1rem;
        animation: pulse 2s infinite;
    }
    .alert-icon {
        font-size: 2rem;
    }
    .alert-title {
        color: #F44336;
        font-weight: bold;
        margin-bottom: 0.25rem;
    }
    .alert-message {
        color: #ccc;
        font-size: 0.9rem;
    }
    @keyframes pulse {
        0%, 100% { opacity: 1; }
        50% { opacity: 0.7; }
    }
    .leak-location, .leak-spectrum, .leak-severity, .leak-sensors {
        margin-bottom: 1rem;
    }
    .leak-location h4, .leak-spectrum h4, .leak-severity h4, .leak-sensors h4 {
        color: #ccc;
        margin-bottom: 0.5rem;
    }
`;
document.head.appendChild(leakDetectorStyle);
