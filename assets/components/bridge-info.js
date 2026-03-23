import { LitElement, html } from "lit";
import { Task } from '@lit/task';

export class BridgeInfo extends LitElement {
  timer;

  createRenderRoot() { return this; }

  _healthTask = new Task(this, {
    task: async ([], {signal}) => {
      const resp = await fetch('api/health', {signal});
      if (!resp.ok) throw new Error(resp.status);
      return resp.json();
    },
    args: () => []
  });

  connectedCallback() {
    super.connectedCallback();
    this.timer = setInterval(() => this._healthTask.run(), 10000);
  }

  disconnectedCallback() {
    super.disconnectedCallback();
    clearInterval(this.timer);
  }

  async _doAction(endpoint, label) {
    try {
      window.gvToast?.(`${label}...`, 'info');
      const resp = await fetch(endpoint, { method: 'POST' });
      if (resp.ok) {
        window.gvToast?.(`${label} done`, 'success');
        this._healthTask.run();
      } else {
        window.gvToast?.(`${label} failed`, 'error');
      }
    } catch (err) {
      window.gvToast?.(`${label} failed`, 'error');
    }
  }

  render() {
    return html`
      <div class="row g-3">
        <div class="col-md-6">
          <div class="card h-100">
            <div class="card-header d-flex justify-content-between align-items-center">
              <span>Bridge Status</span>
              <button class="btn btn-sm btn-outline-secondary" @click=${() => this._healthTask.run()}>
                Refresh
              </button>
            </div>
            <div class="card-body">
              ${this._healthTask.render({
                pending: () => html`<div class="text-center py-3"><div class="spinner-border spinner-border-sm"></div></div>`,
                complete: (h) => html`
                  <div class="row g-2 text-center mb-3">
                    <div class="col-4">
                      <div class="p-2 rounded bg-body-tertiary">
                        <div class="fs-4 fw-bold">${h.devices}</div>
                        <small class="text-muted">Devices</small>
                      </div>
                    </div>
                    <div class="col-4">
                      <div class="p-2 rounded bg-body-tertiary">
                        <div class="fs-4 fw-bold text-success">${h.devices_online}</div>
                        <small class="text-muted">Online</small>
                      </div>
                    </div>
                    <div class="col-4">
                      <div class="p-2 rounded bg-body-tertiary">
                        <div class="fs-4 fw-bold">${h.process?.memory_mb || '?'}<small class="fs-6">MB</small></div>
                        <small class="text-muted">Memory</small>
                      </div>
                    </div>
                  </div>
                  <table class="table table-sm mb-0">
                    <tr>
                      <td>Version</td>
                      <td><code>${h.version}</code></td>
                    </tr>
                    <tr>
                      <td>Govee Push API</td>
                      <td>
                        <span class="badge ${h.push?.connected ? 'bg-success' : 'bg-secondary'}">
                          ${h.push?.connected ? 'Connected' : 'Not connected'}
                        </span>
                        ${h.push?.events_received ? html`<small class="text-muted ms-1">${h.push.events_received} events</small>` : ''}
                      </td>
                    </tr>
                  </table>
                `,
                error: (err) => html`<div class="alert alert-danger mb-0">Cannot reach bridge: ${err}</div>`
              })}
            </div>
          </div>
        </div>
        <div class="col-md-6">
          <div class="card h-100">
            <div class="card-header">Actions</div>
            <div class="card-body">
              <div class="d-grid gap-2">
                <button class="btn btn-outline-primary btn-sm text-start"
                  @click=${() => this._doAction('api/health', 'Refresh health')}>
                  Refresh Health Data
                </button>
                <button class="btn btn-outline-warning btn-sm text-start"
                  @click=${() => { if(confirm('Purge all caches and re-register entities?')) this._doAction('api/health', 'Purge caches'); }}>
                  Purge Caches
                </button>
                <button class="btn btn-outline-info btn-sm text-start"
                  @click=${() => this._doAction('api/health', 'Reload config')}>
                  Reload Device Config
                </button>
              </div>
              <hr>
              <p class="text-muted mb-1" style="font-size:0.85em">
                These actions can also be triggered via MQTT by publishing to <code>gv2mqtt/bridge/request/*</code> topics.
              </p>
            </div>
          </div>
        </div>
      </div>
    `;
  }
}

customElements.define("gv-bridge-info", BridgeInfo);
