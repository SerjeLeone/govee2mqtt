import { LitElement, html } from "lit";

export class ConfigEditor extends LitElement {
  static properties = {
    config: { type: Object },
    devices: { type: Array },
    view: { type: String },
    editingGroup: { type: String },
    jsonText: { type: String },
    jsonError: { type: String },
    dirty: { type: Boolean },
  };

  constructor() {
    super();
    this.config = null;
    this.devices = [];
    this.view = 'groups';
    this.editingGroup = null;
    this.jsonText = '';
    this.jsonError = '';
    this.dirty = false;
  }

  createRenderRoot() { return this; }

  connectedCallback() {
    super.connectedCallback();
    this._loadConfig();
    this._loadDevices();
  }

  async _loadConfig() {
    try {
      const resp = await fetch('api/config');
      if (resp.ok) {
        this.config = await resp.json();
        this.jsonText = JSON.stringify(this.config, null, 2);
        this.dirty = false;
        this.jsonError = '';
        this.requestUpdate();
      }
    } catch (err) { /* ignore */ }
  }

  async _loadDevices() {
    try {
      const resp = await fetch('api/devices');
      if (resp.ok) {
        this.devices = await resp.json();
        this.requestUpdate();
      }
    } catch (err) { /* ignore */ }
  }

  async _saveConfig(config) {
    try {
      const resp = await fetch('api/config', {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(config),
      });
      if (resp.ok) {
        window.gvToast?.('Config saved', 'success');
        this.config = config;
        this.jsonText = JSON.stringify(config, null, 2);
        this.dirty = false;
        this.jsonError = '';
      } else {
        const data = await resp.json().catch(() => ({}));
        window.gvToast?.(data.msg || 'Save failed', 'error');
      }
    } catch (err) {
      window.gvToast?.('Save failed', 'error');
    }
    this.requestUpdate();
  }

  _addGroup() {
    if (!this.config) this.config = { devices: {}, groups: {} };
    if (!this.config.groups) this.config.groups = {};
    const id = `group-${Date.now()}`;
    this.config.groups[id] = { name: 'New Group', members: [] };
    this.editingGroup = id;
    this.dirty = true;
    this.jsonText = JSON.stringify(this.config, null, 2);
    this.requestUpdate();
  }

  _deleteGroup(id) {
    if (!confirm(`Delete group "${this.config.groups[id]?.name}"?`)) return;
    delete this.config.groups[id];
    if (this.editingGroup === id) this.editingGroup = null;
    this.dirty = true;
    this.jsonText = JSON.stringify(this.config, null, 2);
    this.requestUpdate();
  }

  _updateGroupField(groupId, field, value) {
    const group = this.config.groups[groupId];
    if (!group) return;
    if (value === '' || value === undefined) {
      delete group[field];
    } else {
      group[field] = value;
    }
    this.dirty = true;
    this.jsonText = JSON.stringify(this.config, null, 2);
    this.requestUpdate();
  }

  _updateGroupId(oldId, newId) {
    if (!newId || newId === oldId || this.config.groups[newId]) return;
    this.config.groups[newId] = this.config.groups[oldId];
    delete this.config.groups[oldId];
    this.editingGroup = newId;
    this.dirty = true;
    this.jsonText = JSON.stringify(this.config, null, 2);
    this.requestUpdate();
  }

  _toggleGroupMember(groupId, deviceId) {
    const group = this.config.groups[groupId];
    if (!group) return;
    const idx = group.members.indexOf(deviceId);
    if (idx >= 0) {
      group.members.splice(idx, 1);
    } else {
      group.members.push(deviceId);
    }
    this.dirty = true;
    this.jsonText = JSON.stringify(this.config, null, 2);
    this.requestUpdate();
  }

  _onJsonInput(e) {
    this.jsonText = e.target.value;
    this.dirty = true;
    try {
      JSON.parse(this.jsonText);
      this.jsonError = '';
    } catch (err) {
      this.jsonError = err.message;
    }
    this.requestUpdate();
  }

  _saveFromJson() {
    try {
      const config = JSON.parse(this.jsonText);
      this._saveConfig(config);
    } catch (err) {
      this.jsonError = err.message;
      this.requestUpdate();
    }
  }

  _saveFromVisual() {
    this._saveConfig(this.config);
  }

  _discardChanges() {
    this._loadConfig();
    this.editingGroup = null;
  }

