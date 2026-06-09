class LineChart {
    constructor(canvasId, options = {}) {
        this.canvas = document.getElementById(canvasId);
        this.ctx = this.canvas.getContext('2d');
        this.data = [];
        this.options = {
            title: '',
            xLabel: '',
            yLabel: '',
            lineColor: '#00d4ff',
            fillColor: 'rgba(0, 212, 255, 0.1)',
            gridColor: 'rgba(255, 255, 255, 0.1)',
            textColor: '#e0e0e0',
            showGrid: true,
            showPoints: true,
            animation: true,
            ...options
        };
        
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
        
        this.margin = {
            top: 20,
            right: 20,
            bottom: 40,
            left: 50
        };
        
        this.chartWidth = this.displayWidth - this.margin.left - this.margin.right;
        this.chartHeight = this.displayHeight - this.margin.top - this.margin.bottom;
    }

    setData(data) {
        this.data = data || [];
        this.render();
    }

    render() {
        this.ctx.clearRect(0, 0, this.displayWidth, this.displayHeight);
        
        this.drawBackground();
        
        if (this.data.length === 0) {
            this.drawNoData();
            return;
        }
        
        const xValues = this.data.map(d => d.x);
        const yValues = this.data.map(d => d.y);
        
        const xMin = Math.min(...xValues);
        const xMax = Math.max(...xValues);
        let yMin = Math.min(...yValues);
        let yMax = Math.max(...yValues);
        
        const yPadding = (yMax - yMin) * 0.1 || 1;
        yMin -= yPadding;
        yMax += yPadding;
        
        if (this.options.showGrid) {
            this.drawGrid(xMin, xMax, yMin, yMax);
        }
        
        this.drawAxes(xMin, xMax, yMin, yMax);
        this.drawLine(xMin, xMax, yMin, yMax);
        
        if (this.options.showPoints) {
            this.drawPoints(xMin, xMax, yMin, yMax);
        }
        
        this.drawLabels();
    }

    drawBackground() {
        this.ctx.fillStyle = 'rgba(0, 0, 0, 0.3)';
        this.ctx.fillRect(0, 0, this.displayWidth, this.displayHeight);
    }

    drawNoData() {
        this.ctx.fillStyle = '#666';
        this.ctx.font = '14px "Segoe UI", sans-serif';
        this.ctx.textAlign = 'center';
        this.ctx.textBaseline = 'middle';
        this.ctx.fillText('暂无数据', this.displayWidth / 2, this.displayHeight / 2);
    }

    drawGrid(xMin, xMax, yMin, yMax) {
        this.ctx.strokeStyle = this.options.gridColor;
        this.ctx.lineWidth = 1;
        
        const xTicks = 5;
        for (let i = 0; i <= xTicks; i++) {
            const x = this.margin.left + (i / xTicks) * this.chartWidth;
            this.ctx.beginPath();
            this.ctx.moveTo(x, this.margin.top);
            this.ctx.lineTo(x, this.margin.top + this.chartHeight);
            this.ctx.stroke();
        }
        
        const yTicks = 4;
        for (let i = 0; i <= yTicks; i++) {
            const y = this.margin.top + (i / yTicks) * this.chartHeight;
            this.ctx.beginPath();
            this.ctx.moveTo(this.margin.left, y);
            this.ctx.lineTo(this.margin.left + this.chartWidth, y);
            this.ctx.stroke();
        }
    }

    drawAxes(xMin, xMax, yMin, yMax) {
        this.ctx.strokeStyle = 'rgba(255, 255, 255, 0.3)';
        this.ctx.lineWidth = 2;
        
        this.ctx.beginPath();
        this.ctx.moveTo(this.margin.left, this.margin.top);
        this.ctx.lineTo(this.margin.left, this.margin.top + this.chartHeight);
        this.ctx.lineTo(this.margin.left + this.chartWidth, this.margin.top + this.chartHeight);
        this.ctx.stroke();
    }

    getX(x, xMin, xMax) {
        return this.margin.left + ((x - xMin) / (xMax - xMin)) * this.chartWidth;
    }

    getY(y, yMin, yMax) {
        return this.margin.top + this.chartHeight - ((y - yMin) / (yMax - yMin)) * this.chartHeight;
    }

    drawLine(xMin, xMax, yMin, yMax) {
        if (this.data.length < 2) return;
        
        this.ctx.beginPath();
        this.ctx.moveTo(
            this.getX(this.data[0].x, xMin, xMax),
            this.getY(this.data[0].y, yMin, yMax)
        );
        
        for (let i = 1; i < this.data.length; i++) {
            const x = this.getX(this.data[i].x, xMin, xMax);
            const y = this.getY(this.data[i].y, yMin, yMax);
            this.ctx.lineTo(x, y);
        }
        
        this.ctx.strokeStyle = this.options.lineColor;
        this.ctx.lineWidth = 2;
        this.ctx.stroke();
        
        this.ctx.lineTo(
            this.getX(this.data[this.data.length - 1].x, xMin, xMax),
            this.margin.top + this.chartHeight
        );
        this.ctx.lineTo(
            this.getX(this.data[0].x, xMin, xMax),
            this.margin.top + this.chartHeight
        );
        this.ctx.closePath();
        
        const gradient = this.ctx.createLinearGradient(0, this.margin.top, 0, this.margin.top + this.chartHeight);
        gradient.addColorStop(0, this.options.fillColor);
        gradient.addColorStop(1, 'rgba(0, 0, 0, 0)');
        this.ctx.fillStyle = gradient;
        this.ctx.fill();
    }

    drawPoints(xMin, xMax, yMin, yMax) {
        for (let i = 0; i < this.data.length; i += Math.max(1, Math.floor(this.data.length / 20))) {
            const x = this.getX(this.data[i].x, xMin, xMax);
            const y = this.getY(this.data[i].y, yMin, yMax);
            
            this.ctx.beginPath();
            this.ctx.arc(x, y, 3, 0, Math.PI * 2);
            this.ctx.fillStyle = this.options.lineColor;
            this.ctx.fill();
            
            this.ctx.strokeStyle = '#fff';
            this.ctx.lineWidth = 1;
            this.ctx.stroke();
        }
    }

    drawLabels() {
        this.ctx.fillStyle = this.options.textColor;
        this.ctx.font = '11px "Segoe UI", sans-serif';
        this.ctx.textAlign = 'center';
        
        if (this.options.title) {
            this.ctx.font = 'bold 12px "Segoe UI", sans-serif';
            this.ctx.fillText(this.options.title, this.displayWidth / 2, this.margin.top / 2 + 5);
        }
        
        if (this.options.xLabel) {
            this.ctx.font = '11px "Segoe UI", sans-serif';
            this.ctx.fillText(
                this.options.xLabel,
                this.displayWidth / 2,
                this.displayHeight - 8
            );
        }
        
        this.ctx.save();
        this.ctx.translate(12, this.displayHeight / 2);
        this.ctx.rotate(-Math.PI / 2);
        if (this.options.yLabel) {
            this.ctx.fillText(this.options.yLabel, 0, 0);
        }
        this.ctx.restore();
    }

    formatTime(timestamp) {
        const date = new Date(timestamp);
        return date.toLocaleTimeString('zh-CN', {
            hour: '2-digit',
            minute: '2-digit'
        });
    }
}

