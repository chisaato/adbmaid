<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import {
  NAlert,
  NButton,
  NCard,
  NCode,
  NDescriptions,
  NDescriptionsItem,
  NDivider,
  NInput,
  NModal,
  NProgress,
  NSpace,
  NTag,
  useMessage,
} from 'naive-ui'
import { api } from '@/api/client'
import type { DownloadProgress } from '@/api/types'
import { useWorkbenchStore } from '@/stores/workbench'

const store = useWorkbenchStore()
const message = useMessage()

const adbCustomPath = ref('')
const scrcpyCustomPath = ref('')
const mirrorPrefix = ref('')
const localArchivePath = ref('')
const downloading = ref(false)
const downloadProgress = ref<DownloadProgress | null>(null)

watch(
  () => store.toolchainModalOpen,
  async (open) => {
    if (open) {
      await store.refreshToolchain()
      adbCustomPath.value = (await api.getSetting<string>('toolchain.adb_path')) ?? ''
      scrcpyCustomPath.value = (await api.getSetting<string>('toolchain.scrcpy_path')) ?? ''
      mirrorPrefix.value = (await api.getSetting<string>('network.github_mirror')) ?? ''
    }
  }
)

const progressPercent = computed(() => {
  const p = downloadProgress.value
  if (!p || !p.totalBytes || p.totalBytes <= 0) return 0
  return Math.min(100, Math.round((p.receivedBytes / p.totalBytes) * 100))
})

async function handleSavePaths() {
  try {
    await api.saveToolchainPaths(adbCustomPath.value, scrcpyCustomPath.value)
    await api.setSetting('network.github_mirror', mirrorPrefix.value)
    await store.refreshToolchain()
    message.success('工具链配置已保存并重新探测')
  } catch (e) {
    message.error(String(e))
  }
}

async function handleDownloadScrcpy() {
  downloading.value = true
  downloadProgress.value = {
    stage: 'fetching_meta',
    receivedBytes: 0,
    totalBytes: null,
    message: '准备下载...',
  }
  try {
    const tag = await api.downloadToolchain('scrcpy', mirrorPrefix.value || undefined, (p) => {
      downloadProgress.value = p
    })
    await store.refreshToolchain()
    message.success(`scrcpy ${tag} 已就绪`)
  } catch (e) {
    message.error(`下载失败: ${e}`)
  } finally {
    downloading.value = false
  }
}

async function handleImportArchive(kind: 'scrcpy' | 'adb') {
  if (!localArchivePath.value.trim()) {
    message.warning('请输入本地压缩包完整路径（.tar.gz 或 .zip）')
    return
  }
  try {
    await api.importToolchainArchive(localArchivePath.value.trim(), kind)
    await store.refreshToolchain()
    message.success('离线压缩包导入成功')
  } catch (e) {
    message.error(`导入失败: ${e}`)
  }
}
</script>

<template>
  <NModal
    v-model:show="store.toolchainModalOpen"
    preset="card"
    title="工具链探测与托管向导 (Toolchain Manager)"
    style="width: 720px; max-width: 94vw"
  >
    <NSpace vertical :size="14" v-if="store.toolchain">
      <NAlert
        v-if="store.toolchain.scrcpy.isOutdated"
        type="warning"
        title="检测到系统 scrcpy 版本过旧（< 3.0）"
      >
        旧版 scrcpy 不支持虚拟屏与新版 Android 协议，建议点击下方一键下载托管最新版。
      </NAlert>

      <NDescriptions bordered size="small" :column="2">
        <NDescriptionsItem label="运行平台">
          {{ store.toolchain.os }} / {{ store.toolchain.arch }}
        </NDescriptionsItem>
        <NDescriptionsItem label="ADB 状态">
          <NTag :type="store.toolchain.adb.found ? 'success' : 'error'" size="small">
            {{ store.toolchain.adb.found ? `就绪 (${store.toolchain.adb.source})` : '未找到' }}
          </NTag>
        </NDescriptionsItem>
        <NDescriptionsItem label="ADB 路径" :span="2">
          <code>{{ store.toolchain.adb.path || '—' }}</code>
          <span v-if="store.toolchain.adb.version" style="margin-left: 8px; opacity: 0.7">
            ({{ store.toolchain.adb.version }})
          </span>
        </NDescriptionsItem>
        <NDescriptionsItem label="scrcpy 状态">
          <NTag :type="store.toolchain.scrcpy.found ? 'success' : 'warning'" size="small">
            {{
              store.toolchain.scrcpy.found
                ? `v${store.toolchain.scrcpy.version || '?'} (${store.toolchain.scrcpy.source})`
                : '未找到'
            }}
          </NTag>
        </NDescriptionsItem>
        <NDescriptionsItem label="scrcpy-server">
          <code>{{ store.toolchain.scrcpy.serverPath || '随默认目录' }}</code>
        </NDescriptionsItem>
      </NDescriptions>

      <!-- Linux 专项提示 -->
      <NCard
        v-if="store.toolchain.linuxDistro && !store.toolchain.canAutoDownloadScrcpy"
        size="small"
        title="Linux 非 x86_64 架构引导"
      >
        <p style="margin: 0 0 8px">
          当前发行版：<strong>{{ store.toolchain.linuxDistro.prettyName }}</strong>。上游官方静态包仅提供
          x86_64，请使用包管理器或源码编译后在下方绑定自定义路径：
        </p>
        <NCode :code="store.toolchain.linuxDistro.installHint" language="bash" />
      </NCard>

      <!-- 自动下载区 -->
      <NCard size="small" title="自动下载官方托管包 (Genymobile/scrcpy)">
        <NSpace vertical :size="10">
          <NInput
            v-model:value="mirrorPrefix"
            placeholder="可选：GitHub 加速镜像前缀，如 https://ghproxy.net/"
            clearable
          />
          <NSpace>
            <NButton
              type="primary"
              :disabled="!store.toolchain.canAutoDownloadScrcpy"
              :loading="downloading"
              @click="handleDownloadScrcpy"
            >
              下载最新版 scrcpy (含 adb + scrcpy-server)
            </NButton>
            <NButton @click="store.refreshToolchain()">重新探测</NButton>
          </NSpace>
          <div v-if="downloadProgress">
            <NProgress
              type="line"
              :percentage="progressPercent"
              :status="downloadProgress.stage === 'error' ? 'error' : 'default'"
              :processing="downloading"
            />
            <div style="font-size: 12px; opacity: 0.75; margin-top: 4px">
              {{ downloadProgress.message }}
            </div>
          </div>
        </NSpace>
      </NCard>

      <NDivider style="margin: 4px 0">高级：手动路径与离线导入</NDivider>

      <NSpace vertical :size="8">
        <NInput v-model:value="adbCustomPath" placeholder="自定义 adb 可执行文件绝对路径（留空自动探测）" />
        <NInput v-model:value="scrcpyCustomPath" placeholder="自定义 scrcpy 可执行文件绝对路径（留空自动探测）" />
        <NButton size="small" @click="handleSavePaths">保存自定义路径</NButton>
      </NSpace>

      <NSpace align="center">
        <NInput
          v-model:value="localArchivePath"
          placeholder="离线导入官方 .tar.gz / .zip 压缩包绝对路径"
          style="width: 440px"
        />
        <NButton size="small" @click="handleImportArchive('scrcpy')">导入 scrcpy 包</NButton>
        <NButton size="small" @click="handleImportArchive('adb')">导入 platform-tools</NButton>
      </NSpace>
    </NSpace>
  </NModal>
</template>
