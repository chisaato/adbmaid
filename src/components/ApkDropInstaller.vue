<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import {
  NButton,
  NCheckbox,
  NIcon,
  NModal,
  NProgress,
  NSpace,
  NTag,
  useMessage,
} from 'naive-ui'
import {
  CheckmarkCircleOutline,
  CloseCircleOutline,
  CloudUploadOutline,
  CubeOutline,
  TrashOutline,
} from '@vicons/ionicons5'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import { api, isTauriEnv } from '@/api/client'
import type { InstallFlags, InstallProgress } from '@/api/types'
import { useWorkbenchStore } from '@/stores/workbench'

const store = useWorkbenchStore()
const message = useMessage()

const dragging = ref(false)
const dragApkCount = ref<number | null>(null)
const dragOnlyUnsupported = ref(false)

const modalOpen = ref(false)
const pendingPaths = ref<string[]>([])
const targetSerial = ref<string | null>(null)
const targetLabel = ref('')
const installing = ref(false)
const installLogs = ref<InstallProgress[]>([])

const installFlags = ref<InstallFlags>({
  replace: true,
  downgrade: false,
  grantPermissions: true,
  allowTest: false,
})

let unlistenDragDrop: (() => void) | null = null
let browserDragDepth = 0

function fileNameOf(path: string): string {
  const parts = path.split(/[/\\]/)
  return parts[parts.length - 1] || path
}

const completedCount = computed(
  () => installLogs.value.filter((l) => l.status === 'success' || l.status === 'error').length
)

const progressPercent = computed(() => {
  if (pendingPaths.value.length === 0) return 0
  return Math.round((completedCount.value / pendingPaths.value.length) * 100)
})

const hasFinished = computed(
  () =>
    !installing.value &&
    installLogs.value.length > 0 &&
    completedCount.value === pendingPaths.value.length
)

function resetDragState() {
  dragging.value = false
  dragApkCount.value = null
  dragOnlyUnsupported.value = false
}

function handleDroppedPaths(rawPaths: string[]) {
  const apks = rawPaths.filter((p) => p.toLowerCase().endsWith('.apk'))
  if (apks.length === 0) {
    message.warning('未检测到 .apk 文件，仅支持拖入 APK 安装包')
    return
  }

  const dev = store.selectedDevice
  if (!dev) {
    message.warning('当前没有在线设备，请先连接设备后再拖入安装')
    return
  }
  if (dev.state !== 'device') {
    message.warning(`当前选中设备状态为 ${dev.state}，请先完成授权或连接`)
    return
  }

  if (installing.value) {
    message.warning('当前正在安装 APK，请等待本次任务完成')
    return
  }

  const devTitle = dev.alias || dev.marketName || dev.model || dev.serial
  if (modalOpen.value && targetSerial.value === dev.serial && !hasFinished.value) {
    // 弹窗已打开且尚未开始安装时，合并去重追加
    const merged = new Set([...pendingPaths.value, ...apks])
    pendingPaths.value = Array.from(merged)
  } else {
    targetSerial.value = dev.serial
    targetLabel.value = `${devTitle} (${dev.serial})`
    pendingPaths.value = Array.from(new Set(apks))
    installLogs.value = []
    modalOpen.value = true
  }
}

function removePath(path: string) {
  if (installing.value) return
  pendingPaths.value = pendingPaths.value.filter((p) => p !== path)
  if (pendingPaths.value.length === 0) {
    modalOpen.value = false
  }
}

async function startInstall() {
  if (!targetSerial.value || pendingPaths.value.length === 0 || installing.value) return
  installing.value = true
  installLogs.value = []

  try {
    await api.installApks(targetSerial.value, pendingPaths.value, installFlags.value, (p) => {
      const idx = installLogs.value.findIndex((x) => x.index === p.index)
      if (idx >= 0) {
        installLogs.value[idx] = p
      } else {
        installLogs.value.push(p)
      }
    })
    const errCount = installLogs.value.filter((l) => l.status === 'error').length
    const okCount = installLogs.value.filter((l) => l.status === 'success').length
    if (errCount === 0) {
      message.success(`已成功安装 ${okCount} 个 APK`)
    } else {
      message.warning(`安装完成：${okCount} 个成功，${errCount} 个失败`)
    }
    store.notifyApkInstalled()
  } catch (e) {
    message.error(`安装过程异常: ${String(e)}`)
  } finally {
    installing.value = false
  }
}

function closeModal() {
  if (installing.value) return
  modalOpen.value = false
}

// 浏览器预览模式降级处理（仅阻止默认打开并给出提示）
function onBrowserDragEnter(e: DragEvent) {
  e.preventDefault()
  browserDragDepth += 1
  dragging.value = true
}