class EfficiencyChart {
    constructor(canvasId) {
        this.canvas = document.getElementById(canvasId);
        this.ctx = this.canvas.getContext('2d');
        this.efficiencyCurve = [];
        this.polarizationCurve = [];
        this.currentPoint = null;
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
        
        this.margin = {
            top: 30,
            right: 30,
            bottom: 40,
            left: 50
        };
        
        this.chartWidth = this.displayWidth - this.margin.left - this.margin.right;
        this.chartHeight = this.displayHeight - this.margin.top - this.margin.bottom;
    }

    setData(efficiencyCurve, polarizationCurve, currentPoint) {
        this.efficiencyCurve = efficiencyCurve || [];
        this.polarizationCurve = polarizationCurve || [];
        this.currentPoint = currentPoint;
        this.render();
    }

    render() {
        this.ctx.clearRect(0, 0, this.displayWidth, this.displayHeight);
        
        this.ctx.fillStyle = 'rgba(0, 0, 0, 0.3)';
        this.ctx.fillRect(0, 0, this.displayWidth, this.displayHeight);
        
        if (this.efficiencyCurve.length === 0 && this.polarizationCurve.length === 0) {
            this.ctx.fillStyle = '#666';
            this.ctx.font = '14px "Segoe UI", sans-serif';
            this.ctx.textAlign = 'center';
            this.ctx.textBaseline = 'middle';
            this.ctx.fillText('暂无数据', this.displayWidth / 2, this.displayHeight / 2);
            return;
        }
        
        const xMin = 0.5;
        const xMax = 4.0;
        const yMin = 0;
        const yMax = 100;
        
        this.drawGrid(xMin, xMax, yMin, yMax);
        this.drawAxes(xMin, xMax, yMin, yMax);
        
        if (this.efficiencyCurve.length > 0) {
            this.drawEfficiencyCurve(xMin, xMax, yMin, yMax);
        }
        
        if (this.currentPoint) {
            this.drawCurrentPoint(xMin, xMax, yMin, yMax);
        }
        
        this.drawLabels();
        this.drawLegend();
    }

