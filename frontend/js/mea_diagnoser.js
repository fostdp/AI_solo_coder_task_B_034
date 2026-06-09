class MEADiagnoserComponent {
    constructor(containerId) {
        this.container = document.getElementById(containerId);
        this.data = null;
        this.chart = null;
        this.render();
    }

    render() {
        if (!this.container) return;
        
        this.container.innerHTML = `
            <div class="diagnoser-container">
                <h3 class="component-title">膜电极健康诊断</h3>
                <div class="diagnoser-status">
                    <div class="status-indicator" id="mea-status-indicator">
                        <span class="status-dot"></span>
                        <span class="status-text" id="mea-status-text">等待诊断...</span>
                    </div>
                    <div class="diagnostic-time" id="mea-diagnostic-time"></div>
                </div>
                
                <div class="diagnoser-metrics">
                    <div class="metric-card">
                        <span class="metric-label">膜电阻</span>
                        <span class="metric-value" id="mea-membrane-resistance">--</span>
                        <span class="metric-unit">Ω·cm²</span>
                    </div>
                    <div class="metric-card">
                        <span class="metric-label">电荷转移电阻</span>
                        <span class="metric-value" id="mea-charge-resistance">--</span>
                        <span class="metric-unit">Ω·cm²</span>
                    </div>
                    <div class="metric-card">
                        <span class="metric-label">双电层电容</span>
                        <span class="metric-value" id="mea-double-layer-cap">--</span>
                        <span class="metric-unit">F/cm²</span>
                    </div>
                    <div class="metric-card">
                        <span class="metric-label">电导率趋势</span>
                        <span class="metric-value" id="mea-conductivity-trend">--</span>
                        <span class="metric-unit">%/1000h</span>
                    </div>
                </div>
                
                <div class="diagnoser-degradation">
                    <h4>退化模式</h4>
                    <div id="mea-degradation-mode" class="degradation-mode">
                        <span class="degradation-label">未知</span>
                    </div>
                    <div class="confidence-bar">
                        <span class="confidence-label">置信度</span>
                        <div class="confidence-track">
                            <div class="confidence-fill" id="mea-confidence-fill"></div>
                        </div>
                        <span class="confidence-value" id="mea-confidence-value">--%</span>
                    </div>
                </div>
                
                <div class="diagnoser-severity">
                    <h4>严重程度</h4>
                    <div id="mea-severity" class="severity-indicator severity-normal">正常</div>
                </div>
                
                <div class="diagnoser-recommendations">
                    <h4>建议措施</h4>
                    <ul id="mea-recommendations" class="recommendations-list">
                        <li>等待诊断结果...</li>
                    </ul>
                </div>
                
                <div class="diagnoser-icons">
                    <h4>诊断图标</h4>
                    <div id="mea-icons" class="diagnostic-icons"></div>
                </div>
                
                <div class="diagnoser-chart">
                    <h4>奈奎斯特图</h4>
                    <canvas id="mea-nyquist-canvas"></canvas>
                </div>
            </div>
        `;
        
        this.chart = new NyquistChart('mea-nyquist-canvas');
    }

    setData(data) {
        this.data = data;
        this.updateUI();
    }

    updateUI() {
        if (!this.data) return;
        
        const statusMap = {
            'normal': { class: 'status-normal', text: '正常' },
            'mild': { class: 'status-mild', text: '轻度退化' },
            'moderate': { class: 'status-moderate', text: '中度退化' },
            'severe': { class: 'status-severe', text: '严重退化' }
        };
        
        const status = statusMap[this.data.health_status] || statusMap['normal'];
        const indicator = document.getElementById('mea-status-indicator');
        indicator.className = `status-indicator ${status.class}`;
        document.getElementById('mea-status-text').textContent = status.text;
        
        if (this.data.timestamp) {
            const time = new Date(this.data.timestamp).toLocaleString('zh-CN');
            document.getElementById('mea-diagnostic-time').textContent = `诊断时间: ${time}`;
        }
        
        if (this.data.equivalent_circuit) {
            document.getElementById('mea-membrane-resistance').textContent = 
                this.data.equivalent_circuit.r_membrane?.toFixed(4) || '--';
            document.getElementById('mea-charge-resistance').textContent = 
                this.data.equivalent_circuit.r_charge_transfer?.toFixed(4) || '--';
            document.getElementById('mea-double-layer-cap').textContent = 
                this.data.equivalent_circuit.c_double_layer?.toFixed(6) || '--';
        }
        
        if (this.data.conductivity_trend !== undefined) {
            document.getElementById('mea-conductivity-trend').textContent = 
                this.data.conductivity_trend.toFixed(2);
        }
        
        const modeEl = document.getElementById('mea-degradation-mode');
        const modeNames = {
            'membrane_degradation': '膜材料降解',
            'catalyst_degradation': '催化剂老化',
            'ionomer_degradation': '离聚物退化',
            'flooding': '水淹现象',
            'drying': '膜干现象',
            'normal': '正常状态'
        };
        modeEl.innerHTML = `<span class="degradation-label ${this.data.degradation_mode}">
            ${modeNames[this.data.degradation_mode] || this.data.degradation_mode}
        </span>`;
        
        const confidence = this.data.confidence || 0;
        document.getElementById('mea-confidence-fill').style.width = `${confidence}%`;
        document.getElementById('mea-confidence-value').textContent = `${confidence.toFixed(1)}%`;
        
        const severityMap = {
            'normal': '正常',
            'mild': '轻度',
            'moderate': '中度',
            'severe': '严重'
        };
        const severityEl = document.getElementById('mea-severity');
        severityEl.className = `severity-indicator severity-${this.data.severity || 'normal'}`;
        severityEl.textContent = severityMap[this.data.severity] || '未知';
        
        const recList = document.getElementById('mea-recommendations');
        if (this.data.recommendations && this.data.recommendations.length > 0) {
            recList.innerHTML = this.data.recommendations.map(rec => 
                `<li>${rec}</li>`
            ).join('');
        } else {
            recList.innerHTML = '<li>无需特殊处理，继续监控</li>';
        }
        
        const iconsEl = document.getElementById('mea-icons');
        if (this.data.icons && this.data.icons.length > 0) {
            iconsEl.innerHTML = this.data.icons.map(icon => 
                `<span class="diagnostic-icon ${icon.type}">${icon.symbol}</span>`
            ).join('');
        }
        
        if (this.data.eis_data && this.chart) {
            this.chart.setData(this.data.eis_data);
        }
    }

    updateTemperature(temp) {
        const tempEl = document.getElementById('mea-temperature');
        if (tempEl) {
            tempEl.textContent = `${temp.toFixed(1)}°C`;
        }
    }
}

