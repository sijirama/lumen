//INFO: Integrations page - Minimal design
//NOTE: Clean integration cards

import { useState, useEffect } from 'react';
import { Check } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';

//INFO: Integration type
interface Integration {
    name: string;
    enabled: boolean;
    config: string | null;
    last_sync: string | null;
    status: string;
}

function IntegrationsPage() {
    const [integrations, setIntegrations] = useState<Integration[]>([]);
    const [error, setError] = useState<string | null>(null);

    useEffect(() => {
        loadIntegrations();
    }, []);

    async function loadIntegrations() {
        try {
            const data = await invoke<Integration[]>('get_integrations');
            setIntegrations(data);
        } catch (err) {
            setError(`Failed to load integrations: ${err}`);
        }
    }

    function getIntegration(name: string): Integration | undefined {
        return integrations.find(i => i.name === name);
    }

    async function toggleCalendar() {
        const current = getIntegration('google_calendar');
        const newEnabled = !current?.enabled;

        try {
            await invoke('update_integration', {
                integration: {
                    name: 'google_calendar',
                    enabled: newEnabled,
                    config: current?.config || null,
                    last_sync: null,
                    status: newEnabled ? 'connected' : 'disconnected'
                }
            });
            await loadIntegrations();
        } catch (err) {
            setError(`Failed: ${err}`);
        }
    }

    async function selectObsidianVault() {
        try {
            const selectedPath = await open({ directory: true, multiple: false, title: 'Select Vault' });
            if (selectedPath && typeof selectedPath === 'string') {
                await invoke('update_integration', {
                    integration: {
                        name: 'obsidian',
                        enabled: true,
                        config: JSON.stringify({ vault_path: selectedPath }),
                        last_sync: null,
                        status: 'connected'
                    }
                });
                await loadIntegrations();
            }
        } catch (err) {
            setError(`Failed: ${err}`);
        }
    }

    async function disconnectObsidian() {
        try {
            await invoke('update_integration', {
                integration: { name: 'obsidian', enabled: false, config: null, last_sync: null, status: 'disconnected' }
            });
            await loadIntegrations();
        } catch (err) {
            setError(`Failed: ${err}`);
        }
    }

    function getVaultPath(): string | null {
        const obs = getIntegration('obsidian');
        if (!obs?.config) return null;
        try {
            return JSON.parse(obs.config).vault_path;
        } catch {
            return null;
        }
    }

    const calendar = getIntegration('google_calendar');
    const obsidian = getIntegration('obsidian');
    const vaultPath = getVaultPath();

    return (
        <div className="animate-fade-in">
            <h2 style={{ marginBottom: 'var(--spacing-6)' }}>Integrations</h2>

            {error && <div className="error-message">{error}</div>}

            {/* Google Calendar */}
            <div className="integration-card">
                <div className="integration-icon" style={{ background: '#e8f0fe' }}>📅</div>
                <div className="integration-info">
                    <div className="integration-name">Google Calendar</div>
                    <div className="integration-status">
                        {calendar?.enabled ? (
                            <span style={{ color: 'var(--color-success)', display: 'flex', alignItems: 'center', gap: '4px' }}>
                                <Check size={12} /> Connected
                            </span>
                        ) : 'Not connected'}
                    </div>
                </div>
                <button className={`btn btn-sm ${calendar?.enabled ? '' : 'btn-primary'}`} onClick={toggleCalendar}>
                    {calendar?.enabled ? 'Disconnect' : 'Connect'}
                </button>
            </div>

            {/* Obsidian */}
            <div className={`integration-card ${obsidian?.enabled ? 'enabled' : ''}`}>
                <div className="integration-icon" style={{ background: '#f3e8ff' }}>📝</div>
                <div className="integration-info">
                    <div className="integration-name">Obsidian</div>
                    <div className="integration-status">
                        {obsidian?.enabled ? (
                            <span style={{ color: 'var(--color-success)', display: 'flex', alignItems: 'center', gap: '4px' }}>
                                <Check size={12} /> Connected
                            </span>
                        ) : 'Not connected'}
                    </div>
                </div>
                <div style={{ display: 'flex', gap: 'var(--spacing-2)' }}>
                    {obsidian?.enabled ? (
                        <button className="btn btn-sm" onClick={disconnectObsidian}>Disconnect</button>
                    ) : (
                        <button className="btn btn-sm btn-primary" onClick={selectObsidianVault}>Select Vault</button>
                    )}
                </div>
            </div>

            {obsidian?.enabled && (
                <div className="integration-config-panel animate-fade-in">
                    <div className="config-section">
                        <div className="config-header">
                            <div className="config-title">Vault Configuration</div>
                            <div className="config-path">{vaultPath}</div>
                        </div>

                        <div className="config-grid">
                            <div className="config-item">
                                <label>Daily Notes Folder</label>
                                <div className="picker-group">
                                    <input
                                        type="text"
                                        readOnly
                                        className="input input-sm"
                                        placeholder="Root folder"
                                        value={(() => {
                                            try { return JSON.parse(obsidian.config || '{}').daily_notes_path || '' } catch { return '' }
                                        })()}
                                    />
                                    <button className="btn btn-sm" onClick={async () => {
                                        try {
                                            const selected = await open({ directory: true, multiple: false, title: 'Select Daily Notes Folder', defaultPath: vaultPath || undefined });
                                            if (selected && typeof selected === 'string') {
                                                // Make path relative to vault if possible
                                                let relativePath = selected;
                                                if (vaultPath && selected.startsWith(vaultPath)) {
                                                    relativePath = selected.replace(vaultPath, '').replace(/^\//, '');
                                                }
                                                const config = JSON.parse(obsidian.config || '{}');
                                                await invoke('update_integration', {
                                                    integration: {
                                                        ...obsidian,
                                                        config: JSON.stringify({ ...config, daily_notes_path: relativePath })
                                                    }
                                                });
                                                loadIntegrations();
                                            }
                                        } catch (err) { setError(`Failed to select folder: ${err}`); }
                                    }}>Change</button>
                                </div>
                            </div>

                            <div className="config-item">
                                <label>Date Format</label>
                                <div className="picker-group">
                                    <select
                                        className="input input-sm"
                                        value={(() => {
                                            try { return JSON.parse(obsidian.config || '{}').daily_notes_format || 'YYYY-MM-DD' } catch { return 'YYYY-MM-DD' }
                                        })()}
                                        onChange={async (e) => {
                                            const newFormat = e.target.value;
                                            const config = JSON.parse(obsidian.config || '{}');
                                            await invoke('update_integration', {
                                                integration: {
                                                    ...obsidian,
                                                    config: JSON.stringify({ ...config, daily_notes_format: newFormat })
                                                }
                                            });
                                            loadIntegrations();
                                        }}
                                    >
                                        <option value="YYYY-MM-DD">YYYY-MM-DD (Standard)</option>
                                        <option value="DD-MM-YYYY">DD-MM-YYYY</option>
                                        <option value="MM-DD-YYYY">MM-DD-YYYY</option>
                                        <option value="YYYY.MM.DD">YYYY.MM.DD</option>
                                        <option value="DD.MM.YYYY">DD.MM.YYYY</option>
                                    </select>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>
            )}

            {/* Email - Coming Soon */}
            <div className="integration-card disabled">
                <div className="integration-icon" style={{ background: '#fce8e6' }}>📧</div>
                <div className="integration-info">
                    <div className="integration-name">Email</div>
                    <div className="integration-status">Coming soon</div>
                </div>
                <button className="btn btn-sm" disabled>Soon</button>
            </div>
        </div>
    );
}

export default IntegrationsPage;
