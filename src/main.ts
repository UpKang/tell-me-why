import { mount } from 'svelte'
import './app.css'
import App from './App.svelte'
import { initTheme } from './lib/stores/theme'

// 在挂载 App 前同步初始化主题：避免首屏渲染出错误配色后再闪切（FOUC）
initTheme()

const app = mount(App, {
  target: document.getElementById('app')!,
})

export default app
