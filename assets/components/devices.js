import { LitElement, html } from "lit";
import { Task } from '@lit/task';
import { timeAgo } from './timeago.js';

export class DeviceList extends LitElement {
  timer;
  deviceList;
  scenesCache = {};
  musicModesCache = {};
  expandedDevice = null;
  inspectCache = {};
  hideUnavailable = true;

  static properties = {
    expandedDevice: { type: String },
    hideUnavailable: { type: Boolean },
  };

  constructor() {
    super();
    try {
      this.hideUnavailable = localStorage.getItem('gv-hide-unavailable-devices') !== 'false';
    } catch (_) {
      this.hideUnavailable = true;
    }
  }

  _deviceListTask = new Task(this, {
    task: async ([], {signal}) => {
      const response = await fetch('api/devices', {signal});
      if (!response.ok) throw new Error(response.status);
      return response.json();
    },
    args: () => []
  });

  render() {
    return this._deviceListTask.render({
      pending: () => this.deviceList
        ? this._render_device_list(this.deviceList)
        : html`<div class="text-center py-5"><div class="spinner-border text-secondary"></div><p class="mt-2 text-muted">Loading devices...</p></div>`,
      complete: (devices) => {
        this.deviceList = devices;
        return this._render_device_list(devices);
      },
      error: () => html`<div class="alert alert-danger">Failed to load devices. Is the bridge running?</div>`
    });
  }

  createRenderRoot() { return this; }

  connectedCallback() {
    super.connectedCallback();
    this.timer = setInterval(() => this._deviceListTask.run(), 5000);
  }

  disconnectedCallback() {
    super.disconnectedCallback();
    clearInterval(this.timer);
  }

  async _apiAction(url, label) {
    try {
      const resp = await fetch(url, { method: 'POST' });
      if (resp.ok) {
        window.gvToast?.(`${label}`, 'success');
      } else {
        window.gvToast?.(`${label} failed (${resp.status})`, 'error');
      }
    } catch (err) {
      window.gvToast?.(`${label} failed`, 'error');
    }
  }

  _set_power_on(e) {
    e.stopPropagation();
    const id = encodeURIComponent(e.target.dataset.id);
    const name = e.target.dataset.name || id;
    const power = e.target.checked ? 'on' : 'off';
    this._apiAction(`api/device/${id}/power/${power}`, `${name} ${power}`);
  }

  _set_dreamview(e) {
    e.stopPropagation();
    const id = encodeURIComponent(e.target.dataset.id);
    const name = e.target.dataset.name || id;
    const enabled = e.target.checked ? 'on' : 'off';
    this._apiAction(
      `api/device/${id}/dreamview/${enabled}`,
      `${name} DreamView ${enabled}`,
    );
  }

  _set_color(e) {
    e.stopPropagation();
    const id = encodeURIComponent(e.target.dataset.id);
    const color = encodeURIComponent(e.target.value);
    fetch(`api/device/${id}/color/${color}`, { method: 'POST' });
  }

  _set_brightness(e) {
    e.stopPropagation();
    const id = encodeURIComponent(e.target.dataset.id);
    const level = e.target.value;
    // Update the label immediately
    const label = e.target.parentElement?.querySelector('.gv-brightness-label');
    if (label) label.textContent = `${level}%`;
    fetch(`api/device/${id}/brightness/${level}`, { method: 'POST' });
  }

  _set_scene(e) {
    e.stopPropagation();
    const id = encodeURIComponent(e.target.dataset.id);
    const name = e.target.dataset.name || id;
    const scene = e.target.value;
    if (scene) {
      this._apiAction(`api/device/${id}/scene/${encodeURIComponent(scene)}`, `${name}: ${scene}`);
    }
  }

  _set_music_mode(e) {
    e.stopPropagation();
    const id = encodeURIComponent(e.target.dataset.id);
    const name = e.target.dataset.name || id;
    const mode = e.target.value;
    if (mode) {
      this._apiAction(
        `api/device/${id}/music-mode/${encodeURIComponent(mode)}`,
        `${name}: Music mode ${mode}`,
      );
    }
  }

