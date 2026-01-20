import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { enable, disable, isEnabled } from '@tauri-apps/plugin-autostart';

//INFO: Types
interface UserProfile {
    display_name: string;
    location: string | null;
    theme: string;
}

interface HotkeyConfig {
    modifier_keys: string[];
    key: string;
    enabled: boolean;
}

interface ApiKeyStatus {
    provider: string;
    is_configured: boolean;
    masked_key: string | null;
}

function SettingsPage() {
    //INFO: State
    const [displayName, setDisplayName] = useState('');
    const [location, setLocation] = useState('');
    const [hotkeyModifiers, setHotkeyModifiers] = useState<string[]>(['Super']);
    const [hotkeyKey, setHotkeyKey] = useState('L');
    const [geminiApiKey, setGeminiApiKey] = useState('');
    const [geminiKeyConfigured, setGeminiKeyConfigured] = useState(false);
    const [databasePath, setDatabasePath] = useState('');
    const [autostartEnabled, setAutostartEnabled] = useState(false);

    //INFO: UI state
    const [saving, setSaving] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [success, setSuccess] = useState<string | null>(null);

    //INFO: Load settings on mount
    useEffect(() => {
        loadSettings();
    }, []);

    //INFO: Clear success message after 3s
    useEffect(() => {
        if (success) {
            const timer = setTimeout(() => setSuccess(null), 3000);
            return () => clearTimeout(timer);
        }
    }, [success]);

    async function loadSettings() {
        try {
            const profile = await invoke<UserProfile | null>('get_profile');
            if (profile) {
                setDisplayName(profile.display_name);
                setLocation(profile.location || '');
            }

            const hotkey = await invoke<HotkeyConfig | null>('get_hotkey');
            if (hotkey) {
                setHotkeyModifiers(hotkey.modifier_keys);
                setHotkeyKey(hotkey.key);
            }

            const geminiStatus = await invoke<ApiKeyStatus>('get_api_key_status', { provider: 'gemini' });
            setGeminiKeyConfigured(geminiStatus.is_configured);

            const dbPath = await invoke<string>('get_database_path');
            setDatabasePath(dbPath);

            const isAutostart = await isEnabled();
            setAutostartEnabled(isAutostart);
        } catch (err) {
            setError(`Failed to load settings: ${err}`);
        }
    }

    async function toggleAutostart() {
        try {
            if (autostartEnabled) {
                await disable();
            } else {
                await enable();
            }
            setAutostartEnabled(!autostartEnabled);
            setSuccess(autostartEnabled ? 'Auto-launch disabled' : 'Auto-launch enabled');
        } catch (err) {
            setError(`Failed to update auto-launch: ${err}`);
        }
    }

    async function saveProfile() {
        setSaving(true);
        setError(null);
        try {
            await invoke('update_profile', { request: { display_name: displayName, location: location || null, theme: 'light' } });
            setSuccess('Profile saved');
        } catch (err) {
            setError(`Failed to save profile: ${err}`);
        } finally {
            setSaving(false);
        }
    }

    async function saveHotkey() {
        setSaving(true);
        setError(null);
        try {
            await invoke('update_hotkey', { request: { modifier_keys: hotkeyModifiers, key: hotkeyKey, enabled: true } });
            setSuccess('Hotkey saved (restart to apply)');
        } catch (err) {
            setError(`Failed to save hotkey: ${err}`);
        } finally {
            setSaving(false);
        }
    }

    async function saveApiKey() {
        if (!geminiApiKey.trim()) return;
        setSaving(true);
        setError(null);
        try {
            await invoke('update_api_key', { request: { provider: 'gemini', api_key: geminiApiKey } });
            setGeminiApiKey('');
            setGeminiKeyConfigured(true);
            setSuccess('API key saved');
        } catch (err) {
            setError(`Failed to save API key: ${err}`);
        } finally {
            setSaving(false);
        }
    }

    function toggleModifier(mod: string) {
        if (hotkeyModifiers.includes(mod)) {
            setHotkeyModifiers(hotkeyModifiers.filter(m => m !== mod));
        } else {
            setHotkeyModifiers([...hotkeyModifiers, mod]);
        }
    }

    return (
        <div className="animate-fade-in">
            <h2 style={{ marginBottom: 'var(--spacing-6)' }}>Settings</h2>

            {error && <div className="error-message">{error}</div>}
            {success && <div style={{ padding: 'var(--spacing-3)', background: '#e6f4ea', borderRadius: 'var(--radius-md)', color: 'var(--color-success)', fontSize: 'var(--font-size-sm)', marginBottom: 'var(--spacing-3)' }}>{success}</div>}

            {/* App Settings */}
            <section className="settings-section">
                <h4 className="settings-section-title">App</h4>
                <div className="settings-card">
                    <div className="settings-row">
                        <div className="settings-row-info">
                            <span className="settings-row-title">Launch on Startup</span>
                            <span className="settings-row-description">Start Lumen automatically when you log in.</span>
                        </div>
                        <label className="switch">
                            <input
                                type="checkbox"
                                checked={autostartEnabled}
                                onChange={toggleAutostart}
                            />
                            <span className="slider"></span>
                        </label>
                    </div>
                </div>
            </section>

            {/* Profile */}
            <section className="settings-section">
                <h4 className="settings-section-title">Profile</h4>
                <div className="settings-card">
                    <div className="input-group" style={{ marginBottom: 'var(--spacing-4)' }}>
                        <label className="input-label">Display Name</label>
                        <input
                            type="text"
                            className="input"
                            value={displayName}
                            onChange={(e) => setDisplayName(e.target.value)}
                            placeholder="Your name"
                        />
                    </div>
                    <div className="input-group" style={{ marginBottom: 'var(--spacing-6)' }}>
                        <label className="input-label">Home Location</label>
                        <input
                            type="text"
                            className="input"
                            value={location}
                            onChange={(e) => setLocation(e.target.value)}
                            placeholder="e.g. Lagos, London, New York"
                        />
                        <span style={{ fontSize: 'var(--font-size-xs)', color: 'var(--color-text-tertiary)', marginTop: 'var(--spacing-1)' }}>
                            Used for accurate weather in your daily briefing.
                        </span>
                    </div>
                    <button className="btn btn-primary" onClick={saveProfile} disabled={saving}>
                        Save Profile
                    </button>
                </div>
            </section>

            {/* Hotkey */}
            <section className="settings-section">
                <h4 className="settings-section-title">Hotkey</h4>
                <div className="settings-card">
                    <div style={{ marginBottom: 'var(--spacing-4)' }}>
                        <label className="input-label" style={{ marginBottom: 'var(--spacing-2)', display: 'block' }}>Modifiers</label>
                        <div style={{ display: 'flex', gap: 'var(--spacing-2)' }}>
                            {['Super', 'Ctrl', 'Alt', 'Shift'].map(mod => (
                                <button
                                    key={mod}
                                    className={`btn btn-sm ${hotkeyModifiers.includes(mod) ? 'btn-primary' : ''}`}
                                    onClick={() => toggleModifier(mod)}
                                >
                                    {mod}
                                </button>
                            ))}
                        </div>
                    </div>
                    <div className="input-group" style={{ marginBottom: 'var(--spacing-4)' }}>
                        <label className="input-label">Key</label>
                        <input
                            type="text"
                            className="input"
                            value={hotkeyKey}
                            onChange={(e) => setHotkeyKey(e.target.value.toUpperCase())}
                            maxLength={1}
                            style={{ width: '60px', textAlign: 'center' }}
                        />
                    </div>
                    <div style={{ marginBottom: 'var(--spacing-4)' }}>
                        <span style={{ fontSize: 'var(--font-size-sm)', color: 'var(--color-text-secondary)' }}>
                            Current: {hotkeyModifiers.map(m => <kbd key={m} className="kbd" style={{ marginRight: '4px' }}>{m}</kbd>)} + <kbd className="kbd">{hotkeyKey}</kbd>
                        </span>
                    </div>
                    <button className="btn btn-primary" onClick={saveHotkey} disabled={saving}>
                        Save
                    </button>
                </div>
            </section>

            {/* API Key */}
            <section className="settings-section">
                <h4 className="settings-section-title">Gemini API Key</h4>
                <div className="settings-card">
                    {geminiKeyConfigured && (
                        <div style={{ marginBottom: 'var(--spacing-3)' }}>
                            <span className="badge badge-success">Configured</span>
                        </div>
                    )}
                    <div className="input-group" style={{ marginBottom: 'var(--spacing-4)' }}>
                        <input
                            type="password"
                            className="input"
                            value={geminiApiKey}
                            onChange={(e) => setGeminiApiKey(e.target.value)}
                            placeholder={geminiKeyConfigured ? 'Enter new key to update...' : 'Enter your API key...'}
                        />
                    </div>
                    <button className="btn btn-primary" onClick={saveApiKey} disabled={saving || !geminiApiKey.trim()}>
                        Save
                    </button>
                    <p style={{ marginTop: 'var(--spacing-3)', fontSize: 'var(--font-size-xs)', color: 'var(--color-text-tertiary)' }}>
                        Get your key from <a href="https://aistudio.google.com/apikey" target="_blank" rel="noreferrer">Google AI Studio</a>
                    </p>
                </div>
            </section>

            {/* Data */}
            <section className="settings-section">
                <h4 className="settings-section-title">Data</h4>
                <div className="settings-card">
                    <p style={{ fontSize: 'var(--font-size-sm)', color: 'var(--color-text-secondary)', marginBottom: 'var(--spacing-2)' }}>
                        Database location:
                    </p>
                    <code style={{ fontSize: 'var(--font-size-xs)', color: 'var(--color-text-tertiary)', wordBreak: 'break-all' }}>
                        {databasePath}
                    </code>
                </div>
            </section>
        </div>
    );
}

export default SettingsPage;