class NyquistChart {
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
            this.ctx.fillText('暂无EIS数据', this.displayWidth / 2, this.displayHeight / 2);
            return;
        }
        
        const zReal = this.data.map(d => d.z_real);
        const zImag = this.data.map(d => Math.abs(d.z_imag));
        
        const xMin = Math.min(...zReal) * 0.9;
        const xMax = Math.max(...zReal) * 1.1;
        const yMin = 0;
        const yMax = Math.max(...zImag) * 1.2;
        
        this.drawGrid(xMin, xMax, yMin, yMax);
        this.drawAxes(xMin, xMax, yMin, yMax);
        this.drawPoints(zReal, zImag, xMin, xMax, yMin, yMax);
    }

    drawGrid(xMin, xMax, yMin, yMax) {
        this.ctx.strokeStyle = 'rgba(255, 255, 255, 0.1)';
        this.ctx.lineWidth = 1;
        
        const xSteps = 5;
        for (let i = 0; i <= xSteps; i++) {
            const x = this.margin.left + (i / xSteps) * this.chartWidth;
            this.ctx.beginPath();
            this.ctx.moveTo(x, this.margin.top);
            this.ctx.lineTo(x, this.displayHeight - this.margin.bottom);
            this.ctx.stroke();
        }
        
        const ySteps = 5;
        for (let i = 0; i <= ySteps; i++) {
            const y = this.margin.top + (i / ySteps) * this.chartHeight;
            this.ctx.beginPath();
            this.ctx.moveTo(this.margin.left, y);
            this.ctx.lineTo(this.displayWidth - this.margin.right, y);
            this.ctx.stroke();
        }
    }

    drawAxes(xMin, xMax, yMin, yMax) {
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
        this.ctx.font = '12px "Segoe UI", sans-serif';
        this.ctx.textAlign = 'center';
        this.ctx.fillText("Z' (Ω·cm²)", this.displayWidth / 2, this.displayHeight - 10);
        
        this.ctx.save();
        this.ctx.translate(15, this.displayHeight / 2);
        this.ctx.rotate(-Math.PI / 2);
        this.ctx.fillText("-Z'' (Ω·cm²)", 0, 0);
        this.ctx.restore();
    }

    drawPoints(zReal, zImag, xMin, xMax, yMin, yMax) {
        this.ctx.strokeStyle = '#00d4ff';
        this.ctx.lineWidth = 2;
        this.ctx.beginPath();
        
        for (let i = 0; i < zReal.length; i++) {
            const x = this.margin.left + ((zReal[i] - xMin) / (xMax - xMin)) * this.chartWidth;
            const y = this.displayHeight - this.margin.bottom - ((zImag[i] - yMin) / (yMax - yMin)) * this.chartHeight;
            
            if (i === 0) {
                this.ctx.moveTo(x, y);
            } else {
                this.ctx.lineTo(x, y);
            }
        }
        this.ctx.stroke();
        
        for (let i = 0; i < zReal.length; i++) {
            const x = this.margin.left + ((zReal[i] - xMin) / (xMax - xMin)) * this.chartWidth;
            const y = this.displayHeight - this.margin.bottom - ((zImag[i] - yMin) / (yMax - yMin)) * this.chartHeight;
            
            this.ctx.fillStyle = '#00d4ff';
            this.ctx.beginPath();
            this.ctx.arc(x, y, 4, 0, Math.PI * 2);
            this.ctx.fill();
        }
    }
}