  async _loadScenes(device_id) {
    if (this.scenesCache[device_id]) return this.scenesCache[device_id];
    try {
      const resp = await fetch(`api/device/${encodeURIComponent(device_id)}/scenes`);
      if (resp.ok) {
        this.scenesCache[device_id] = await resp.json();
        this.requestUpdate();
        return this.scenesCache[device_id];
      }
    } catch (err) { /* ignore */ }
    return [];
  }

  async _loadMusicModes(device_id) {
    if (this.musicModesCache[device_id]) return this.musicModesCache[device_id];
    try {
      const resp = await fetch(`api/device/${encodeURIComponent(device_id)}/music-modes`);
      if (resp.ok) {
        this.musicModesCache[device_id] = await resp.json();
        this.requestUpdate();
        return this.musicModesCache[device_id];
      }
    } catch (err) { /* ignore */ }
    return [];
  }

  async _toggleDetail(e) {
    const id = e.currentTarget.dataset.id;
    if (this.expandedDevice === id) {
      this.expandedDevice = null;
    } else {
      this.expandedDevice = id;
      if (!this.inspectCache[id]) {
        try {
          const resp = await fetch(`api/device/${encodeURIComponent(id)}/inspect`);
          if (resp.ok) this.inspectCache[id] = await resp.json();
        } catch (err) {
          this.inspectCache[id] = { error: err.message };
        }
      }
    }
    this.requestUpdate();
  }

  _render_detail = (item) => {
    const data = this.inspectCache[item.safe_id];
    if (!data) return html`<tr><td colspan="3"><div class="text-center py-3"><div class="spinner-border spinner-border-sm"></div></div></td></tr>`;

    const stateJson = data.current_state
      ? JSON.stringify(data.current_state, null, 2)
      : null;

    const sceneGroups = data.platform_scene_capability_names
      ? Object.entries(data.platform_scene_capability_names)
      : [];

    return html`
      <tr class="gv-detail-panel">
        <td colspan="3" class="p-0">
          <div class="card border-0 bg-body-tertiary m-2">
            <div class="card-body">
              <div class="row g-3">
                <div class="col-md-4">
                  <h6 class="text-muted mb-2">Device</h6>
                  <dl class="row mb-0" style="font-size:0.9em">
                    <dt class="col-4">ID</dt><dd class="col-8"><code style="font-size:0.85em">${data.id || item.id}</code></dd>
                    <dt class="col-4">Model</dt><dd class="col-8">${data.sku || item.sku}</dd>
                    ${item.is_virtual ? html`
                      <dt class="col-4">Type</dt><dd class="col-8">
                        ${item.virtual_kind === 'dream_view_scene' ? 'DreamView scene' : 'Group control'}
                      </dd>
                      <dt class="col-4">Members</dt><dd class="col-8">
                        ${item.virtual_member_count ?? 'Unknown'}
                      </dd>` : ''}
                    <dt class="col-4">Room</dt><dd class="col-8">${data.room || 'Not assigned'}</dd>
                    ${data.active_scene ? html`<dt class="col-4">Scene</dt><dd class="col-8"><span class="badge bg-primary">${data.active_scene}</span></dd>` : ''}
                  </dl>
                </div>
                <div class="col-md-4">
                  <h6 class="text-muted mb-2">Scenes</h6>
                  ${sceneGroups.length > 0
                    ? html`<ul class="list-unstyled mb-0" style="font-size:0.9em">
                        ${sceneGroups.map(([inst, names]) =>
                          html`<li><span class="badge bg-outline-secondary border me-1">${inst}</span> ${names.length} available</li>`
                        )}
                      </ul>`
                    : html`<p class="text-muted mb-0" style="font-size:0.9em">No scene data from Platform API</p>`
                  }
                  ${data.platform_music_mode_names?.length
                    ? html`<p class="mt-1 mb-0" style="font-size:0.9em"><span class="badge bg-outline-secondary border">Music</span> ${data.platform_music_mode_names.length} modes</p>`
                    : ''
                  }
                </div>
                <div class="col-md-4">
                  <h6 class="text-muted mb-2">State</h6>
                  ${stateJson
                    ? html`<pre class="bg-dark rounded p-2 mb-0" style="font-size:0.75em;max-height:180px;overflow:auto">${stateJson}</pre>`
                    : html`<p class="text-muted mb-0" style="font-size:0.9em">No state data yet</p>`
                  }
                </div>
              </div>
              ${data.platform_device_info?.capabilities?.length
                ? html`
                  <div class="mt-2 pt-2 border-top">
                    <small class="text-muted">${data.platform_device_info.capabilities.length} capabilities:
                      ${data.platform_device_info.capabilities.map(cap =>
                        html`<span class="badge bg-dark border me-1">${cap.instance}</span>`
                      )}
                    </small>
                  </div>` : ''
              }
            </div>
          </div>
        </td>
      </tr>
    `;
  }

