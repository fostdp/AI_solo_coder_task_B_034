class RenewableIntegratorComponent {
    constructor(containerId) {
        this.container = document.getElementById(containerId);
        this.data = null;
        this.powerChart = null;
        this.controlChart = null;
        this.render();
    }

    render() {
        if (!this.container) return;
        
        this.container.innerHTML = `
            <div class="renewable-container">
                <h3 class="component-title">可再生能源耦合控制</h3>
                
                <div class="renewable-status">
                    <div class="status-indicator status-normal">
                        <span class="status-dot"></span>
                        <span class="status-text" id="renewable-status-text">系统正常</span>
                    </div>
                    <div class="control-mode" id="renewable-control-mode">MPC控制</div>
                </div>
                
                <div class="renewable-metrics">
                    <div class="metric-card">
                        <span class="metric-label">光伏出力</span>
                        <span class="metric-value" id="renewable-solar-power">--</span>
                        <span class="metric-unit">kW</span>
                    </div>
                    <div class="metric-card">
                        <span class="metric-label">风电出力</span>
                        <span class="metric-value" id="renewable-wind-power">--</span>
                        <span class="metric-unit">kW</span>
                    </div>
                    <div class="metric-card">
                        <span class="metric-label">电网功率</span>
                        <span class="metric-value" id="renewable-grid-power">--</span>
                        <span class="metric-unit">kW</span>
                    </div>
                    <div class="metric-card">
                        <span class="metric-label">电解槽功率</span>
                        <span class="metric-value" id="renewable-electrolyzer-power">--</span>
                        <span class="metric-unit">kW</span>
                    </div>
                </div>
                
                <div class="renewable-metrics secondary">
                    <div class="metric-card">
                        <span class="metric-label">可再生能源占比</span>
                        <span class="metric-value" id="renewable-percentage">--</span>
                        <span class="metric-unit">%</span>
                    </div>
                    <div class="metric-card">
                        <span class="metric-label">MPC求解时间</span>
                        <span class="metric-value" id="renewable-mpc-time">--</span>
                        <span class="metric-unit">ms</span>
                    </div>
                    <div class="metric-card">
                        <span class="metric-label">跟踪误差</span>
                        <span class="metric-value" id="renewable-tracking-error">--</span>
                        <span class="metric-unit">%</span>
                    </div>
                    <div class="metric-card">
                        <span class="metric-label">控制模式</span>
                        <span class="metric-value" id="renewable-solve-mode">--</span>
                        <span class="metric-unit">模式</span>
                    </div>
                </div>
                
                <div class="renewable-power-chart">
                    <h4>功率曲线</h4>
                    <canvas id="renewable-power-canvas"></canvas>
                </div>
                
                <div class="renewable-control-chart">
                    <h4>MPC控制信号</h4>
                    <canvas id="renewable-control-canvas"></canvas>
                </div>
                
                <div class="renewable-predictions">
                    <h4>功率预测 (未来24小时)</h4>
                    <div class="prediction-grid">
                        <div class="prediction-item">
                            <span class="prediction-label">光伏预测</span>
                            <span class="prediction-value" id="renewable-solar-prediction">-- kW</span>
                        </div>
                        <div class="prediction-item">
                            <span class="prediction-label">风电预测</span>
                            <span class="prediction-value" id="renewable-wind-prediction">-- kW</span>
                        </div>
                        <div class="prediction-item">
                            <span class="prediction-label">总可再生能源</span>
                            <span class="prediction-value" id="renewable-total-prediction">-- kW</span>
                        </div>
                        <div class="prediction-item">
                            <span class="prediction-label">预期电解槽功率</span>
                            <span class="prediction-value" id="renewable-electrolyzer-prediction">-- kW</span>
                        </div>
                    </div>
                </div>
                
                <div class="renewable-optimization">
                    <h4>优化结果</h4>
                    <div class="optimization-result">
                        <div class="opt-item">
                            <span class="opt-label">当前控制量</span>
                            <span class="opt-value" id="renewable-current-control">--</span>
                        </div>
                        <div class="opt-item">
                            <span class="opt-label">目标功率</span>
                            <span class="opt-value" id="renewable-target-power">-- kW</span>
                        </div>
                        <div class="opt-item">
                            <span class="opt-label">热启动</span>
                            <span class="opt-value" id="renewable-warm-start">--</span>
                        </div>
                        <div class="opt-item">
                            <span class="opt-label">近似求解</span>
                            <span class="opt-value" id="renewable-approx-solve">--</span>
                        </div>
                    </div>
                </div>
            </div>
        `;
        
        this.powerChart = new PowerChart('renewable-power-canvas');
        this.controlChart = new ControlChart('renewable-control-canvas');
    }

    setData(data) {
        this.data = data;
        this.updateUI();
    }

    updateUI() {
        if (!this.data) return;
        
        const statusText = document.getElementById('renewable-status-text');
        if (this.data.status === 'normal') {
            statusText.textContent = '系统正常';
        } else if (this.data.status === 'warning') {
            statusText.textContent = '功率波动';
        } else {
            statusText.textContent = '异常';
        }
        
        document.getElementById('renewable-solar-power').textContent = 
            this.data.solar_power?.toFixed(1) || '--';
        document.getElementById('renewable-wind-power').textContent = 
            this.data.wind_power?.toFixed(1) || '--';
        document.getElementById('renewable-grid-power').textContent = 
            this.data.grid_power?.toFixed(1) || '--';
        document.getElementById('renewable-electrolyzer-power').textContent = 
            this.data.electrolyzer_power?.toFixed(1) || '--';
        
        const totalPower = (this.data.solar_power || 0) + (this.data.wind_power || 0) + (this.data.grid_power || 0);
        const renewablePower = (this.data.solar_power || 0) + (this.data.wind_power || 0);
        const renewablePercentage = totalPower > 0 ? (renewablePower / totalPower * 100) : 0;
        document.getElementById('renewable-percentage').textContent = renewablePercentage.toFixed(1);
        
        document.getElementById('renewable-mpc-time').textContent = 
            this.data.mpc_compute_time_ms?.toFixed(0) || '--';
        document.getElementById('renewable-tracking-error').textContent = 
            this.data.tracking_error_percent?.toFixed(2) || '--';
        
        const solveMode = this.data.used_approximation ? '近似求解' : '精确求解';
        document.getElementById('renewable-solve-mode').textContent = solveMode;
        
        document.getElementById('renewable-current-control').textContent = 
            this.data.current_control?.toFixed(2) || '--';
        document.getElementById('renewable-target-power').textContent = 
            this.data.target_power?.toFixed(1) || '--';
        document.getElementById('renewable-warm-start').textContent = 
            this.data.used_warm_start ? '启用' : '未启用';
        document.getElementById('renewable-approx-solve').textContent = 
            this.data.used_approximation ? '启用' : '未启用';
        
        if (this.data.predictions) {
            const pred = this.data.predictions;
            document.getElementById('renewable-solar-prediction').textContent = 
                `${pred.solar?.toFixed(1) || '--'} kW`;
            document.getElementById('renewable-wind-prediction').textContent = 
                `${pred.wind?.toFixed(1) || '--'} kW`;
            document.getElementById('renewable-total-prediction').textContent = 
                `${((pred.solar || 0) + (pred.wind || 0)).toFixed(1)} kW`;
            document.getElementById('renewable-electrolyzer-prediction').textContent = 
                `${pred.electrolyzer?.toFixed(1) || '--'} kW`;
        }
        
        if (this.powerChart && this.data.power_history) {
            this.powerChart.setData(this.data.power_history);
        }
        
        if (this.controlChart && this.data.control_history) {
            this.controlChart.setData(this.data.control_history);
        }
    }
}

