//INFO: Setup Wizard - Minimal design
//NOTE: Clean, simple onboarding flow

import { useState } from 'react';
import { Sparkles, ChevronRight, ChevronLeft, Check, User, Keyboard, Key, Plug } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';

interface SetupWizardProps {
    onComplete: (name: string) => void;
}

type Step = 'welcome' | 'user' | 'hotkey' | 'api' | 'integrations' | 'done';
const STEPS: Step[] = ['welcome', 'user', 'hotkey', 'api', 'integrations', 'done'];

function SetupWizard({ onComplete }: SetupWizardProps) {
    const [step, setStep] = useState<Step>('welcome');
    const [displayName, setDisplayName] = useState('');
    const [hotkeyMods, setHotkeyMods] = useState<string[]>(['Super']);
    const [hotkeyKey, setHotkeyKey] = useState('L');
    const [apiKey, setApiKey] = useState('');
    const [testingApi, setTestingApi] = useState(false);
    const [apiValid, setApiValid] = useState<boolean | null>(null);
    const [vaultPath, setVaultPath] = useState<string | null>(null);
    const [saving, setSaving] = useState(false);
    const [error, setError] = useState<string | null>(null);

    const stepIndex = STEPS.indexOf(step);

    function next() {
        const i = stepIndex + 1;
        if (i < STEPS.length) setStep(STEPS[i]);
        setError(null);
    }

    function prev() {
        const i = stepIndex - 1;
        if (i >= 0) setStep(STEPS[i]);
        setError(null);
    }

    function toggleMod(mod: string) {
        if (hotkeyMods.includes(mod)) setHotkeyMods(hotkeyMods.filter(m => m !== mod));
        else setHotkeyMods([...hotkeyMods, mod]);
    }

    async function testApi() {
        if (!apiKey.trim()) return;
        setTestingApi(true);
        setApiValid(null);
        try {
            const valid = await invoke<boolean>('test_gemini_api_key', { apiKey });
            setApiValid(valid);
        } catch {
            setApiValid(false);
        } finally {
            setTestingApi(false);
        }
    }

    async function selectVault() {
        try {
            const path = await open({ directory: true, title: 'Select Obsidian Vault' });
            if (path && typeof path === 'string') setVaultPath(path);
        } catch (err) {
            console.error(err);
        }
    }

    async function finish() {
        setSaving(true);
        setError(null);
        try {
            await invoke('setup_save_profile', { request: { display_name: displayName, location: null, theme: 'light' } });
            await invoke('setup_save_hotkey', { request: { modifier_keys: hotkeyMods, key: hotkeyKey } });
            if (apiKey.trim()) {
                await invoke('setup_save_api_key', { request: { provider: 'gemini', api_key: apiKey } });
            }
            if (vaultPath) {
                await invoke('setup_save_integration', { request: { name: 'obsidian', enabled: true, config: JSON.stringify({ vault_path: vaultPath }) } });
            }
            await invoke('complete_setup');
            onComplete(displayName);
        } catch (err) {
            setError(`Setup failed: ${err}`);
        } finally {
            setSaving(false);
        }
    }

    function canProceed(): boolean {
        if (step === 'user') return displayName.trim().length > 0;
        if (step === 'hotkey') return hotkeyMods.length > 0 && hotkeyKey.length > 0;
        return true;
    }

    function renderStep() {
        switch (step) {
            case 'welcome':
                return (
                    <div className="setup-header">
                        <div className="setup-icon"><Sparkles size={24} /></div>
                        <h2 className="setup-title">Welcome to Lumen</h2>
                        <p className="setup-subtitle">Your minimal AI assistant. Let's set you up.</p>
                    </div>
                );

            case 'user':
                return (
                    <>
                        <div className="setup-header">
                            <div className="setup-icon"><User size={24} /></div>
                            <h2 className="setup-title">What's your name?</h2>
                            <p className="setup-subtitle">This is how I'll greet you.</p>
                        </div>
                        <div className="setup-content">
                            <input
                                type="text"
                                className="input"
                                value={displayName}
                                onChange={(e) => setDisplayName(e.target.value)}
                                placeholder="Your name"
                                autoFocus
                            />
                        </div>
                    </>
                );

            case 'hotkey':
                return (
                    <>
                        <div className="setup-header">
                            <div className="setup-icon"><Keyboard size={24} /></div>
                            <h2 className="setup-title">Choose a hotkey</h2>
                            <p className="setup-subtitle">Press this to summon me.</p>
                        </div>
                        <div className="setup-content">
                            <div style={{ marginBottom: 'var(--spacing-4)' }}>
                                <label className="input-label" style={{ marginBottom: 'var(--spacing-2)', display: 'block' }}>Modifiers</label>
                                <div style={{ display: 'flex', gap: 'var(--spacing-2)', flexWrap: 'wrap' }}>
                                    {['Super', 'Ctrl', 'Alt', 'Shift'].map(mod => (
                                        <button key={mod} className={`btn btn-sm ${hotkeyMods.includes(mod) ? 'btn-primary' : ''}`} onClick={() => toggleMod(mod)}>
                                            {mod}
                                        </button>
                                    ))}
                                </div>
                            </div>
                            <div className="input-group">
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
                            <div style={{ marginTop: 'var(--spacing-4)', textAlign: 'center' }}>
                                {hotkeyMods.map(m => <kbd key={m} className="kbd" style={{ marginRight: '4px' }}>{m}</kbd>)} + <kbd className="kbd">{hotkeyKey}</kbd>
                            </div>
                        </div>
                    </>
                );

            case 'api':
                return (
                    <>
                        <div className="setup-header">
                            <div className="setup-icon"><Key size={24} /></div>
                            <h2 className="setup-title">Gemini API Key</h2>
                            <p className="setup-subtitle">I use Google's Gemini AI.</p>
                        </div>
                        <div className="setup-content">
                            <div className="input-group" style={{ marginBottom: 'var(--spacing-3)' }}>
                                <input
                                    type="password"
                                    className="input"
                                    value={apiKey}
                                    onChange={(e) => { setApiKey(e.target.value); setApiValid(null); }}
                                    placeholder="Enter API key"
                                />
                            </div>
                            <div style={{ display: 'flex', gap: 'var(--spacing-2)', alignItems: 'center', marginBottom: 'var(--spacing-3)' }}>
                                <button className="btn btn-sm" onClick={testApi} disabled={!apiKey.trim() || testingApi}>
                                    {testingApi ? 'Testing...' : 'Test'}
                                </button>
                                {apiValid === true && <span className="badge badge-success"><Check size={12} /> Valid</span>}
                                {apiValid === false && <span className="badge badge-error">Invalid</span>}
                            </div>
                            <p style={{ fontSize: 'var(--font-size-xs)', color: 'var(--color-text-tertiary)' }}>
                                Get your key from <a href="https://aistudio.google.com/apikey" target="_blank" rel="noreferrer">Google AI Studio</a>. You can skip and add later.
                            </p>
                        </div>
                    </>
                );

            case 'integrations':
                return (
                    <>
                        <div className="setup-header">
                            <div className="setup-icon"><Plug size={24} /></div>
                            <h2 className="setup-title">Integrations</h2>
                            <p className="setup-subtitle">Optional. Connect later in settings.</p>
                        </div>
                        <div className="setup-content">
                            <div className="integration-card" style={{ marginBottom: 'var(--spacing-3)' }}>
                                <div className="integration-icon" style={{ background: '#f3e8ff' }}>📝</div>
                                <div className="integration-info">
                                    <div className="integration-name">Obsidian</div>
                                    <div className="integration-status">
                                        {vaultPath ? (
                                            <span style={{ fontSize: 'var(--font-size-xs)', color: 'var(--color-success)' }}>Selected</span>
                                        ) : 'Not connected'}
                                    </div>
                                </div>
                                <button className={`btn btn-sm ${vaultPath ? '' : 'btn-primary'}`} onClick={selectVault}>
                                    {vaultPath ? 'Change' : 'Select'}
                                </button>
                            </div>
                        </div>
                    </>
                );

            case 'done':
                return (
                    <div className="setup-header">
                        <div className="setup-icon" style={{ background: 'var(--color-success)' }}><Check size={24} /></div>
                        <h2 className="setup-title">All set!</h2>
                        <p className="setup-subtitle">
                            Press {hotkeyMods.map(m => <kbd key={m} className="kbd" style={{ margin: '0 2px' }}>{m}</kbd>)} + <kbd className="kbd">{hotkeyKey}</kbd> to chat.
                        </p>
                    </div>
                );
        }
    }

    return (
        <div className="setup-container">
            <div className="setup-card">
                {/* Progress */}
                <div className="setup-progress">
                    {STEPS.map((s, i) => (
                        <div key={s} className={`setup-dot ${i === stepIndex ? 'active' : i < stepIndex ? 'completed' : ''}`} />
                    ))}
                </div>

                {renderStep()}

                {error && <div className="error-message">{error}</div>}

                {/* Footer */}
                <div className="setup-footer">
                    {stepIndex > 0 && step !== 'done' ? (
                        <button className="btn" onClick={prev}><ChevronLeft size={16} /> Back</button>
                    ) : <div />}

                    {step === 'done' ? (
                        <button className="btn btn-primary" onClick={finish} disabled={saving} style={{ flex: 1 }}>
                            {saving ? 'Starting...' : 'Get Started'}
                        </button>
                    ) : (
                        <button className="btn btn-primary" onClick={next} disabled={!canProceed()}>
                            {step === 'integrations' ? 'Finish' : 'Continue'} <ChevronRight size={16} />
                        </button>
                    )}
                </div>
            </div>
        </div>
    );
}

export default SetupWizard;
