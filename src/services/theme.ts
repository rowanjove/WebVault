export type ThemeMode = 'light' | 'dark';

const THEME_KEY = 'webvault_theme';

export const getStoredTheme = (): ThemeMode => {
  if (typeof window === 'undefined') return 'light';
  const saved = localStorage.getItem(THEME_KEY);
  if (saved === 'dark' || saved === 'light') {
    return saved;
  }
  // 默认浅色（白灰风格）
  return 'light';
};

export const applyTheme = (theme: ThemeMode): void => {
  if (typeof document === 'undefined') return;
  const root = document.documentElement;
  if (theme === 'dark') {
    root.classList.add('dark');
  } else {
    root.classList.remove('dark');
  }
  localStorage.setItem(THEME_KEY, theme);
  window.dispatchEvent(new CustomEvent('webvault-theme-change', { detail: theme }));
};

export const toggleTheme = (): ThemeMode => {
  const current = getStoredTheme();
  const next: ThemeMode = current === 'dark' ? 'light' : 'dark';
  applyTheme(next);
  return next;
};
