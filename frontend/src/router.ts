import { createRouter, createWebHashHistory, createWebHistory } from 'vue-router'
import { useAuthStore } from './stores/auth'

const routes = [
  { path: '/login', component: () => import('./views/LoginView.vue'), meta: { public: true } },
  { path: '/', component: () => import('./layouts/AppLayout.vue'), redirect: '/dashboard', children: [
    { path: 'dashboard', component: () => import('./views/DashboardView.vue'), meta: { title: '综合仪表盘' } },
    { path: 'projects', component: () => import('./views/ProjectsView.vue'), meta: { title: '项目管理' } },
    { path: 'projects/:id', component: () => import('./views/ProjectDetailView.vue'), meta: { title: '项目详情' } },
    { path: 'projects/:id/board', component: () => import('./views/TaskBoardView.vue'), meta: { title: '任务看板' } },
    { path: 'tasks/:id', component: () => import('./views/TaskDetailView.vue'), meta: { title: '任务详情' } },
    { path: 'team', component: () => import('./views/TeamView.vue'), meta: { title: '团队负载' } },
    { path: 'costs', component: () => import('./views/CostsView.vue'), meta: { title: '工时与成本' } },
    { path: 'risks', component: () => import('./views/RisksView.vue'), meta: { title: '风险预警' } },
    { path: 'decisions', component: () => import('./views/DecisionsView.vue'), meta: { title: '技术方案决策' } },
  ]},
  { path: '/:pathMatch(.*)*', redirect: '/' },
]

const router = createRouter({
  history: location.protocol === 'file:' ? createWebHashHistory() : createWebHistory(),
  routes,
})
router.beforeEach((to) => {
  const auth = useAuthStore()
  if (!to.meta.public && !auth.authenticated) return `/login?redirect=${encodeURIComponent(to.fullPath)}`
  if (to.path === '/login' && auth.authenticated) return '/dashboard'
})
export default router
