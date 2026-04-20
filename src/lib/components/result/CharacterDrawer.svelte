<script lang="ts">
  /**
   * CharacterDrawer - 人物档案抽屉
   *
   * 从右侧滑入的完整人物画像面板。由 ResultView 控制打开/关闭。
   * 内部展示：
   * - 头像 + 姓名 + 角色
   * - 人物描述
   * - 动机列表（带可信度）
   *
   * 设计理念：
   * - 把"画像"和"结果"合并在同一页面上（吃瓜连贯性）
   * - 关闭方式：点击遮罩 / 点关闭按钮 / Esc
   */

  import type { CharacterProfile } from '../CharacterProfilePanel.svelte';

  interface Props {
    character: CharacterProfile | null;
    /** 关闭回调 */
    onClose: () => void;
  }

  const { character, onClose }: Props = $props();

  /** 监听 ESC：必须在 mount 后；Svelte 5 里用 $effect 更干净 */
  $effect(() => {
    if (!character) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  });
</script>

{#if character}
  <!-- 遮罩：点击关闭 -->
  <div
    class="drawer-overlay"
    onclick={onClose}
    onkeydown={(e) => e.key === 'Escape' && onClose()}
    role="presentation"
  ></div>

  <!-- 抽屉主体：右侧滑入（用 div 而非 aside 以允许 role="dialog"） -->
  <div
    class="char-drawer"
    role="dialog"
    aria-modal="true"
    aria-labelledby="drawer-name"
    tabindex="-1"
  >
    <header class="drawer-head">
      <div class="drawer-avatar">
        <span>{character.name.charAt(0)}</span>
      </div>
      <div class="drawer-meta">
        <h2 id="drawer-name" class="drawer-name">{character.name}</h2>
        <span class="drawer-role">{character.role}</span>
      </div>
      <button
        type="button"
        class="drawer-close"
        onclick={onClose}
        aria-label="关闭人物档案"
      >
        ✕
      </button>
    </header>

    <div class="drawer-body">
      <!-- 人物描述 -->
      <section class="drawer-section">
        <h3 class="section-title">人物画像</h3>
        <p class="drawer-desc">{character.description || '（暂无详细描述）'}</p>
      </section>

      <!-- 动机列表 -->
      {#if character.motivations && character.motivations.length > 0}
        <section class="drawer-section">
          <h3 class="section-title">可能动机</h3>
          <ul class="motiv-list" role="list">
            {#each character.motivations as m (m.id)}
              <li class="motiv-item">
                <div class="motiv-head">
                  <span class="motiv-content">{m.content}</span>
                  <span class="motiv-conf" aria-label="可信度 {m.confidence}">
                    {m.confidence}%
                  </span>
                </div>
                {#if m.sourceHint}
                  <div class="motiv-source">来源提示：{m.sourceHint}</div>
                {/if}
              </li>
            {/each}
          </ul>
        </section>
      {/if}
    </div>
  </div>
{/if}

<style>
  .drawer-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.55);
    backdrop-filter: blur(2px);
    z-index: 90;
    animation: fade-in 180ms ease;
  }

  .char-drawer {
    position: fixed;
    top: 0;
    right: 0;
    height: 100vh;
    width: min(420px, 92vw);
    background: var(--bg-secondary, #0e0e12);
    border-left: 1px solid var(--border, rgba(255, 255, 255, 0.1));
    box-shadow: -12px 0 40px rgba(0, 0, 0, 0.5);
    z-index: 91;
    display: flex;
    flex-direction: column;
    animation: slide-in 220ms cubic-bezier(0.16, 1, 0.3, 1);
    overflow: hidden;
  }

  @keyframes fade-in {
    from { opacity: 0; }
    to { opacity: 1; }
  }

  @keyframes slide-in {
    from { transform: translateX(24px); opacity: 0; }
    to { transform: translateX(0); opacity: 1; }
  }

  .drawer-head {
    display: flex;
    align-items: center;
    gap: 0.85rem;
    padding: 1rem 1.2rem;
    border-bottom: 1px solid var(--border, rgba(255, 255, 255, 0.08));
    background: var(--bg-card, #17171c);
  }

  .drawer-avatar {
    width: 56px;
    height: 56px;
    border-radius: 50%;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    background: linear-gradient(135deg, var(--accent-hot, #e6534f), var(--accent-gold, #d4a851));
    color: #fff;
    font-size: 1.4rem;
    font-weight: 700;
    font-family: 'Source Serif Pro', 'Noto Serif SC', Georgia, serif;
    flex-shrink: 0;
  }

  .drawer-meta {
    flex: 1;
    min-width: 0;
  }

  .drawer-name {
    margin: 0 0 0.15rem;
    font-family: 'Source Serif Pro', 'Noto Serif SC', Georgia, serif;
    font-size: 1.2rem;
    color: var(--text-primary, #f6f6f3);
    letter-spacing: -0.01em;
  }

  .drawer-role {
    display: inline-block;
    font-size: 0.7rem;
    letter-spacing: 0.2em;
    text-transform: uppercase;
    color: var(--accent-hot, #e6534f);
    font-family: 'JetBrains Mono', ui-monospace, monospace;
  }

  .drawer-close {
    background: transparent;
    border: 1px solid var(--border, rgba(255, 255, 255, 0.2));
    border-radius: 50%;
    width: 32px;
    height: 32px;
    color: var(--text-secondary, #9a9a94);
    cursor: pointer;
    transition: color 120ms ease, border-color 120ms ease;
    flex-shrink: 0;
  }
  .drawer-close:hover,
  .drawer-close:focus-visible {
    color: var(--accent-hot, #e6534f);
    border-color: var(--accent-hot, #e6534f);
    outline: none;
  }

  .drawer-body {
    flex: 1;
    overflow-y: auto;
    padding: 1.25rem 1.2rem 2rem;
  }

  .drawer-section {
    margin-bottom: 1.5rem;
  }

  .section-title {
    margin: 0 0 0.5rem;
    font-size: 0.72rem;
    letter-spacing: 0.3em;
    text-transform: uppercase;
    color: var(--text-secondary, #9a9a94);
    font-family: 'JetBrains Mono', ui-monospace, monospace;
  }

  .drawer-desc {
    margin: 0;
    font-size: 0.9rem;
    line-height: 1.6;
    color: var(--text-primary, #f6f6f3);
  }

  .motiv-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }

  .motiv-item {
    background: var(--bg-card, #17171c);
    border: 1px solid var(--border, rgba(255, 255, 255, 0.08));
    border-radius: 8px;
    padding: 0.75rem 0.85rem;
  }

  .motiv-head {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 0.75rem;
  }

  .motiv-content {
    flex: 1;
    font-size: 0.88rem;
    line-height: 1.5;
    color: var(--text-primary, #f6f6f3);
  }

  .motiv-conf {
    font-family: 'JetBrains Mono', ui-monospace, monospace;
    font-size: 0.75rem;
    color: var(--accent-gold, #d4a851);
    padding: 2px 6px;
    border-radius: 4px;
    border: 1px solid rgba(212, 168, 81, 0.4);
    flex-shrink: 0;
  }

  .motiv-source {
    margin-top: 0.4rem;
    font-size: 0.72rem;
    color: var(--text-secondary, #9a9a94);
    font-style: italic;
  }

  @media (prefers-reduced-motion: reduce) {
    .char-drawer,
    .drawer-overlay {
      animation: none;
    }
  }
</style>
