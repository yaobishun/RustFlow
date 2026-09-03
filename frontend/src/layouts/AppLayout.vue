<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useAuthStore } from '../stores/auth'
import { useProjectStore } from '../stores/projects'
import { DataAnalysis, Files, Management, Money, Opportunity, SwitchButton, UserFilled, Warning } from '@element-plus/icons-vue'

const collapsed = ref(false)
const mobileOpen = ref(false)
const route = useRoute(); const router = useRouter(); const auth = useAuthStore(); const projects = useProjectStore()
const title = computed(() => String(route.meta.title || 'RustFlow'))
const menu = [
  { path:'/dashboard', label:'综合仪表盘', icon:DataAnalysis }, { path:'/projects', label:'项目管理', icon:Files },
  { path:'/team', label:'团队负载', icon:UserFilled }, { path:'/costs', label:'工时与成本', icon:Money },
  { path:'/risks', label:'风险预警', icon:Warning }, { path:'/decisions', label:'方案决策', icon:Opportunity },
]
function navigate(path:string) { mobileOpen.value=false; router.push(path) }
async function logout() { await auth.logout(); router.push('/login') }
function changeProject(value: number) { projects.select(value); window.dispatchEvent(new CustomEvent('rustflow:project-change', { detail: value })) }
onMounted(() => projects.load().catch(() => undefined))
</script>
<template>
  <div class="app-shell" :class="`role-${auth.user?.role || 'guest'}`">
    <div v-if="mobileOpen" class="mobile-mask" @click="mobileOpen=false" />
    <aside class="sidebar" :class="{ collapsed, 'mobile-open': mobileOpen }">
      <div class="brand"><div class="brand-mark">R</div><div v-if="!collapsed" class="brand-copy"><strong>RustFlow</strong><span>研发协同 · 成本决策</span></div></div>
      <nav><button v-for="item in menu" :key="item.path" :class="{active:route.path.startsWith(item.path)}" @click="navigate(item.path)"><el-icon><component :is="item.icon" /></el-icon><span v-if="!collapsed">{{ item.label }}</span></button></nav>
      <div class="sidebar-foot" v-if="!collapsed"><span>Rust 核心驱动</span><small>工程管理可视化平台</small></div>
    </aside>
    <main class="main-area">
      <header class="topbar">
        <button class="icon-button desktop-toggle" @click="collapsed=!collapsed"><el-icon><Management /></el-icon></button>
        <button class="icon-button mobile-toggle" @click="mobileOpen=true"><el-icon><Management /></el-icon></button>
        <div class="topbar-title"><span>工作台</span><strong>{{ title }}</strong></div><el-select v-if="projects.projects.length" :model-value="projects.selectedId" class="project-switcher" placeholder="选择项目" @change="changeProject"><el-option v-for="p in projects.projects" :key="p.id" :label="p.name" :value="p.id"/></el-select>
        <div class="user-area"><div class="avatar">{{ (auth.user?.display_name || auth.user?.name || auth.user?.username || '用').slice(0,1) }}</div><div class="user-copy"><strong>{{ auth.user?.display_name || auth.user?.name || auth.user?.username }}</strong><span>{{ auth.user?.role || '用户' }}</span></div><el-button text @click="logout"><el-icon><SwitchButton /></el-icon></el-button></div>
      </header>
      <section class="page-content"><router-view /></section>
    </main>
  </div>
</template>