  render() {
    if (!this.config) {
      return html`<div class="text-center py-5">
        <div class="spinner-border text-secondary"></div>
        <p class="mt-2 text-muted">Loading configuration...</p>
      </div>`;
    }

    return html`
      <ul class="nav nav-pills mb-3">
        <li class="nav-item">
          <a class="nav-link ${this.view === 'groups' ? 'active' : ''}" href="#"
            @click=${(e) => { e.preventDefault(); this.view = 'groups'; this.requestUpdate(); }}>
            Groups
          </a>
        </li>
        <li class="nav-item">
          <a class="nav-link ${this.view === 'overrides' ? 'active' : ''}" href="#"
            @click=${(e) => { e.preventDefault(); this.view = 'overrides'; this.requestUpdate(); }}>
            Device Overrides
          </a>
        </li>
        <li class="nav-item">
          <a class="nav-link ${this.view === 'json' ? 'active' : ''}" href="#"
            @click=${(e) => { e.preventDefault(); this.view = 'json'; this.requestUpdate(); }}>
            Raw Config
          </a>
        </li>
      </ul>
      ${this.view === 'groups' ? this._renderGroups()
        : this.view === 'overrides' ? this._renderOverrides()
        : this._renderJson()}
    `;
  }

  _renderSaveBar() {
    if (!this.dirty) return '';
    return html`
      <div class="d-flex gap-2">
        <button class="btn btn-primary btn-sm" @click=${() =>
          this.view === 'json' ? this._saveFromJson() : this._saveFromVisual()}
          ?disabled=${this.view === 'json' && !!this.jsonError}>
          Save Changes
        </button>
        <button class="btn btn-outline-secondary btn-sm" @click=${() => this._discardChanges()}>
          Discard
        </button>
      </div>`;
  }

  // ---- Groups ----

  _renderGroups() {
    const groups = this.config.groups || {};
    const entries = Object.entries(groups);

    return html`
      <div class="d-flex justify-content-between align-items-center mb-3">
        <h5 class="mb-0">Device Groups</h5>
        <div class="d-flex gap-2">
          ${this._renderSaveBar()}
          <button class="btn btn-outline-success btn-sm" @click=${() => this._addGroup()}>
            + Add Group
          </button>
        </div>
      </div>
      ${entries.length === 0
        ? html`<div class="text-center text-muted py-4">
            <p>No groups configured.</p>
            <p><small>Groups let you control multiple devices as one entity in Home Assistant.</small></p>
          </div>`
        : entries.map(([id, group]) => this._renderGroupCard(id, group))
      }
    `;
  }

  _renderGroupCard(id, group) {
    const isEditing = this.editingGroup === id;

    return html`
      <div class="card mb-3">
        <div class="card-header d-flex justify-content-between align-items-center">
          ${isEditing
            ? html`<input class="form-control form-control-sm" style="max-width:250px"
                     .value=${group.name}
                     @input=${(e) => this._updateGroupField(id, 'name', e.target.value)}>`
            : html`<strong>${group.name}</strong>`
          }
          <div class="d-flex gap-1">
            <button class="btn btn-sm btn-outline-secondary"
              @click=${() => { this.editingGroup = isEditing ? null : id; this.requestUpdate(); }}>
              ${isEditing ? 'Done' : 'Edit'}
            </button>
            <button class="btn btn-sm btn-outline-danger" @click=${() => this._deleteGroup(id)}>
              Delete
            </button>
          </div>
        </div>
        ${isEditing ? this._renderGroupEdit(id, group) : this._renderGroupSummary(id, group)}
      </div>
    `;
  }