class PowerChart {
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
            this.ctx.fillText('暂无功率数据', this.displayWidth / 2, this.displayHeight / 2);
            return;
        }
        
        const allValues = [];
        this.data.forEach(d => {
            if (d.solar !== undefined) allValues.push(d.solar);
            if (d.wind !== undefined) allValues.push(d.wind);
            if (d.grid !== undefined) allValues.push(d.grid);
            if (d.electrolyzer !== undefined) allValues.push(d.electrolyzer);
        });
        
        const yMin = Math.min(...allValues, 0);
        const yMax = Math.max(...allValues) * 1.1;
        
        this.drawGrid(yMin, yMax);
        this.drawAxes(yMin, yMax);
        
        const colors = {
            solar: '#FFC107',
            wind: '#4CAF50',
            grid: '#2196F3',
            electrolyzer: '#00d4ff'
        };
        
        ['solar', 'wind', 'grid', 'electrolyzer'].forEach((type, idx) => {
            this.drawLine(type, colors[type], idx);
        });
        
        this.drawLegend(colors);
    }

    drawGrid(yMin, yMax) {
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
    }

    drawAxes(yMin, yMax) {
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
        this.ctx.fillText('时间', this.displayWidth / 2, this.displayHeight - 10);
        
        this.ctx.save();
        this.ctx.translate(12, this.displayHeight / 2);
        this.ctx.rotate(-Math.PI / 2);
        this.ctx.fillText('功率 (kW)', 0, 0);
        this.ctx.restore();
        
        this.ctx.textAlign = 'right';
        for (let i = 0; i <= 5; i++) {
            const y = this.displayHeight - this.margin.bottom - (i / 5) * this.chartHeight;
            const val = yMin + (i / 5) * (yMax - yMin);
            this.ctx.fillText(val.toFixed(0), this.margin.left - 5, y + 4);
        }
    }

    drawLine(type, color, idx) {
        const values = this.data.map(d => d[type]);
        const allValues = [];
        this.data.forEach(d => {
            if (d.solar !== undefined) allValues.push(d.solar);
            if (d.wind !== undefined) allValues.push(d.wind);
            if (d.grid !== undefined) allValues.push(d.grid);
            if (d.electrolyzer !== undefined) allValues.push(d.electrolyzer);
        });
        
        const yMin = Math.min(...allValues, 0);
        const yMax = Math.max(...allValues) * 1.1;
        
        this.ctx.strokeStyle = color;
        this.ctx.lineWidth = 2;
        this.ctx.beginPath();
        
        values.forEach((val, i) => {
            const x = this.margin.left + (i / (values.length - 1)) * this.chartWidth;
            const y = this.displayHeight - this.margin.bottom - ((val - yMin) / (yMax - yMin)) * this.chartHeight;
            
            if (i === 0) {
                this.ctx.moveTo(x, y);
            } else {
                this.ctx.lineTo(x, y);
            }
        });
        this.ctx.stroke();
    }

    drawLegend(colors) {
        const names = {
            solar: '光伏',
            wind: '风电',
            grid: '电网',
            electrolyzer: '电解槽'
        };
        
        this.ctx.font = '11px "Segoe UI", sans-serif';
        this.ctx.textAlign = 'left';
        
        let x = this.margin.left + 10;
        const y = this.margin.top + 15;
        
        Object.keys(colors).forEach((type, idx) => {
            this.ctx.fillStyle = colors[type];
            this.ctx.fillRect(x, y - 8, 12, 12);
            
            this.ctx.fillStyle = '#ccc';
            this.ctx.fillText(names[type], x + 18, y + 2);
            
            x += 80;
        });
    }
}

