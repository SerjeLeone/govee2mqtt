import { LitElement, html } from "lit";

export class LogViewer extends LitElement {
  static properties = {
    logs: { type: Array },
    connected: { type: Boolean },
    filter: { type: String },
    levelFilter: { type: String },
    autoScroll: { type: Boolean },
  };

  constructor() {
    super();
    this.logs = [];
    this.connected = false;
    this.filter = '';
    this.levelFilter = '';
    this.autoScroll = true;
    this.ws = null;
  }

  createRenderRoot() { return this; }

  connectedCallback() {
    super.connectedCallback();
    this._loadInitialLogs();
    this._connectWebSocket();
  }

  disconnectedCallback() {
    super.disconnectedCallback();
    if (this.ws) { this.ws.close(); this.ws = null; }
  }

  async _loadInitialLogs() {
    try {
      const resp = await fetch('api/logs');
      if (resp.ok) {
        this.logs = await resp.json();
        this.requestUpdate();
        if (this.autoScroll) this._scrollToBottom();
      }
    } catch (err) { /* ignore */ }
  }

  _connectWebSocket() {
    const proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
    const base = document.querySelector('base')?.getAttribute('href') || '/';
    const url = `${proto}//${location.host}${base}api/ws/logs`;

    this.ws = new WebSocket(url);
    this.ws.onopen = () => { this.connected = true; this.requestUpdate(); };
    this.ws.onclose = () => {
      this.connected = false;
      this.requestUpdate();
      setTimeout(() => this._connectWebSocket(), 5000);
    };
    this.ws.onmessage = (event) => {
      try {
        const entry = JSON.parse(event.data);
        this.logs = [...this.logs.slice(-499), entry];
        this.requestUpdate();
        if (this.autoScroll) this._scrollToBottom();
      } catch (e) { /* ignore */ }
    };
  }

  _scrollToBottom() {
    requestAnimationFrame(() => {
      const c = this.querySelector('#log-container');
      if (c) c.scrollTop = c.scrollHeight;
    });
  }

  _onFilterChange(e) { this.filter = e.target.value.toLowerCase(); this.requestUpdate(); }
  _setLevelFilter(level) { this.levelFilter = this.levelFilter === level ? '' : level; this.requestUpdate(); }
  _toggleAutoScroll() { this.autoScroll = !this.autoScroll; this.requestUpdate(); }
  _clearLogs() { this.logs = []; this.requestUpdate(); }

  _levelColor(level) {
    return { 'ERROR': 'danger', 'WARN': 'warning', 'INFO': 'info', 'DEBUG': 'secondary', 'TRACE': 'dark' }[level] || 'secondary';
  }

  render() {
    let filtered = this.logs;
    if (this.levelFilter) {
      filtered = filtered.filter(l => l.level === this.levelFilter);
    }
    if (this.filter) {
      filtered = filtered.filter(l =>
        l.message.toLowerCase().includes(this.filter) ||
        l.target.toLowerCase().includes(this.filter));
    }

    const levels = ['ERROR', 'WARN', 'INFO', 'DEBUG', 'TRACE'];

    return html`
      <div class="d-flex justify-content-between align-items-center mb-2 flex-wrap gap-2">
        <div class="d-flex align-items-center gap-2">
          <h5 class="mb-0">Logs</h5>
          <span class="badge ${this.connected ? 'bg-success' : 'bg-danger'}">
            ${this.connected ? 'Live' : 'Reconnecting...'}
          </span>
        </div>
        <div class="d-flex gap-1 flex-wrap">
          ${levels.map(level => html`
            <button class="btn btn-sm ${this.levelFilter === level ? `btn-${this._levelColor(level)}` : `btn-outline-${this._levelColor(level)}`}"
              @click=${() => this._setLevelFilter(level)}>
              ${level}
            </button>
          `)}
        </div>
        <div class="d-flex gap-2 align-items-center">
          <input type="text" class="form-control form-control-sm" style="width:180px"
            placeholder="Filter..." @input=${this._onFilterChange}>
          <button class="btn btn-sm btn-outline-secondary" title="Toggle auto-scroll"
            @click=${this._toggleAutoScroll}>
            ${this.autoScroll ? 'Auto-scroll ON' : 'Auto-scroll OFF'}
          </button>
          <button class="btn btn-sm btn-outline-danger" @click=${this._clearLogs}>Clear</button>
        </div>
      </div>
      <div id="log-container" class="gv-log-container bg-dark rounded p-2">
        ${filtered.length === 0
          ? html`<p class="text-muted text-center py-4">No log entries${this.levelFilter ? ` for level ${this.levelFilter}` : ''}</p>`
          : filtered.map(entry => html`
            <div class="mb-1">
              <span class="text-muted">${entry.timestamp?.substring(11, 19) || ''}</span>
              <span class="badge bg-${this._levelColor(entry.level)}" style="width:45px;font-size:0.75em">${entry.level}</span>
              <span class="text-info">${entry.target?.replace('govee::', '') || ''}</span>
              ${entry.message}
            </div>
          `)}
      </div>
      <small class="text-muted">${filtered.length} of ${this.logs.length} entries</small>
    `;
  }
}

customElements.define("gv-log-viewer", LogViewer);
