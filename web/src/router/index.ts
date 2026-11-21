import { createRouter, createWebHistory, createMemoryHistory, type RouteRecordRaw } from 'vue-router'
import { getStoredToken } from '@/api/client'

import LoginView from '@/views/LoginView.vue'
import DashboardView from '@/views/DashboardView.vue'
import DevicesView from '@/views/DevicesView.vue'
import DeviceDetailView from '@/views/DeviceDetailView.vue'
import GroupsView from '@/views/GroupsView.vue'
import PatchManagementView from '@/views/PatchManagementView.vue'
import CatalogView from '@/views/CatalogView.vue'
import DeploymentsView from '@/views/DeploymentsView.vue'
import JobsAndWindowsView from '@/views/JobsAndWindowsView.vue'
import PoliciesView from '@/views/PoliciesView.vue'
import EnrollmentView from '@/views/EnrollmentView.vue'

export const routes: RouteRecordRaw[] = [
  {
    path: '/login',
    name: 'login',
    component: LoginView,
    meta: { public: true },
  },
  {
    path: '/',
    name: 'dashboard',
    component: DashboardView,
  },
  {
    path: '/devices',
    name: 'devices',
    component: DevicesView,
  },
  {
    path: '/devices/:id',
    name: 'device-detail',
    component: DeviceDetailView,
  },
  {
    path: '/groups',
    name: 'groups',
    component: GroupsView,
  },
  {
    path: '/patches',
    name: 'patches',
    component: PatchManagementView,
  },
  {
    path: '/catalog',
    name: 'catalog',
    component: CatalogView,
  },
  {
    path: '/deployments',
    name: 'deployments',
    component: DeploymentsView,
  },
  {
    path: '/jobs',
    name: 'jobs',
    component: JobsAndWindowsView,
  },
  {
    path: '/policies',
    name: 'policies',
    component: PoliciesView,
  },
  {
    path: '/enrollment',
    name: 'enrollment',
    component: EnrollmentView,
  },
]

export function createAppRouter(useMemory = false) {
  const router = createRouter({
    history: useMemory ? createMemoryHistory() : createWebHistory(),
    routes,
  })

  router.beforeEach((to) => {
    const token = getStoredToken()
    if (!to.meta.public && !token) {
      return {
        path: '/login',
        query: to.fullPath && to.fullPath !== '/' ? { redirect: to.fullPath } : undefined,
      }
    }
    if (to.path === '/login' && token) {
      return { path: '/' }
    }
    return true
  })

  return router
}

const router = createAppRouter(false)
export default router