class ControlChart {
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
            this.ctx.fillText('暂无控制数据', this.displayWidth / 2, this.displayHeight / 2);
            return;
        }
        
        const controlValues = this.data.map(d => d.control_signal);
        const targetValues = this.data.map(d => d.target);
        
        const allValues = [...controlValues, ...targetValues];
        const yMin = Math.min(...allValues) * 0.9;
        const yMax = Math.max(...allValues) * 1.1;
        
        this.drawGrid(yMin, yMax);
        this.drawAxes(yMin, yMax);
        
        this.ctx.strokeStyle = '#FF9800';
        this.ctx.lineWidth = 2;
        this.ctx.setLineDash([5, 5]);
        this.drawLineData(targetValues, yMin, yMax, '#FF9800');
        this.ctx.setLineDash([]);
        
        this.ctx.strokeStyle = '#00d4ff';
        this.ctx.lineWidth = 2;
        this.drawLineData(controlValues, yMin, yMax, '#00d4ff');
        
        this.ctx.font = '11px "Segoe UI", sans-serif';
        this.ctx.fillStyle = '#FF9800';
        this.ctx.fillRect(this.margin.left + 10, this.margin.top + 5, 12, 12);
        this.ctx.fillStyle = '#ccc';
        this.ctx.fillText('目标功率', this.margin.left + 28, this.margin.top + 15);
        
        this.ctx.fillStyle = '#00d4ff';
        this.ctx.fillRect(this.margin.left + 110, this.margin.top + 5, 12, 12);
        this.ctx.fillStyle = '#ccc';
        this.ctx.fillText('控制信号', this.margin.left + 128, this.margin.top + 15);
    }

    drawGrid(yMin, yMax) {
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
    }

    drawAxes(yMin, yMax) {
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
        this.ctx.fillText('时间步', this.displayWidth / 2, this.displayHeight - 10);
        
        this.ctx.save();
        this.ctx.translate(12, this.displayHeight / 2);
        this.ctx.rotate(-Math.PI / 2);
        this.ctx.fillText('功率 (kW)', 0, 0);
        this.ctx.restore();
        
        this.ctx.textAlign = 'right';
        for (let i = 0; i <= 5; i++) {
            const y = this.displayHeight - this.margin.bottom - (i / 5) * this.chartHeight;
            const val = yMin + (i / 5) * (yMax - yMin);
            this.ctx.fillText(val.toFixed(0), this.margin.left - 5, y + 4);
        }
    }

    drawLineData(values, yMin, yMax, color) {
        this.ctx.strokeStyle = color;
        this.ctx.beginPath();
        
        values.forEach((val, i) => {
            const x = this.margin.left + (i / (values.length - 1)) * this.chartWidth;
            const y = this.displayHeight - this.margin.bottom - ((val - yMin) / (yMax - yMin)) * this.chartHeight;
            
            if (i === 0) {
                this.ctx.moveTo(x, y);
            } else {
                this.ctx.lineTo(x, y);
            }
        });
        this.ctx.stroke();
    }
}

