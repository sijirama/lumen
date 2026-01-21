//INFO: Main Window component - Minimal header with tiny nav links
//NOTE: Clean, minimalistic design with navigation in header

import { useState } from 'react';
import { Settings, Plug } from 'lucide-react';
import Dashboard from '../pages/Dashboard';
import SettingsPage from '../pages/Settings';
import IntegrationsPage from '../pages/Integrations';

//INFO: Props interface for MainWindow
interface MainWindowProps {
    userName: string | null;
}

//INFO: Available pages
type PageType = 'dashboard' | 'settings' | 'integrations';

function MainWindow({ userName }: MainWindowProps) {
    //INFO: Track which page is currently active
    const [activePage, setActivePage] = useState<PageType>('dashboard');

    //INFO: Render the current page
    const renderPage = () => {
        switch (activePage) {
            case 'dashboard':
                return <Dashboard userName={userName || 'there'} />;
            case 'settings':
                return <SettingsPage />;
            case 'integrations':
                return <IntegrationsPage />;
            default:
                return <Dashboard userName={userName || 'there'} />;
        }
    };

    return (
        <div className="app-layout with-app-bg">
            {/* INFO: Minimal header with logo and tiny nav links */}
            <header className="app-header">
                <div className="header-container">
                    <div className="app-logo">
                        <div className="app-logo-icon" style={{ background: 'transparent' }}>
                            <img src="/logo.png" alt="Lumen Logo" style={{ width: '100%', height: '100%', objectFit: 'contain' }} />
                        </div>
                        <span>Lumen</span>
                    </div>

                    <nav className="app-nav">
                        <button
                            className={`nav-link ${activePage === 'dashboard' ? 'active' : ''}`}
                            onClick={() => setActivePage('dashboard')}
                        >
                            Home
                        </button>
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

            {/* INFO: Main content */}
            <main className="app-main">
                {renderPage()}
            </main>
        </div>
    );
}

export default MainWindow;