    drawGrid(xMin, xMax, yMin, yMax) {
        this.ctx.strokeStyle = 'rgba(255, 255, 255, 0.1)';
        this.ctx.lineWidth = 1;
        
        const xTicks = 7;
        for (let i = 0; i <= xTicks; i++) {
            const x = this.margin.left + (i / xTicks) * this.chartWidth;
            this.ctx.beginPath();
            this.ctx.moveTo(x, this.margin.top);
            this.ctx.lineTo(x, this.margin.top + this.chartHeight);
            this.ctx.stroke();
            
            this.ctx.fillStyle = '#888';
            this.ctx.font = '10px "Segoe UI", sans-serif';
            this.ctx.textAlign = 'center';
            const xValue = xMin + (i / xTicks) * (xMax - xMin);
            this.ctx.fillText(xValue.toFixed(1), x, this.margin.top + this.chartHeight + 20);
        }
        
        const yTicks = 5;
        for (let i = 0; i <= yTicks; i++) {
            const y = this.margin.top + (i / yTicks) * this.chartHeight;
            this.ctx.beginPath();
            this.ctx.moveTo(this.margin.left, y);
            this.ctx.lineTo(this.margin.left + this.chartWidth, y);
            this.ctx.stroke();
            
            this.ctx.fillStyle = '#888';
            this.ctx.textAlign = 'right';
            const yValue = yMax - (i / yTicks) * (yMax - yMin);
            this.ctx.fillText(yValue.toFixed(0), this.margin.left - 5, y + 3);
        }
    }

    drawAxes(xMin, xMax, yMin, yMax) {
        this.ctx.strokeStyle = 'rgba(255, 255, 255, 0.3)';
        this.ctx.lineWidth = 2;
        
        this.ctx.beginPath();
        this.ctx.moveTo(this.margin.left, this.margin.top);
        this.ctx.lineTo(this.margin.left, this.margin.top + this.chartHeight);
        this.ctx.lineTo(this.margin.left + this.chartWidth, this.margin.top + this.chartHeight);
        this.ctx.stroke();
    }

    getX(x, xMin, xMax) {
        return this.margin.left + ((x - xMin) / (xMax - xMin)) * this.chartWidth;
    }

    getY(y, yMin, yMax) {
        return this.margin.top + this.chartHeight - ((y - yMin) / (yMax - yMin)) * this.chartHeight;
    }

    drawEfficiencyCurve(xMin, xMax, yMin, yMax) {
        if (this.efficiencyCurve.length < 2) return;
        
        this.ctx.beginPath();
        this.ctx.moveTo(
            this.getX(this.efficiencyCurve[0][0], xMin, xMax),
            this.getY(this.efficiencyCurve[0][1], yMin, yMax)
        );
        
        for (let i = 1; i < this.efficiencyCurve.length; i++) {
            const x = this.getX(this.efficiencyCurve[i][0], xMin, xMax);
            const y = this.getY(this.efficiencyCurve[i][1], yMin, yMax);
            this.ctx.lineTo(x, y);
        }
        
        const gradient = this.ctx.createLinearGradient(0, this.margin.top, 0, this.margin.top + this.chartHeight);
        gradient.addColorStop(0, 'rgba(76, 175, 80, 0.3)');
        gradient.addColorStop(1, 'rgba(76, 175, 80, 0)');
        
        this.ctx.lineTo(
            this.getX(this.efficiencyCurve[this.efficiencyCurve.length - 1][0], xMin, xMax),
            this.margin.top + this.chartHeight
        );
        this.ctx.lineTo(
            this.getX(this.efficiencyCurve[0][0], xMin, xMax),
            this.margin.top + this.chartHeight
        );
        this.ctx.closePath();
        this.ctx.fillStyle = gradient;
        this.ctx.fill();
        
        this.ctx.beginPath();
        this.ctx.moveTo(
            this.getX(this.efficiencyCurve[0][0], xMin, xMax),
            this.getY(this.efficiencyCurve[0][1], yMin, yMax)
        );
        
        for (let i = 1; i < this.efficiencyCurve.length; i++) {
            const x = this.getX(this.efficiencyCurve[i][0], xMin, xMax);
            const y = this.getY(this.efficiencyCurve[i][1], yMin, yMax);
            this.ctx.lineTo(x, y);
        }
        
        this.ctx.strokeStyle = '#4CAF50';
        this.ctx.lineWidth = 2.5;
        this.ctx.stroke();
        
        this.ctx.beginPath();
        this.ctx.moveTo(
            this.getX(this.polarizationCurve[0][0], xMin, xMax),
            this.getY(this.polarizationCurve[0][1] * 40, yMin, yMax)
        );
        
        for (let i = 1; i < this.polarizationCurve.length; i++) {
            const x = this.getX(this.polarizationCurve[i][0], xMin, xMax);
            const y = this.getY(this.polarizationCurve[i][1] * 40, yMin, yMax);
            this.ctx.lineTo(x, y);
        }
        
        this.ctx.strokeStyle = '#FF9800';
        this.ctx.lineWidth = 2;
        this.ctx.setLineDash([5, 5]);
        this.ctx.stroke();
        this.ctx.setLineDash([]);
    }

