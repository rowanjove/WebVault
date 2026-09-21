import { create } from 'zustand';
import type { Site, SystemStats, CrawlJob } from '../types';

export type TabId = 
  | 'dashboard'
  | 'sites'
  | 'timemachine'
  | 'timeline'
  | 'replay'
  | 'search'
  | 'diff'
  | 'monitor'
  | 'tasks'
  | 'wayback'
  | 'settings';

export type TimeMachineMode = 'replay' | 'diff' | 'resources';

interface AppState {
  currentTab: TabId;
  setCurrentTab: (tab: TabId) => void;
  
  // TimeMachine integrated mode
  timeMachineMode: TimeMachineMode;
  setTimeMachineMode: (mode: TimeMachineMode) => void;
  openTimeMachine: (params?: {
    siteId?: string | null;
    pageId?: string | null;
    captureId?: string | null;
    mode?: TimeMachineMode;
  }) => void;

  // Selected context
  selectedSiteId: string | null;
  setSelectedSiteId: (id: string | null) => void;
  
  selectedPageId: string | null;
  setSelectedPageId: (id: string | null) => void;
  
  selectedCaptureId: string | null;
  setSelectedCaptureId: (id: string | null) => void;
  
  // Diff pair
  diffOldCaptureId: string | null;
  diffNewCaptureId: string | null;
  setDiffPair: (oldId: string, newId: string, ctx?: { siteId?: string | null; pageId?: string | null }) => void;

  // Cached sites & stats
  sites: Site[];
  setSites: (sites: Site[]) => void;
  stats: SystemStats | null;
  setStats: (stats: SystemStats | null) => void;
  jobs: CrawlJob[];
  setJobs: (jobs: CrawlJob[]) => void;

  // Quick action modal
  isAddSiteOpen: boolean;
  setIsAddSiteOpen: (open: boolean) => void;

  // Onboarding tutorial modal
  isOnboardingOpen: boolean;
  setIsOnboardingOpen: (open: boolean) => void;

  // Theme support: 'light' (default) | 'dark'
  theme: 'light' | 'dark';
  setTheme: (theme: 'light' | 'dark') => void;
  toggleTheme: () => void;
}

export const useAppStore = create<AppState>((set) => ({
  currentTab: 'dashboard',
  setCurrentTab: (tab) => {
    // Gracefully map legacy tab IDs to timemachine
    if (tab === 'timeline' || tab === 'replay') {
      set({ currentTab: 'timemachine', timeMachineMode: 'replay' });
    } else if (tab === 'diff') {
      set({ currentTab: 'timemachine', timeMachineMode: 'diff' });
    } else {
      set({ currentTab: tab });
    }
  },

  timeMachineMode: 'replay',
  setTimeMachineMode: (mode) => set({ timeMachineMode: mode }),

  openTimeMachine: (params) => {
    set((state) => ({
      currentTab: 'timemachine',
      timeMachineMode: params?.mode || 'replay',
      selectedSiteId: params?.siteId !== undefined ? params.siteId : state.selectedSiteId,
      selectedPageId: params?.pageId !== undefined ? params.pageId : state.selectedPageId,
      selectedCaptureId: params?.captureId !== undefined ? params.captureId : state.selectedCaptureId,
    }));
  },

  selectedSiteId: null,
  setSelectedSiteId: (id) => set({ selectedSiteId: id }),

  selectedPageId: null,
  setSelectedPageId: (id) => set({ selectedPageId: id }),

  selectedCaptureId: null,
  setSelectedCaptureId: (id) => set({ selectedCaptureId: id }),

  diffOldCaptureId: null,
  diffNewCaptureId: null,
  setDiffPair: (oldId, newId, ctx) => set((state) => ({
    diffOldCaptureId: oldId,
    diffNewCaptureId: newId,
    currentTab: 'timemachine',
    timeMachineMode: 'diff',
    selectedSiteId: ctx?.siteId !== undefined ? ctx.siteId : state.selectedSiteId,
    selectedPageId: ctx?.pageId !== undefined ? ctx.pageId : state.selectedPageId,
    selectedCaptureId: newId,
  })),

  sites: [],
  setSites: (sites) => set({ sites }),
  stats: null,
  setStats: (stats) => set({ stats }),
  jobs: [],
  setJobs: (jobs) => set({ jobs }),

  isAddSiteOpen: false,
  setIsAddSiteOpen: (open) => set({ isAddSiteOpen: open }),

  isOnboardingOpen: false,
  setIsOnboardingOpen: (open) => set({ isOnboardingOpen: open }),

  theme: typeof window !== 'undefined' && (localStorage.getItem('webvault_theme') === 'dark') ? 'dark' : 'light',
  setTheme: (newTheme) => {
    if (typeof document !== 'undefined') {
      if (newTheme === 'dark') {
        document.documentElement.classList.add('dark');
      } else {
        document.documentElement.classList.remove('dark');
      }
      localStorage.setItem('webvault_theme', newTheme);
    }
    set({ theme: newTheme });
  },
  toggleTheme: () => {
    set((state) => {
      const nextTheme = state.theme === 'dark' ? 'light' : 'dark';
      if (typeof document !== 'undefined') {
        if (nextTheme === 'dark') {
          document.documentElement.classList.add('dark');
        } else {
          document.documentElement.classList.remove('dark');
        }
        localStorage.setItem('webvault_theme', nextTheme);
      }
      return { theme: nextTheme };
    });
  },
}));
