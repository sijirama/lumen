//INFO: Main App component - Routes between different windows and pages
//NOTE: Detects which window is open (main or overlay) and renders appropriate content

import { useEffect, useState } from 'react';
import { useSearchParams } from 'react-router-dom';
import { invoke } from '@tauri-apps/api/core';

//INFO: Import window components
import MainWindow from './windows/MainWindow';
import OverlayWindow from './windows/OverlayWindow';
import WidgetWindow from './windows/WidgetWindow';
import SetupWizard from './components/setup/SetupWizard';

//INFO: Type for setup status response from backend
interface SetupStatusResponse {
  setup_complete: boolean;
  user_profile: {
    display_name: string;
    location: string | null;
    theme: string;
  } | null;
}

function App() {
  //INFO: Track whether the app is still loading initial state
  const [isLoading, setIsLoading] = useState(true);
  //INFO: Track whether setup wizard has been completed
  const [setupComplete, setSetupComplete] = useState(false);
  //INFO: Store the user's name for greeting
  const [userName, setUserName] = useState<string | null>(null);
  //INFO: Get URL search params to detect overlay window
  const [searchParams] = useSearchParams();

  //INFO: Check which window is being rendered
  const isOverlay = searchParams.get('window') === 'overlay';
  const isWidget = searchParams.get('window') === 'widget';

  //INFO: Check setup status on app load
  useEffect(() => {
    async function checkSetupStatus() {
      try {
        //INFO: Call backend to check if setup wizard was completed
        const status = await invoke<SetupStatusResponse>('check_setup_status');
        setSetupComplete(status.setup_complete);
        setUserName(status.user_profile?.display_name || null);
      } catch (error) {
        console.error('Failed to check setup status:', error);
        //INFO: Default to showing setup if check fails
        setSetupComplete(false);
      } finally {
        setIsLoading(false);
      }
    }

    checkSetupStatus();
  }, []);

  //INFO: Handler for when setup is completed
  const handleSetupComplete = (name: string) => {
    setSetupComplete(true);
    setUserName(name);
  };

  //INFO: Show loading state while checking setup status
  if (isLoading) {
    return (
      <div className="loading-container" style={{
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        minHeight: '100vh',
        background: 'var(--color-bg-primary)'
      }}>
        <div className="loading-spinner" />
      </div>
    );
  }

  //INFO: If this is the overlay window, always show overlay
  if (isOverlay) {
    return <OverlayWindow />;
  }

  //INFO: If this is the widget window, always show widget
  if (isWidget) {
    return <WidgetWindow />;
  }

  //INFO: If setup is not complete, show setup wizard
  if (!setupComplete) {
    return <SetupWizard onComplete={handleSetupComplete} />;
  }

  //INFO: Otherwise show the main application
  return <MainWindow userName={userName} />;
}

export default App;
