/**
 * 主题 store：支持三态主题切换（浅色 / 暗黑 / 跟随系统）
 *
 * 设计说明：
 * - 用户选择 ('light' | 'dark' | 'system') 持久化到 localStorage
 * - 实际生效的主题 ('light' | 'dark') 写到 <html data-theme="..."> 属性上，
 *   由 CSS 变量通过 [data-theme="dark"] 选择器切换配色
 * - system 模式下监听 prefers-color-scheme，系统切换时自动跟随
 * - 初始化必须在首屏渲染前同步执行，避免白屏闪烁 (FOUC)
 */

import { writable } from 'svelte/store';

/** 用户可选择的主题模式（三态） */
export type ThemeMode = 'light' | 'dark' | 'system';

/** 实际应用到 DOM 的主题（两态，system 会解析成这两者之一） */
export type ResolvedTheme = 'light' | 'dark';

/** localStorage 中保存用户选择的 key */
const STORAGE_KEY = 'tellmewhy-theme';

/** <html> 上写入的属性名，CSS 通过 [data-theme="dark"] 匹配 */
const DATA_ATTR = 'data-theme';

/**
 * 从 localStorage 读取用户保存的主题模式
 * 如果没有保存过或值非法，返回默认值 'system'
 */
function readStoredMode(): ThemeMode {
  if (typeof localStorage === 'undefined') return 'system';
  const stored = localStorage.getItem(STORAGE_KEY);
  if (stored === 'light' || stored === 'dark' || stored === 'system') {
    return stored;
  }
  return 'system';
}

/**
 * 查询系统当前的配色偏好
 * 返回 'dark' 表示系统为暗黑模式，否则为 'light'
 */
function getSystemTheme(): ResolvedTheme {
  if (typeof window === 'undefined' || !window.matchMedia) return 'light';
  return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
}

/**
 * 根据用户选择的模式，计算最终应该应用的主题
 * - light / dark：直接返回
 * - system：查询系统偏好
 */
export function resolveTheme(mode: ThemeMode): ResolvedTheme {
  if (mode === 'system') return getSystemTheme();
  return mode;
}

/**
 * 将已解析出的主题写入 <html data-theme="..."> 属性
 * CSS 变量通过该属性切换配色
 */
function applyTheme(resolved: ResolvedTheme): void {
  if (typeof document === 'undefined') return;
  document.documentElement.setAttribute(DATA_ATTR, resolved);
}

/**
 * 主题 store：暴露用户选择的 mode
 * 初始化时读取 localStorage，缺省为 'system'
 */
export const themeMode = writable<ThemeMode>(readStoredMode());

/**
 * 已解析出的实际主题 store（只读给外部使用，供 UI 显示当前态）
 * 与 themeMode 同步更新
 */
export const resolvedTheme = writable<ResolvedTheme>(resolveTheme(readStoredMode()));

/**
 * 切换主题模式到指定值
 * 副作用：写 localStorage、更新 <html> 属性、同步 resolvedTheme store
 */
export function setThemeMode(mode: ThemeMode): void {
  themeMode.set(mode);
  if (typeof localStorage !== 'undefined') {
    localStorage.setItem(STORAGE_KEY, mode);
  }
  const resolved = resolveTheme(mode);
  applyTheme(resolved);
  resolvedTheme.set(resolved);
}

/**
 * 在应用启动时调用一次：
 * 1. 立即应用当前主题到 DOM（防止首屏闪烁）
 * 2. 监听系统配色变化，当用户处于 system 模式时自动跟随
 *
 * 注意：此函数应该在 main.ts 顶部同步执行，避免用户看到错误配色的闪烁
 */
export function initTheme(): void {
  const mode = readStoredMode();
  const resolved = resolveTheme(mode);
  applyTheme(resolved);
  resolvedTheme.set(resolved);

  // 监听系统主题变化：仅在 system 模式下响应
  if (typeof window !== 'undefined' && window.matchMedia) {
    const mql = window.matchMedia('(prefers-color-scheme: dark)');
    const handler = (): void => {
      // 读取用户当前的选择，仅在 system 模式下才跟随系统变化
      const currentMode = readStoredMode();
      if (currentMode === 'system') {
        const next = getSystemTheme();
        applyTheme(next);
        resolvedTheme.set(next);
      }
    };
    // 兼容老浏览器：优先使用 addEventListener，不支持则回退 addListener
    if (mql.addEventListener) {
      mql.addEventListener('change', handler);
    } else if ((mql as MediaQueryList & { addListener?: (cb: () => void) => void }).addListener) {
      (mql as MediaQueryList & { addListener: (cb: () => void) => void }).addListener(handler);
    }
  }
}
