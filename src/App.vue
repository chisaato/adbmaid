<script setup lang="ts">
import { computed, h, onMounted, type Component } from 'vue'
import {
  NAlert,
  NConfigProvider,
  NDialogProvider,
  NGlobalStyle,
  NIcon,
  NLayout,
  NLayoutContent,
  NLayoutSider,
  NMenu,
  NMessageProvider,
  NSpace,
  NTag,
  darkTheme,
  zhCN,
  dateZhCN,
  type MenuOption,
} from 'naive-ui'
import {
  AppsOutline,
  DesktopOutline,
  DocumentTextOutline,
  PhonePortraitOutline,
  SettingsOutline,
  TerminalOutline,
  WifiOutline,
} from '@vicons/ionicons5'
import { isTauriEnv } from '@/api/client'
import ApkDropInstaller from '@/components/ApkDropInstaller.vue'
import DeviceSwitcherPopover from '@/components/DeviceSwitcherPopover.vue'
import ToolchainModal from '@/components/ToolchainModal.vue'
import { useWorkbenchStore, type NavKey } from '@/stores/workbench'
import AppsView from '@/views/AppsView.vue'
import DevicesView from '@/views/DevicesView.vue'
import LogcatView from '@/views/LogcatView.vue'
import MdnsRadarView from '@/views/MdnsRadarView.vue'
import ScrcpyView from '@/views/ScrcpyView.vue'
import SettingsView from '@/views/SettingsView.vue'
import TerminalView from '@/views/TerminalView.vue'

const store = useWorkbenchStore()

function renderIcon(icon: Component) {
  return () => h(NIcon, null, { default: () => h(icon) })
}

const menuOptions: MenuOption[] = [
  { label: '设备概览', key: 'devices', icon: renderIcon(PhonePortraitOutline) },
  { label: '无线发现 (mDNS)', key: 'mdns', icon: renderIcon(WifiOutline) },
  { label: '应用管理', key: 'apps', icon: renderIcon(AppsOutline) },
  { label: '交互终端', key: 'terminal', icon: renderIcon(TerminalOutline) },
  { label: '实时 Logcat', key: 'logcat', icon: renderIcon(DocumentTextOutline) },
  { label: '投屏 (Scrcpy)', key: 'scrcpy', icon: renderIcon(DesktopOutline) },
  { label: '全局设置', key: 'settings', icon: renderIcon(SettingsOutline) },
]

const currentView = computed(() => {
  switch (store.activeNav) {
    case 'devices':
      return DevicesView
    case 'mdns':
      return MdnsRadarView
    case 'apps':
      return AppsView
    case 'terminal':
      return TerminalView
    case 'logcat':
      return LogcatView
    case 'scrcpy':
      return ScrcpyView
    case 'settings':
      return SettingsView
  }
})

const adbReady = computed(() => store.toolchain?.adb.found ?? false)
const scrcpyReady = computed(() => store.toolchain?.scrcpy.found ?? false)
const previewMode = !isTauriEnv()

onMounted(() => {
  void store.init()
})
</script>

<template>
  <NConfigProvider
    :theme="store.isDark ? darkTheme : null"
    :locale="zhCN"
    :date-locale="dateZhCN"
  >
    <NGlobalStyle />
    <NMessageProvider>
      <NDialogProvider>
        <NLayout has-sider class="app-shell">
          <NLayoutSider
            bordered
            :width="248"
            content-style="display: flex; flex-direction: column; height: 100%; padding: 12px 10px;"
          >
            <!-- 品牌头 -->
            <div class="brand-header">
              <span class="brand-name">adbmaid</span>
              <NTag size="tiny" type="info" round>v0.1 原型</NTag>
            </div>

            <!-- 在线设备切换器（紧凑弹出式，支持多设备搜索） -->
            <DeviceSwitcherPopover />

            <!-- 主导航 -->
            <div class="nav-wrap">
              <NMenu
                :value="store.activeNav"
                :options="menuOptions"
                :indent="16"
                @update:value="(k) => (store.activeNav = k as NavKey)"
              />
            </div>

            <!-- 底部工具链状态胶囊 -->
            <div class="toolchain-capsule" @click="store.toolchainModalOpen = true">
              <NSpace justify="space-between" align="center" style="width: 100%">
                <span style="font-size: 12px; font-weight: 600">工具链状态</span>
                <NSpace :size="4">
                  <NTag size="tiny" :type="adbReady ? 'success' : 'error'">adb</NTag>
                  <NTag size="tiny" :type="scrcpyReady ? 'success' : 'warning'">scrcpy</NTag>
                </NSpace>
              </NSpace>
            </div>
          </NLayoutSider>

          <NLayoutContent content-style="padding: 16px 20px; min-height: 100vh;">
            <NAlert
              v-if="previewMode"
              type="info"
              closable
              style="margin-bottom: 12px"
            >
              当前处于浏览器 UI 原型预览模式（使用内置模拟数据）；通过 Tauri 桌面运行时将自动连接真实 5037 ADB Server 与 SQLite。
            </NAlert>
            <component :is="currentView" />
          </NLayoutContent>
        </NLayout>

        <ToolchainModal />
        <ApkDropInstaller />
      </NDialogProvider>
    </NMessageProvider>
  </NConfigProvider>
</template>

<style scoped>
.app-shell {
  height: 100vh;
}
.brand-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 4px 8px 12px;
}
.brand-name {
  font-size: 18px;
  font-weight: 700;
  letter-spacing: 0.4px;
}
.nav-wrap {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}
.toolchain-capsule {
  margin-top: 8px;
  padding: 8px 10px;
  border-radius: 8px;
  border: 1px solid rgba(128, 128, 128, 0.25);
  cursor: pointer;
  transition: background-color 0.15s;
}
.toolchain-capsule:hover {
  background-color: rgba(128, 128, 128, 0.1);
}
</style>