  _render_badge(label, active, activeClass = 'text-bg-info') {
    if (active === undefined || active === null) {
      return html`<span class="badge rounded-pill text-bg-secondary">${label}: unknown</span>`;
    }
    return html`<span class="badge rounded-pill ${active ? activeClass : 'text-bg-secondary'}">${label}</span>`;
  }

  _render_cloud_badge(item) {
    if (!item.cloud_supported) {
      return html`<span class="badge rounded-pill text-bg-secondary">Cloud: unavailable</span>`;
    }
    if (item.cloud_online === true) {
      return html`<span class="badge rounded-pill text-bg-success">Cloud: online</span>`;
    }
    if (item.cloud_online === false) {
      return html`<span class="badge rounded-pill text-bg-danger">Cloud: offline</span>`;
    }
    if (!item.cloud_status_observable) {
      return html`<span class="badge rounded-pill text-bg-info">Cloud: control only</span>`;
    }
    return html`<span class="badge rounded-pill text-bg-secondary">Cloud: checking…</span>`;
  }

  _toggleUnavailableFilter(e) {
    this.hideUnavailable = e.target.checked;
    try {
      localStorage.setItem('gv-hide-unavailable-devices', String(this.hideUnavailable));
    } catch (_) { /* persistence is optional */ }
  }

  _status_summary(devices, virtualControls) {
    const mqttConfigured = devices.some((item) => item.mqtt_configured);
    const apiCount = devices.filter((item) => item.api_metadata).length;
    const lanCount = devices.filter((item) => item.lan_discovered).length;
    return html`
      <div class="device-status-summary mb-3">
        ${this._render_badge('MQTT configured', mqttConfigured)}
        <span class="badge rounded-pill text-bg-info">API metadata: ${apiCount}</span>
        <span class="badge rounded-pill text-bg-info">LAN discovered: ${lanCount}</span>
        ${virtualControls.length > 0
          ? html`<span class="badge rounded-pill text-bg-secondary">
              Group/scene controls: ${virtualControls.length}
            </span>`
          : ''}
        <label class="form-check form-switch device-availability-filter mb-0"
               title="Hide devices that have not answered through LAN or cloud">
          <input class="form-check-input" type="checkbox" role="switch"
                 .checked=${this.hideUnavailable}
                 @change=${this._toggleUnavailableFilter}>
          <span class="form-check-label">Hide unavailable</span>
        </label>
      </div>`;
  }

  _room_groups(devices) {
    const groups = new Map();
    for (const item of devices) {
      const room = item.room || 'Unassigned';
      if (!groups.has(room)) groups.set(room, []);
      groups.get(room).push(item);
    }
    return [...groups.entries()];
  }