const renewableStyle = document.createElement('style');
renewableStyle.textContent = `
    .renewable-container {
        padding: 1rem;
        background: rgba(0, 0, 0, 0.3);
        border-radius: 8px;
    }
    .component-title {
        color: #00d4ff;
        margin: 0 0 1rem 0;
        font-size: 1.2rem;
    }
    .renewable-status {
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
    .control-mode {
        color: #00d4ff;
        padding: 0.25rem 0.75rem;
        background: rgba(0, 212, 255, 0.1);
        border-radius: 4px;
        font-size: 0.9rem;
    }
    .renewable-metrics {
        display: grid;
        grid-template-columns: repeat(4, 1fr);
        gap: 0.75rem;
        margin-bottom: 0.75rem;
    }
    .renewable-metrics.secondary .metric-value {
        color: #FF9800;
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
    #renewable-power-canvas, #renewable-control-canvas {
        width: 100%;
        height: 200px;
        border-radius: 6px;
        margin-top: 0.5rem;
    }
    .prediction-grid {
        display: grid;
        grid-template-columns: repeat(4, 1fr);
        gap: 0.75rem;
    }
    .prediction-item {
        background: rgba(0, 0, 0, 0.3);
        padding: 0.75rem;
        border-radius: 6px;
        text-align: center;
    }
    .prediction-label {
        display: block;
        color: #888;
        font-size: 0.8rem;
        margin-bottom: 0.25rem;
    }
    .prediction-value {
        display: block;
        color: #4CAF50;
        font-weight: bold;
    }
    .optimization-result {
        display: grid;
        grid-template-columns: repeat(4, 1fr);
        gap: 0.75rem;
    }
    .opt-item {
        background: rgba(0, 0, 0, 0.3);
        padding: 0.75rem;
        border-radius: 6px;
        text-align: center;
    }
    .opt-label {
        display: block;
        color: #888;
        font-size: 0.8rem;
        margin-bottom: 0.25rem;
    }
    .opt-value {
        display: block;
        color: #FF9800;
        font-weight: bold;
    }
    .renewable-power-chart, .renewable-control-chart, 
    .renewable-predictions, .renewable-optimization {
        margin-bottom: 1rem;
    }
    .renewable-power-chart h4, .renewable-control-chart h4,
    .renewable-predictions h4, .renewable-optimization h4 {
        color: #ccc;
        margin-bottom: 0.5rem;
    }
`;
document.head.appendChild(renewableStyle);