    drawCurrentPoint(xMin, xMax, yMin, yMax) {
        const x = this.getX(this.currentPoint.currentDensity, xMin, xMax);
        const y = this.getY(this.currentPoint.efficiency, yMin, yMax);
        
        this.ctx.beginPath();
        this.ctx.arc(x, y, 8, 0, Math.PI * 2);
        this.ctx.fillStyle = 'rgba(244, 67, 54, 0.3)';
        this.ctx.fill();
        
        this.ctx.beginPath();
        this.ctx.arc(x, y, 5, 0, Math.PI * 2);
        this.ctx.fillStyle = '#F44336';
        this.ctx.fill();
        this.ctx.strokeStyle = '#fff';
        this.ctx.lineWidth = 2;
        this.ctx.stroke();
        
        this.ctx.fillStyle = '#F44336';
        this.ctx.font = 'bold 11px "Segoe UI", sans-serif';
        this.ctx.textAlign = 'center';
        this.ctx.fillText(
            `当前: ${this.currentPoint.efficiency.toFixed(1)}%`,
            x,
            y - 15
        );
    }

    drawLabels() {
        this.ctx.fillStyle = '#e0e0e0';
        this.ctx.font = '11px "Segoe UI", sans-serif';
        this.ctx.textAlign = 'center';
        this.ctx.fillText(
            '电流密度 (A/cm²)',
            this.displayWidth / 2,
            this.displayHeight - 8
        );
        
        this.ctx.save();
        this.ctx.translate(12, this.displayHeight / 2);
        this.ctx.rotate(-Math.PI / 2);
        this.ctx.fillText('效率 (%) / 电压×40 (V)', 0, 0);
        this.ctx.restore();
    }

    drawLegend() {
        const legends = [
            { label: '效率曲线', color: '#4CAF50', dashed: false },
            { label: '极化曲线', color: '#FF9800', dashed: true },
            { label: '当前工作点', color: '#F44336', dashed: false }
        ];
        
        let x = this.displayWidth - 120;
        let y = this.margin.top + 10;
        
        legends.forEach(legend => {
            this.ctx.beginPath();
            if (legend.dashed) {
                this.ctx.setLineDash([5, 3]);
            }
            this.ctx.moveTo(x, y);
            this.ctx.lineTo(x + 25, y);
            this.ctx.strokeStyle = legend.color;
            this.ctx.lineWidth = 2;
            this.ctx.stroke();
            this.ctx.setLineDash([]);
            
            if (legend.label === '当前工作点') {
                this.ctx.beginPath();
                this.ctx.arc(x + 12, y, 4, 0, Math.PI * 2);
                this.ctx.fillStyle = legend.color;
                this.ctx.fill();
            }
            
            this.ctx.fillStyle = '#e0e0e0';
            this.ctx.font = '10px "Segoe UI", sans-serif';
            this.ctx.textAlign = 'left';
            this.ctx.fillText(legend.label, x + 30, y + 3);
            
            y += 20;
        });
    }
}