  _render_item = (item) => {
    const hasState = !!item.state;
    const isOn = item.state?.on ?? false;
    const brightness = item.state?.brightness ?? 0;
    const color_value = hasState
      ? (item.state.color.r << 16) | (item.state.color.g << 8) | item.state.color.b
      : 0;
    const rgb_hex = `#${color_value.toString(16).padStart(6, '0')}`;
    const updated = hasState ? timeAgo(new Date(item.state.updated)) : '';
    const source = item.state?.source || '';

    const scenes = item.is_virtual
      ? []
      : (this.scenesCache[item.safe_id] || [])
          .filter(scene => scene && !scene.toLowerCase().startsWith('music: '));
    if (!item.is_virtual && !this.scenesCache[item.safe_id]) this._loadScenes(item.safe_id);
    const active_scene = item.state?.scene || '';
    const musicModes = item.supports_music_mode
      ? (this.musicModesCache[item.safe_id] || [])
      : [];
    if (item.supports_music_mode && !this.musicModesCache[item.safe_id]) {
      this._loadMusicModes(item.safe_id);
    }
    const activeMusicMode = item.active_music_mode
      || (active_scene.startsWith('Music: ') ? active_scene.slice('Music: '.length) : '');

    const isExpanded = this.expandedDevice === item.safe_id;

    const rows = [html`
      <tr class="gv-device-row ${isExpanded ? 'table-active' : ''}"
          data-id=${item.safe_id} @click=${this._toggleDetail}>
        <td>
          <span class="gv-status-dot ${item.available ? 'online' : 'offline'}"></span>
          <strong>${item.name}</strong>
          ${item.room ? html`<br><small class="text-muted">${item.room}</small>` : ''}
          <div class="d-flex flex-wrap gap-1 mt-1">
            ${item.is_virtual
              ? html`
                  <span class="badge rounded-pill text-bg-info">
                    ${item.virtual_kind === 'dream_view_scene' ? 'DreamView scene' : 'Group'}
                  </span>
                  <span class="badge rounded-pill text-bg-secondary">Platform control</span>
                  ${item.virtual_member_count != null
                    ? html`<span class="badge rounded-pill text-bg-secondary">
                        ${item.virtual_member_count} member${item.virtual_member_count !== 1 ? 's' : ''}
                      </span>`
                    : ''}
                `
              : html`
                  ${this._render_badge('API', item.api_metadata)}
                  ${this._render_badge('LAN', item.lan_discovered)}
                `}
            ${item.is_virtual ? '' : this._render_cloud_badge(item)}
          </div>
        </td>
        <td class="gv-device-controls" @click=${(e) => e.stopPropagation()}>
          <div class="d-flex align-items-center gap-2 flex-wrap">
            <span class="form-switch mb-0">
              <input data-id=${item.safe_id} data-name=${item.name}
                class="form-check-input" type="checkbox" role="switch"
                @click=${this._set_power_on} ?checked=${isOn}
                .indeterminate=${!hasState}
                title=${hasState ? 'Power' : 'Power state is not known yet'}>
            </span>
            ${item.supports_brightness ? html`
              <div class="d-flex align-items-center gap-1">
                <input type="range" class="form-range" style="width:80px" min="0" max="100"
                  data-id=${item.safe_id} @change=${this._set_brightness} value=${brightness}>
                <span class="gv-brightness-label">${brightness}%</span>
              </div>` : ''}
            ${item.supports_rgb ? html`
              <input class="form-control form-control-color p-0 border-0" style="width:28px;height:28px"
                data-id=${item.safe_id} @change=${this._set_color} type="color" value=${rgb_hex}>` : ''}
            ${scenes.length > 0 ? html`
              <select class="form-select form-select-sm gv-scene-select"
                data-id=${item.safe_id} data-name=${item.name}
                @change=${this._set_scene} @click=${(e) => e.stopPropagation()}>
                <option value="">Scene...</option>
                ${scenes.filter(s => s).map(s => html`
                  <option value=${s} ?selected=${s === active_scene}>${s}</option>
                `)}
              </select>` : ''}
            ${musicModes.length > 0 ? html`
              <select class="form-select form-select-sm gv-scene-select gv-music-mode-select"
                data-id=${item.safe_id} data-name=${item.name}
                title="Platform API music mode"
                @change=${this._set_music_mode} @click=${(e) => e.stopPropagation()}>
                <option value="">Music mode...</option>
                ${musicModes.filter(mode => mode).map(mode => html`
                  <option value=${mode} ?selected=${mode === activeMusicMode}>${mode}</option>
                `)}
              </select>` : ''}
            ${item.supports_dreamview ? html`
              <label class="form-check form-switch d-flex align-items-center gap-1 mb-0 gv-dreamview-control"
                     title="LAN-first hardware DreamView control">
                <input class="form-check-input mt-0" type="checkbox" role="switch"
                  data-id=${item.safe_id} data-name=${item.name}
                  @click=${this._set_dreamview}
                  ?checked=${item.dreamview_enabled === true}
                  .indeterminate=${item.dreamview_enabled == null}>
                <span class="small">DreamView</span>
              </label>` : ''}
          </div>
        </td>
        <td class="text-end d-none d-md-table-cell">
          <small class="text-muted">${updated}</small>
          ${source ? html`<br><span class="badge rounded-pill bg-dark border" style="font-size:0.7em">${source}</span>` : ''}
        </td>
      </tr>
    `];

    if (isExpanded) rows.push(this._render_detail(item));
    return rows;
  }