function onBrowserDragOver(e: DragEvent) {
  e.preventDefault()
}

function onBrowserDragLeave(e: DragEvent) {
  e.preventDefault()
  browserDragDepth = Math.max(0, browserDragDepth - 1)
  if (browserDragDepth === 0) {
    resetDragState()
  }
}

function onBrowserDrop(e: DragEvent) {
  e.preventDefault()
  browserDragDepth = 0
  resetDragState()
  const files = Array.from(e.dataTransfer?.files ?? [])
  const apkNames = files.filter((f) => f.name.toLowerCase().endsWith('.apk')).map((f) => f.name)
  if (apkNames.length === 0) {
    message.warning('仅支持拖入 .apk 安装包文件')
    return
  }
  message.info(`预览模式检测到 ${apkNames.length} 个 APK；请在 Tauri 桌面端拖入以获取本地路径并执行安装`)
}

onMounted(async () => {
  if (isTauriEnv()) {
    try {
      unlistenDragDrop = await getCurrentWebview().onDragDropEvent((event) => {
        const payload = event.payload
        if (payload.type === 'enter') {
          dragging.value = true
          if (payload.paths && payload.paths.length > 0) {
            const apks = payload.paths.filter((p) => p.toLowerCase().endsWith('.apk'))
            dragApkCount.value = apks.length
            dragOnlyUnsupported.value = apks.length === 0
          } else {
            dragApkCount.value = null
            dragOnlyUnsupported.value = false
          }
        } else if (payload.type === 'over') {
          dragging.value = true
        } else if (payload.type === 'leave') {
          resetDragState()
        } else if (payload.type === 'drop') {
          resetDragState()
          handleDroppedPaths(payload.paths ?? [])
        }
      })
    } catch {
      // 忽略非桌面环境监听异常
    }
  } else {
    window.addEventListener('dragenter', onBrowserDragEnter)
    window.addEventListener('dragover', onBrowserDragOver)
    window.addEventListener('dragleave', onBrowserDragLeave)
    window.addEventListener('drop', onBrowserDrop)
  }
})

onBeforeUnmount(() => {
  if (unlistenDragDrop) {
    unlistenDragDrop()
    unlistenDragDrop = null
  }
  if (!isTauriEnv()) {
    window.removeEventListener('dragenter', onBrowserDragEnter)
    window.removeEventListener('dragover', onBrowserDragOver)
    window.removeEventListener('dragleave', onBrowserDragLeave)
    window.removeEventListener('drop', onBrowserDrop)
  }
})
</script>

