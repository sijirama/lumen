import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { enable, disable, isEnabled } from '@tauri-apps/plugin-autostart';
import { CheckCircle2, AlertCircle, Copy } from 'lucide-react';
import Button from '../components/ui/Button';

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
    snipper_modifier_keys: string[];
    snipper_key: string;
    snipper_enabled: boolean;
}

function SettingsPage() {
    //INFO: State
    const [displayName, setDisplayName] = useState('');
    const [location, setLocation] = useState('');

    // Main Hotkey
    const [hotkeyModifiers, setHotkeyModifiers] = useState<string[]>(['Super']);
    const [hotkeyKey, setHotkeyKey] = useState('L');

    // Snipper Hotkey
    const [snipperModifiers, setSnipperModifiers] = useState<string[]>(['Super', 'Shift']);
    const [snipperKey, setSnipperKey] = useState('S');

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
                // Load Snipper config if available (backend defaults handle nulls)
                if (hotkey.snipper_modifier_keys) setSnipperModifiers(hotkey.snipper_modifier_keys);
                if (hotkey.snipper_key) setSnipperKey(hotkey.snipper_key);
            }

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
            await invoke('update_hotkey', {
                request: {
                    modifier_keys: hotkeyModifiers,
                    key: hotkeyKey,
                    enabled: true,
                    snipper_modifier_keys: snipperModifiers,
                    snipper_key: snipperKey,
                    snipper_enabled: true
                }
            });
            setSuccess('Shortcuts saved (restart to apply)');
        } catch (err) {
            setError(`Failed to save hotkey: ${err}`);
        } finally {
            setSaving(false);
        }
    }

    function toggleModifier(mod: string, isMain: boolean) {
        if (isMain) {
            if (hotkeyModifiers.includes(mod)) {
                setHotkeyModifiers(hotkeyModifiers.filter(m => m !== mod));
            } else {
                setHotkeyModifiers([...hotkeyModifiers, mod]);
            }
        } else {
            if (snipperModifiers.includes(mod)) {
                setSnipperModifiers(snipperModifiers.filter(m => m !== mod));
            } else {
                setSnipperModifiers([...snipperModifiers, mod]);
            }
        }
    }

    async function copyDatabasePath() {
        await navigator.clipboard.writeText(databasePath);
        setSuccess('Database path copied');
    }

    return (
        <div className="admin-page animate-fade-in">
            <div className="admin-page-header">
                <h2>Settings</h2>
                {success && (
                    <div className="flex max-w-full items-center gap-1.5 rounded-full bg-background-tertiary px-3 py-1 text-xs font-medium text-success">
                        <CheckCircle2 size={12} />
                        {success}
                    </div>
                )}
                {error && (
                    <div className="flex max-w-full items-center gap-1.5 rounded-full bg-[#fce8e6] px-3 py-1 text-xs font-medium text-error">
                        <AlertCircle size={12} />
                        <span className="min-w-0 break-words">{error}</span>
                    </div>
                )}
            </div>

            {/* General Settings */}
            <section className="admin-section">
                <h4 className="admin-section-title">
                    General
                </h4>
                <div className="settings-card px-4 py-3">
                    <div className="settings-row">
                        <div className="settings-row-info">
                            <span className="settings-row-title text-[0.9rem]">Launch on Startup</span>
                            <span className="settings-row-description text-[0.8rem]">Start Lumen automatically when you log in.</span>
                        </div>
                        <label className="switch scale-[0.8]">
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
            <section className="admin-section">
                <h4 className="admin-section-title">
                    Personalization
                </h4>
                <div className="settings-card p-4">
                    <div className="mb-3">
                        <label className="mb-1 block text-[0.8rem] font-medium text-foreground-secondary">Display Name</label>
                        <input
                            type="text"
                            className="input px-2.5 py-1.5 text-[0.9rem]"
                            value={displayName}
                            onChange={(e) => setDisplayName(e.target.value)}
                            placeholder="Your name"
                        />
                    </div>
                    <div className="mb-4">
                        <label className="mb-1 block text-[0.8rem] font-medium text-foreground-secondary">Home Location</label>
                        <input
                            type="text"
                            className="input px-2.5 py-1.5 text-[0.9rem]"
                            value={location}
                            onChange={(e) => setLocation(e.target.value)}
                            placeholder="e.g. Lagos, London"
                        />
                        <div className="mt-1 text-xs text-foreground-tertiary">
                            Used for local weather updates.
                        </div>
                    </div>
                    <div className="flex justify-end">
                        <Button size="sm" onClick={saveProfile} disabled={saving}>
                            Save Changes
                        </Button>
                    </div>
                </div>
            </section>

            {/* Hotkey */}
            <section className="admin-section">
                <h4 className="admin-section-title">
                    Shortcuts
                </h4>
                <div className="settings-card p-4">

                    {/* Main Activation */}
                    <div className="settings-stack">
                        <label className="mb-1.5 block text-[0.8rem] font-medium text-foreground-secondary">Activation Hotkey</label>
                        <div className="hotkey-row">
                            <div className="hotkey-modifiers">
                                {['Super', 'Ctrl', 'Alt', 'Shift'].map(mod => (
                                    <button
                                        key={mod}
                                        onClick={() => toggleModifier(mod, true)}
                                        className={`admin-pill ${hotkeyModifiers.includes(mod) ? 'active' : ''}`}
                                    >
                                        {mod}
                                    </button>
                                ))}
                            </div>
                            <span className="hotkey-plus">+</span>
                            <div className="hotkey-key-slot">
                                <span className="hotkey-key-label">Key</span>
                                <input
                                    type="text"
                                    value={hotkeyKey}
                                    onChange={(e) => setHotkeyKey(e.target.value.toUpperCase())}
                                    maxLength={1}
                                    className="hotkey-key-input"
                                />
                            </div>
                        </div>
                    </div>

                    {/* Snipper Shortcut */}
                    <div className="settings-stack">
                        <label className="mb-1.5 block text-[0.8rem] font-medium text-foreground-secondary">Snipping Tool</label>
                        <div className="hotkey-row">
                            <div className="hotkey-modifiers">
                                {['Super', 'Ctrl', 'Alt', 'Shift'].map(mod => (
                                    <button
                                        key={mod}
                                        onClick={() => toggleModifier(mod, false)}
                                        className={`admin-pill ${snipperModifiers.includes(mod) ? 'active' : ''}`}
                                    >
                                        {mod}
                                    </button>
                                ))}
                            </div>
                            <span className="hotkey-plus">+</span>
                            <div className="hotkey-key-slot">
                                <span className="hotkey-key-label">Key</span>
                                <input
                                    type="text"
                                    value={snipperKey}
                                    onChange={(e) => setSnipperKey(e.target.value.toUpperCase())}
                                    maxLength={1}
                                    className="hotkey-key-input"
                                />
                            </div>
                        </div>
                    </div>

                    <div className="settings-actions">
                        <Button size="sm" onClick={saveHotkey} disabled={saving}>
                            Update Shortcuts
                        </Button>
                    </div>
                </div>
            </section>

            {/* Data */}
            <section className="admin-section">
                <h4 className="admin-section-title">Data</h4>
                <div className="settings-card settings-data-card">
                    <div className="settings-row">
                        <div className="settings-row-info">
                            <span className="settings-row-title">Database Location</span>
                            <code className="settings-path-value" title={databasePath}>{databasePath}</code>
                        </div>
                        <Button size="sm" variant="secondary" onClick={copyDatabasePath} disabled={!databasePath}>
                            <Copy size={13} />
                            Copy
                        </Button>
                    </div>
                </div>
            </section>
        </div>
    );
}

export default SettingsPage;
