//INFO: Main Window component - Minimal header with tiny nav links
//NOTE: The chat overlay is the primary UX; this window is the admin panel for
//      settings and integrations. No dashboard / digest page anymore.

import { useState } from 'react';
import { Settings, Plug } from 'lucide-react';
import SettingsPage from '../pages/Settings';
import IntegrationsPage from '../pages/Integrations';

interface MainWindowProps {
    userName: string | null;
}

type PageType = 'settings' | 'integrations';

function MainWindow({ userName: _userName }: MainWindowProps) {
    const [activePage, setActivePage] = useState<PageType>('settings');

    const renderPage = () => {
        switch (activePage) {
            case 'settings':
                return <SettingsPage />;
            case 'integrations':
                return <IntegrationsPage />;
            default:
                return <SettingsPage />;
        }
    };

    return (
        <div className="app-layout with-app-bg">
            <header className="app-header">
                <div className="header-container">
                    <div className="app-logo">
                        <div className="app-logo-icon" style={{ background: 'transparent' }}>
                            <img src="/logo.png" alt="Lumen Logo" style={{ width: '100%', height: '100%', objectFit: 'contain' }} />
                        </div>
                    </div>

                    <nav className="app-nav">
                        <button
                            className={`nav-link ${activePage === 'settings' ? 'active' : ''}`}
                            onClick={() => setActivePage('settings')}
                        >
                            <Settings size={14} style={{ marginRight: '4px' }} />
                            Settings
                        </button>
                        <button
                            className={`nav-link ${activePage === 'integrations' ? 'active' : ''}`}
                            onClick={() => setActivePage('integrations')}
                        >
                            <Plug size={14} style={{ marginRight: '4px' }} />
                            Integrations
                        </button>
                    </nav>
                </div>
            </header>

            <main className="app-main">
                {renderPage()}
            </main>
        </div>
    );
}

export default MainWindow;