<template>
  <!-- 全窗口拖拽提示遮罩 -->
  <Transition name="fade-overlay">
    <div v-if="dragging" class="apk-drop-overlay" :class="{ warn: dragOnlyUnsupported }">
      <div class="drop-card">
        <NIcon size="46" :color="dragOnlyUnsupported ? '#f0a020' : '#18a058'">
          <CloudUploadOutline />
        </NIcon>
        <div class="drop-title">
          <template v-if="dragOnlyUnsupported">
            仅支持 .apk 安装包文件
          </template>
          <template v-else-if="dragApkCount !== null">
            松开鼠标，准备安装 {{ dragApkCount }} 个 APK
          </template>
          <template v-else>
            松开鼠标，安装 APK 到当前设备
          </template>
        </div>
        <div class="drop-subtitle">
          <template v-if="store.selectedDevice">
            目标设备：{{ store.selectedDevice.alias || store.selectedDevice.model || store.selectedDevice.serial }}
            ({{ store.selectedDevice.serial }})
          </template>
          <template v-else>
            当前暂无在线设备，请先连接设备
          </template>
        </div>
      </div>
    </div>
  </Transition>

  <!-- 拖拽安装确认与进度弹窗 -->
  <NModal
    v-model:show="modalOpen"
    preset="card"
    title="拖拽安装 APK"
    style="width: 540px; max-width: 92vw;"
    :mask-closable="!installing"
    :closable="!installing"
  >
    <NSpace vertical :size="14">
      <div class="target-bar">
        <span class="target-label">目标设备</span>
        <NTag type="info" size="small" :bordered="false">{{ targetLabel }}</NTag>
      </div>

      <div class="apk-list">
        <div v-for="path in pendingPaths" :key="path" class="apk-item">
          <NIcon size="16" class="apk-icon">
            <CubeOutline />
          </NIcon>
          <div class="apk-info">
            <div class="apk-name">{{ fileNameOf(path) }}</div>
            <div class="apk-path" :title="path">{{ path }}</div>
          </div>
          <NButton
            v-if="!installing && !hasFinished"
            quaternary
            circle
            size="tiny"
            @click="removePath(path)"
          >
            <template #icon>
              <NIcon size="14">
                <TrashOutline />
              </NIcon>
            </template>
          </NButton>
        </div>
      </div>

      <NSpace :size="14" wrap>
        <NCheckbox v-model:checked="installFlags.replace" :disabled="installing">
          覆盖安装 (-r)
        </NCheckbox>
        <NCheckbox v-model:checked="installFlags.grantPermissions" :disabled="installing">
          自动授权 (-g)
        </NCheckbox>
        <NCheckbox v-model:checked="installFlags.downgrade" :disabled="installing">
          允许降级 (-d)
        </NCheckbox>
        <NCheckbox v-model:checked="installFlags.allowTest" :disabled="installing">
          允许测试包 (-t)
        </NCheckbox>
      </NSpace>

      <div v-if="installing || installLogs.length > 0" class="progress-section">
        <NProgress
          type="line"
          :percentage="progressPercent"
          :status="installLogs.some((l) => l.status === 'error') ? 'warning' : 'success'"
          :processing="installing"
        />
        <div class="log-list">
          <div v-for="log in installLogs" :key="log.index" class="log-row">
            <NIcon
              v-if="log.status === 'success'"
              size="15"
              color="#18a058"
              style="flex-shrink: 0"
            >
              <CheckmarkCircleOutline />
            </NIcon>
            <NIcon
              v-else-if="log.status === 'error'"
              size="15"
              color="#d03050"
              style="flex-shrink: 0"
            >
              <CloseCircleOutline />
            </NIcon>
            <NTag
              size="tiny"
              :type="
                log.status === 'success'
                  ? 'success'
                  : log.status === 'error'
                    ? 'error'
                    : 'info'
              "
            >
              {{ log.index + 1 }}/{{ log.total }}
            </NTag>
            <span class="log-file">{{ fileNameOf(log.apkPath) }}</span>
            <span class="log-msg">{{ log.message }}</span>
          </div>
        </div>
      </div>
    </NSpace>

    <template #footer>
      <NSpace justify="end">
        <NButton :disabled="installing" @click="closeModal">
          {{ hasFinished ? '关闭' : '取消' }}
        </NButton>
        <NButton
          v-if="!hasFinished"
          type="primary"
          :loading="installing"
          :disabled="pendingPaths.length === 0"
          @click="startInstall"
        >
          开始安装 ({{ pendingPaths.length }})
        </NButton>
      </NSpace>
    </template>
  </NModal>
</template>

<style scoped>
.apk-drop-overlay {
  position: fixed;
  inset: 0;
  z-index: 9999;
  display: flex;
  align-items: center;
  justify-content: center;
  background-color: rgba(16, 20, 26, 0.72);
  backdrop-filter: blur(5px);
  pointer-events: none;
}
.drop-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
  padding: 36px 48px;
  border-radius: 16px;
  border: 2px dashed rgba(24, 160, 88, 0.75);
  background-color: rgba(24, 28, 36, 0.92);
  color: #fff;
  text-align: center;
  box-shadow: 0 18px 48px rgba(0, 0, 0, 0.45);
}
.apk-drop-overlay.warn .drop-card {
  border-color: rgba(240, 160, 32, 0.8);
}
.drop-title {
  font-size: 18px;
  font-weight: 600;
}
.drop-subtitle {
  font-size: 13px;
  opacity: 0.78;
}
.fade-overlay-enter-active,
.fade-overlay-leave-active {
  transition: opacity 0.14s ease;
}
.fade-overlay-enter-from,
.fade-overlay-leave-to {
  opacity: 0;
}
.target-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 6px 10px;
  border-radius: 6px;
  background-color: rgba(128, 128, 128, 0.08);
}
.target-label {
  font-size: 12px;
  opacity: 0.72;
}
.apk-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
  max-height: 180px;
  overflow-y: auto;
}
.apk-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 7px 10px;
  border-radius: 6px;
  border: 1px solid rgba(128, 128, 128, 0.22);
}
.apk-icon {
  flex-shrink: 0;
  color: #18a058;
}
.apk-info {
  flex: 1;
  min-width: 0;
}
.apk-name {
  font-size: 13px;
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.apk-path {
  font-size: 11px;
  font-family: monospace;
  opacity: 0.6;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.progress-section {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.log-list {
  display: flex;
  flex-direction: column;
  gap: 5px;
  max-height: 150px;
  overflow-y: auto;
  font-size: 12px;
}
.log-row {
  display: flex;
  align-items: center;
  gap: 6px;
}
.log-file {
  font-weight: 500;
  flex-shrink: 0;
}
.log-msg {
  opacity: 0.78;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
