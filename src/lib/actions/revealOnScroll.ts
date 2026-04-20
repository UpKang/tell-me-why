/**
 * revealOnScroll - IntersectionObserver Svelte action
 *
 * 用法：
 *   <section use:revealOnScroll>...</section>
 *
 * 效果：
 * - 元素默认态由 CSS `.reveal-pending { opacity: 0; transform: translateY(18px); }` 控制
 * - 当元素进入视口 ≥15% 时，动作把它切换到 `.reveal-shown`，CSS 补动画
 * - 尊重 prefers-reduced-motion：直接立即可见，不做位移
 *
 * 为什么不写在每个组件里：reveal 逻辑高度可复用，集中到一个 action
 * 既避免重复代码，又保证行为一致（同样的阈值、同样的 reduced-motion 处理）。
 */

/**
 * 检测用户是否偏好减少动态效果
 * 每次调用都重新查询——便于系统级偏好动态切换时自动生效
 */
function prefersReducedMotion(): boolean {
  // 非浏览器环境（SSR / 单元测试）默认不降级
  if (typeof window === 'undefined' || !window.matchMedia) return false;
  return window.matchMedia('(prefers-reduced-motion: reduce)').matches;
}

/**
 * Svelte action：把元素变成"滚动到视口才揭示"
 *
 * @param node - 被 use:revealOnScroll 修饰的 DOM 节点
 * @returns destroy 回调，Svelte 会在元素卸载时自动调用
 */
export function revealOnScroll(node: HTMLElement): { destroy(): void } {
  // reduced-motion 场景直接显示，不启用 observer
  if (prefersReducedMotion()) {
    node.classList.add('reveal-shown');
    return {
      destroy() {
        // 无需清理
      },
    };
  }

  // 非 reduced-motion：加 pending 类，进入视口后切到 shown
  node.classList.add('reveal-pending');

  // 如果运行环境没有 IntersectionObserver（老浏览器、测试环境），直接显示
  if (typeof IntersectionObserver === 'undefined') {
    node.classList.remove('reveal-pending');
    node.classList.add('reveal-shown');
    return { destroy() {} };
  }

  const observer = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (entry.isIntersecting) {
          node.classList.remove('reveal-pending');
          node.classList.add('reveal-shown');
          // 一次性揭示后即可停止观察，节省开销
          observer.unobserve(node);
        }
      }
    },
    {
      // 15% 以上进入视口才触发，避免顶部擦边时就闪现
      threshold: 0.15,
      // 视口上下各 50px 的"预揭示"边距，让动画不会在最后一刻才开始
      rootMargin: '0px 0px -50px 0px',
    },
  );

  observer.observe(node);

  return {
    destroy() {
      observer.disconnect();
    },
  };
}
