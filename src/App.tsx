import React, { useEffect } from 'react';
import { Sidebar } from './components/layout/Sidebar';
import { Dashboard } from './pages/Dashboard';
import { Sites } from './pages/Sites';
import { TimeMachine } from './pages/TimeMachine';
import { Search } from './pages/Search';
import { Monitor } from './pages/Monitor';
import { Tasks } from './pages/Tasks';
import { Wayback } from './pages/Wayback';
import { Settings } from './pages/Settings';
import { AddSiteModal } from './components/modals/AddSiteModal';
import { OnboardingModal } from './components/modals/OnboardingModal';
import { ToastContainer } from './components/ui/Toast';
import { useAppStore } from './stores/useAppStore';
import { api } from './services/tauri';

export const App: React.FC = () => {
  const { currentTab, setSites, setStats, setJobs, theme, setIsOnboardingOpen } = useAppStore();

  useEffect(() => {
    // Initial system load
    api.getSystemStats().then(setStats).catch(console.error);
    api.listSites().then(setSites).catch(console.error);
    api.listJobs().then(setJobs).catch(console.error);

    // Auto-open interactive tour if first time
    if (typeof window !== 'undefined' && localStorage.getItem('webvault_tour_completed') !== 'true') {
      setIsOnboardingOpen(true);
    }
  }, []);

  useEffect(() => {
    if (theme === 'dark') {
      document.documentElement.classList.add('dark');
    } else {
      document.documentElement.classList.remove('dark');
    }
  }, [theme]);

  return (
    <div className="flex w-screen h-screen overflow-hidden bg-neutral-950 font-sans text-neutral-100 select-none transition-colors duration-150">
      {/* Sidebar Navigation */}
      <Sidebar />

      {/* Dynamic Content View */}
      <main className="flex-1 h-screen flex flex-col overflow-hidden">
        {currentTab === 'dashboard' && <Dashboard />}
        {currentTab === 'sites' && <Sites />}
        {(currentTab === 'timemachine' ||
          currentTab === 'timeline' ||
          currentTab === 'replay' ||
          currentTab === 'diff') && <TimeMachine />}
        {currentTab === 'search' && <Search />}
        {currentTab === 'monitor' && <Monitor />}
        {currentTab === 'tasks' && <Tasks />}
        {currentTab === 'wayback' && <Wayback />}
        {currentTab === 'settings' && <Settings />}
      </main>

      {/* Global Modals & Notifications */}
      <AddSiteModal />
      <OnboardingModal />
      <ToastContainer />
    </div>
  );
};

export default App;