  _renderGroupEdit(id, group) {
    return html`
      <div class="card-body">
        <div class="row g-3 mb-3">
          <div class="col-md-4">
            <label class="form-label">Group ID</label>
            <input class="form-control form-control-sm" .value=${id}
              @change=${(e) => this._updateGroupId(id, e.target.value.trim())}>
            <small class="text-muted">Used as the entity slug in HA</small>
          </div>
          <div class="col-md-4">
            <label class="form-label">Room</label>
            <input class="form-control form-control-sm" .value=${group.room || ''}
              @input=${(e) => this._updateGroupField(id, 'room', e.target.value)}>
          </div>
          <div class="col-md-4">
            <label class="form-label">Icon</label>
            <input class="form-control form-control-sm" .value=${group.icon || ''}
              placeholder="mdi:ceiling-light"
              @input=${(e) => this._updateGroupField(id, 'icon', e.target.value)}>
          </div>
        </div>
        <label class="form-label">Members <small class="text-muted">(click to toggle)</small></label>
        <div class="d-flex flex-wrap gap-2">
          ${this.devices.length === 0
            ? html`<span class="text-muted">No devices found</span>`
            : this.devices.map(dev => {
              const isMember = (group.members || []).includes(dev.id);
              return html`
                <button class="btn btn-sm ${isMember ? 'btn-primary' : 'btn-outline-secondary'}"
                  @click=${() => this._toggleGroupMember(id, dev.id)}>
                  ${dev.name}${dev.room ? html` <small class="opacity-75">(${dev.room})</small>` : ''}
                </button>
              `;
            })}
        </div>
      </div>
    `;
  }

  _renderGroupSummary(id, group) {
    return html`
      <div class="card-body py-2">
        <small class="text-muted">
          ${(group.members || []).length} member${(group.members || []).length !== 1 ? 's' : ''}
          ${group.room ? html` &middot; ${group.room}` : ''}
        </small>
        ${(group.members || []).length > 0 ? html`
          <div class="mt-1">
            ${group.members.map(m => {
              const dev = this.devices.find(d => d.id === m);
              return html`<span class="badge bg-secondary me-1">${dev ? dev.name : m}</span>`;
            })}
          </div>
        ` : ''}
      </div>
    `;
  }

  // ---- Device Overrides ----

  _renderOverrides() {
    const overrides = this.config.devices || {};
    const entries = Object.entries(overrides);

    return html`
      <div class="d-flex justify-content-between align-items-center mb-3">
        <h5 class="mb-0">Device Overrides</h5>
        ${this._renderSaveBar()}
      </div>
      ${entries.length === 0
        ? html`<div class="text-center text-muted py-4">
            <p>No device overrides configured.</p>
            <p><small>Use the Raw Config tab or edit <code>govee-device-config.json</code> to add per-device overrides.</small></p>
          </div>`
        : html`
          <div class="gv-table-wrap">
            <table class="table table-sm">
              <thead>
                <tr>
                  <th>Device ID / SKU</th>
                  <th>Name</th>
                  <th>Room</th>
                  <th>Icon</th>
                  <th>Color Temp</th>
                  <th>Flags</th>
                </tr>
              </thead>
              <tbody>
                ${entries.map(([key, ovr]) => html`
                  <tr>
                    <td><code style="font-size:0.85em">${key}</code></td>
                    <td>${ovr.name || ''}</td>
                    <td>${ovr.room || ''}</td>
                    <td>${ovr.icon ? html`<code style="font-size:0.85em">${ovr.icon}</code>` : ''}</td>
                    <td>${ovr.color_temp_range ? `${ovr.color_temp_range[0]}-${ovr.color_temp_range[1]}K` : ''}</td>
                    <td>
                      ${ovr.prefer_lan ? html`<span class="badge bg-info me-1">LAN</span>` : ''}
                      ${ovr.disable_effects ? html`<span class="badge bg-warning me-1">No FX</span>` : ''}
                    </td>
                  </tr>
                `)}
              </tbody>
            </table>
          </div>
          <small class="text-muted">Edit overrides in the Raw Config tab or directly in <code>govee-device-config.json</code>.</small>
        `
      }
    `;
  }

  // ---- Raw JSON ----

  _renderJson() {
    return html`
      <div class="d-flex justify-content-between align-items-center mb-3">
        <h5 class="mb-0">Raw Configuration</h5>
        ${this._renderSaveBar()}
      </div>
      ${this.jsonError
        ? html`<div class="alert alert-danger py-2 mb-2" style="font-size:0.9em">${this.jsonError}</div>`
        : ''}
      <textarea class="form-control font-monospace" rows="20"
        .value=${this.jsonText}
        @input=${(e) => this._onJsonInput(e)}
        style="font-size: 0.85em; tab-size: 2;"></textarea>
      <small class="text-muted mt-1 d-block">
        Edit the device configuration JSON directly. Changes are validated before saving.
        This file is located at <code>$XDG_CACHE_HOME/govee-device-config.json</code>.
      </small>
    `;
  }
}

customElements.define("gv-config-editor", ConfigEditor);