const meaDiagnoserStyle = document.createElement('style');
meaDiagnoserStyle.textContent = `
    .diagnoser-container {
        padding: 1rem;
        background: rgba(0, 0, 0, 0.3);
        border-radius: 8px;
    }
    .component-title {
        color: #00d4ff;
        margin: 0 0 1rem 0;
        font-size: 1.2rem;
    }
    .diagnoser-status {
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
    .diagnostic-time {
        color: #888;
        font-size: 0.9rem;
    }
    .diagnoser-metrics {
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
    .confidence-bar {
        display: flex;
        align-items: center;
        gap: 0.75rem;
        margin-top: 0.5rem;
    }
    .confidence-label {
        color: #888;
        font-size: 0.9rem;
        min-width: 60px;
    }
    .confidence-track {
        flex: 1;
        height: 8px;
        background: rgba(255, 255, 255, 0.1);
        border-radius: 4px;
        overflow: hidden;
    }
    .confidence-fill {
        height: 100%;
        background: linear-gradient(90deg, #4CAF50, #00d4ff);
        border-radius: 4px;
        transition: width 0.5s ease;
    }
    .confidence-value {
        color: #00d4ff;
        font-weight: bold;
        min-width: 50px;
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
    .recommendations-list {
        margin: 0;
        padding-left: 1.25rem;
        color: #ccc;
    }
    .recommendations-list li {
        margin-bottom: 0.5rem;
    }
    .diagnostic-icons {
        display: flex;
        gap: 0.5rem;
        flex-wrap: wrap;
    }
    .diagnostic-icon {
        width: 36px;
        height: 36px;
        display: flex;
        align-items: center;
        justify-content: center;
        border-radius: 6px;
        background: rgba(0, 212, 255, 0.1);
        color: #00d4ff;
        font-size: 1.2rem;
    }
    .diagnoser-chart {
        margin-top: 1rem;
    }
    .diagnoser-chart h4 {
        color: #ccc;
        margin-bottom: 0.5rem;
    }
    #mea-nyquist-canvas {
        width: 100%;
        height: 250px;
        border-radius: 6px;
    }
    .degradation-label {
        display: inline-block;
        padding: 0.25rem 0.75rem;
        border-radius: 4px;
        background: rgba(0, 212, 255, 0.2);
        color: #00d4ff;
    }
    .degradation-label.normal { background: rgba(76, 175, 80, 0.2); color: #4CAF50; }
    .degradation-label.membrane_degradation { background: rgba(244, 67, 54, 0.2); color: #F44336; }
    .degradation-label.catalyst_degradation { background: rgba(255, 152, 0, 0.2); color: #FF9800; }
`;
document.head.appendChild(meaDiagnoserStyle);
