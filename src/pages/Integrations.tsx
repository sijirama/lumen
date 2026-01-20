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

            // Pre-fill Google credentials if they exist
            const g = data.find(i => i.name === 'google');
            if (g?.config) {
                try {
                    const cfg = JSON.parse(g.config);
                    if (cfg.client_id) setGoogleClientId(cfg.client_id);
                    if (cfg.client_secret) setGoogleClientSecret(cfg.client_secret);
                } catch (e) { console.error('Failed to parse Google config', e); }
            }
        } catch (err) {
            setError(`Failed to load integrations: ${err}`);
        }
    }

    function getIntegration(name: string): Integration | undefined {
        return integrations.find(i => i.name === name);
    }

    // Google Auth State
    const [googleClientId, setGoogleClientId] = useState('');
    const [googleClientSecret, setGoogleClientSecret] = useState('');
    const [isAuthenticating, setIsAuthenticating] = useState(false);

    async function handleGoogleAuth() {
        if (!googleClientId.trim() || !googleClientSecret.trim()) {
            setError('Please enter both Google Client ID and Client Secret');
            return;
        }

        setIsAuthenticating(true);
        setError(null);
        try {
            // 1. Save config first
            await invoke('save_google_config', {
                clientId: googleClientId,
                clientSecret: googleClientSecret
            });

            // 2. Start interactive auth
            // This will open the browser and block until redirect is caught
            await invoke('start_google_auth');

            await loadIntegrations();
        } catch (err) {
            setError(`Google Authentication failed: ${err}`);
        } finally {
            setIsAuthenticating(false);
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

    const google = getIntegration('google');
    const obsidian = getIntegration('obsidian');
    const vaultPath = getVaultPath();

    return (
        <div className="animate-fade-in text-slate-200">
            <h2 style={{ marginBottom: 'var(--spacing-6)' }}>Integrations</h2>

            {error && <div className="error-message">{error}</div>}

            {/* Google Services */}
            <div className={`integration-card ${google?.enabled ? 'enabled' : ''}`}>
                <div className="integration-icon" style={{ background: '#e8f0fe' }}>G</div>
                <div className="integration-info">
                    <div className="integration-name">Google Services</div>
                    <div className="integration-description" style={{ fontSize: 'var(--font-size-xs)', color: 'var(--color-text-tertiary)' }}>
                        Connect to Google Calendar and Gmail
                    </div>
                    <div className="integration-status">
                        {google?.enabled ? (
                            <span style={{ color: 'var(--color-success)', display: 'flex', alignItems: 'center', gap: '4px' }}>
                                <Check size={12} /> Connected
                            </span>
                        ) : 'Not connected'}
                    </div>
                </div>
                <div style={{ display: 'flex', gap: 'var(--spacing-2)' }}>
                    {google?.enabled && (
                        <button className="btn btn-sm" onClick={async () => {
                            try {
                                // Extract current config to keep credentials
                                if (google.config) {
                                    const cfg = JSON.parse(google.config);
                                    setGoogleClientId(cfg.client_id || '');
                                    setGoogleClientSecret(cfg.client_secret || '');
                                }
                                await invoke('update_integration', {
                                    integration: { name: 'google', enabled: false, config: google.config, last_sync: google.last_sync, status: 'disconnected' }
                                });
                                await loadIntegrations();
                            } catch (err) {
                                setError(`Failed to disconnect: ${err}`);
                            }
                        }}>Disconnect</button>
                    )}
                </div>
            </div>

            {!google?.enabled && (
                <div className="integration-config-panel animate-fade-in" style={{ marginBottom: 'var(--spacing-6)' }}>
                    <div className="config-section">
                        <div className="config-title">Google API Credentials</div>
                        <p style={{ fontSize: 'var(--font-size-xs)', color: 'var(--color-text-secondary)', marginBottom: 'var(--spacing-4)' }}>
                            Enter your Google Cloud Project credentials to enable local OAuth.
                            Lumen handles the handshake entirely on your machine.
                        </p>
                        <div className="config-grid">
                            <div className="config-item">
                                <label>Client ID</label>
                                <input
                                    type="text"
                                    className="input input-sm"
                                    placeholder="458...-apps.googleusercontent.com"
                                    value={googleClientId}
                                    onChange={(e) => setGoogleClientId(e.target.value)}
                                />
                            </div>
                            <div className="config-item">
                                <label>Client Secret</label>
                                <input
                                    type="password"
                                    className="input input-sm"
                                    placeholder="GOCSPX-..."
                                    value={googleClientSecret}
                                    onChange={(e) => setGoogleClientSecret(e.target.value)}
                                />
                            </div>
                        </div>
                        <button
                            className="btn btn-sm btn-primary"
                            style={{ marginTop: 'var(--spacing-4)' }}
                            onClick={handleGoogleAuth}
                            disabled={isAuthenticating}
                        >
                            {isAuthenticating ? 'Authenticating...' : 'Authorize & Connect'}
                        </button>
                    </div>
                </div>
            )}

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

        </div>
    );
}

export default IntegrationsPage;