class ModalChart {
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
    }

    setData(data, options = {}) {
        this.data = data || [];
        this.options = {
            lineColor: '#00d4ff',
            fillColor: 'rgba(0, 212, 255, 0.1)',
            ...options
        };
        this.render();
    }

    render() {
        this.ctx.clearRect(0, 0, this.displayWidth, this.displayHeight);
        
        if (this.data.length === 0) {
            this.ctx.fillStyle = '#666';
            this.ctx.font = '12px "Segoe UI", sans-serif';
            this.ctx.textAlign = 'center';
            this.ctx.textBaseline = 'middle';
            this.ctx.fillText('暂无数据', this.displayWidth / 2, this.displayHeight / 2);
            return;
        }
        
        const margin = { top: 20, right: 20, bottom: 30, left: 50 };
        const chartWidth = this.displayWidth - margin.left - margin.right;
        const chartHeight = this.displayHeight - margin.top - margin.bottom;
        
        const xValues = this.data.map(d => new Date(d.timestamp).getTime());
        const yValues = this.data.map(d => d.value);
        
        const xMin = Math.min(...xValues);
        const xMax = Math.max(...xValues);
        let yMin = Math.min(...yValues);
        let yMax = Math.max(...yValues);
        
        const yPadding = (yMax - yMin) * 0.1 || 1;
        yMin -= yPadding;
        yMax += yPadding;
        
        this.ctx.strokeStyle = 'rgba(255, 255, 255, 0.1)';
        this.ctx.lineWidth = 1;
        
        for (let i = 0; i <= 4; i++) {
            const y = margin.top + (i / 4) * chartHeight;
            this.ctx.beginPath();
            this.ctx.moveTo(margin.left, y);
            this.ctx.lineTo(margin.left + chartWidth, y);
            this.ctx.stroke();
            
            this.ctx.fillStyle = '#888';
            this.ctx.font = '10px "Segoe UI", sans-serif';
            this.ctx.textAlign = 'right';
            const yValue = yMax - (i / 4) * (yMax - yMin);
            this.ctx.fillText(yValue.toFixed(2), margin.left - 5, y + 3);
        }
        
        this.ctx.strokeStyle = 'rgba(255, 255, 255, 0.3)';
        this.ctx.lineWidth = 1;
        this.ctx.beginPath();
        this.ctx.moveTo(margin.left, margin.top);
        this.ctx.lineTo(margin.left, margin.top + chartHeight);
        this.ctx.lineTo(margin.left + chartWidth, margin.top + chartHeight);
        this.ctx.stroke();
        
        this.ctx.beginPath();
        const getX = (x) => margin.left + ((x - xMin) / (xMax - xMin)) * chartWidth;
        const getY = (y) => margin.top + chartHeight - ((y - yMin) / (yMax - yMin)) * chartHeight;
        
        this.ctx.moveTo(getX(xValues[0]), getY(yValues[0]));
        
        for (let i = 1; i < this.data.length; i++) {
            const x = getX(xValues[i]);
            const y = getY(yValues[i]);
            this.ctx.lineTo(x, y);
        }
        
        this.ctx.lineTo(getX(xValues[xValues.length - 1]), margin.top + chartHeight);
        this.ctx.lineTo(getX(xValues[0]), margin.top + chartHeight);
        this.ctx.closePath();
        
        const gradient = this.ctx.createLinearGradient(0, margin.top, 0, margin.top + chartHeight);
        gradient.addColorStop(0, this.options.fillColor);
        gradient.addColorStop(1, 'rgba(0, 0, 0, 0)');
        this.ctx.fillStyle = gradient;
        this.ctx.fill();
        
        this.ctx.beginPath();
        this.ctx.moveTo(getX(xValues[0]), getY(yValues[0]));
        
        for (let i = 1; i < this.data.length; i++) {
            const x = getX(xValues[i]);
            const y = getY(yValues[i]);
            this.ctx.lineTo(x, y);
        }
        
        this.ctx.strokeStyle = this.options.lineColor;
        this.ctx.lineWidth = 2;
        this.ctx.stroke();
        
        this.ctx.fillStyle = '#888';
        this.ctx.font = '10px "Segoe UI", sans-serif';
        this.ctx.textAlign = 'center';
        
        const timeStep = Math.floor(this.data.length / 5);
        for (let i = 0; i < this.data.length; i += timeStep) {
            const x = getX(xValues[i]);
            const time = new Date(xValues[i]);
            const timeStr = time.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' });
            this.ctx.fillText(timeStr, x, margin.top + chartHeight + 20);
        }
    }
}
