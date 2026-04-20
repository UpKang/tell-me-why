/**
 * 应用状态管理
 *
 * 使用 Svelte 传统的 writable store 来管理应用的全局状态
 * 注意：$state rune 只能在 .svelte 和 .svelte.js/ts 文件中使用
 */

import { writable } from 'svelte/store';

/**
 * 设置模态框状态管理
 *
 * 返回一个包含状态和方法的对象
 */
export function createSettingsStore() {
  /**
   * 模态框是否打开的状态
   * 使用 writable store 来管理
   */
  const isOpen = writable(false);

  /**
   * 打开设置模态框
   */
  function open(): void {
    isOpen.set(true);
  }

  /**
   * 关闭设置模态框
   */
  function close(): void {
    isOpen.set(false);
  }

  /**
   * 切换设置模态框状态
   */
  function toggle(): void {
    isOpen.update((value) => !value);
  }

  return {
    isOpen,
    open,
    close,
    toggle,
  };
}

/**
 * Toast 提示状态管理
 *
 * 用于显示临时的提示消息
 */
export function createToastStore() {
  /**
   * 提示消息内容
   */
  const message = writable('');

  /**
   * 提示类型：success、error、info
   */
  const type = writable<'success' | 'error' | 'info' | ''>('');

  /**
   * 是否可见
   */
  const isVisible = writable(false);

  /**
   * 显示 Toast 提示
   *
   * @param msg - 提示消息内容
   * @param t - 提示类型
   * @param duration - 显示时长（毫秒），默认 3000
   */
  function show(msg: string, t: 'success' | 'error' | 'info', duration = 3000): void {
    message.set(msg);
    type.set(t);
    isVisible.set(true);

    // 自动隐藏
    setTimeout(() => {
      hide();
    }, duration);
  }

  /**
   * 隐藏 Toast 提示
   */
  function hide(): void {
    isVisible.set(false);
    message.set('');
    type.set('');
  }

  return {
    message,
    type,
    isVisible,
    show,
    hide,
  };
}