  _render_device_list = (devices) => {
    if (devices.length === 0) {
      return html`
        <div class="gv-empty-state">
          <h5>No devices found</h5>
          <p>Make sure your Govee credentials are configured and the bridge can reach the Govee API.</p>
          <p><small class="text-muted">Devices discovered via LAN, IoT, or Platform API will appear here automatically.</small></p>
        </div>`;
    }

    const physicalDevices = devices.filter((item) => !item.is_virtual);
    const virtualControls = devices.filter((item) => item.is_virtual);
    const visibleDevices = this.hideUnavailable
      ? physicalDevices.filter((item) => item.available)
      : physicalDevices;
    const visibleVirtualControls = this.hideUnavailable
      ? virtualControls.filter((item) => item.available)
      : virtualControls;
    const renderSection = (title, items, firstColumn = 'Device') => items.length > 0 ? html`
      <section class="device-room-section mb-4">
        <h2>${title}</h2>
        <div class="gv-table-wrap">
          <table class="table table-hover align-middle mb-2">
            <thead>
              <tr>
                <th>${firstColumn}</th>
                <th>Controls</th>
                <th class="text-end d-none d-md-table-cell">Status</th>
              </tr>
            </thead>
            <tbody>${items.map(this._render_item)}</tbody>
          </table>
        </div>
      </section>` : '';

    return html`
      ${this._status_summary(physicalDevices, virtualControls)}
      ${visibleDevices.length === 0
        ? html`<div class="alert alert-secondary">
            All unavailable devices are hidden. Turn off <strong>Hide unavailable</strong>
            to inspect them.
          </div>`
        : ''}
      ${renderSection('Groups & scenes', visibleVirtualControls, 'Group or scene')}
      ${this._room_groups(visibleDevices).map(([room, items]) => html`
        ${renderSection(room, items)}`)}
      <small class="text-muted">
        ${visibleDevices.length === physicalDevices.length
          ? `${physicalDevices.length} device${physicalDevices.length !== 1 ? 's' : ''}`
          : `${visibleDevices.length} of ${physicalDevices.length} devices shown`}.
        ${virtualControls.length > 0
          ? `${visibleVirtualControls.length} of ${virtualControls.length} group/scene controls shown.`
          : ''}
        Click a row for details.
      </small>`;
  }
}

customElements.define("gv-device-list", DeviceList);
