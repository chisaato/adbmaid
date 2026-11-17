<script setup lang="ts">
import { onMounted, ref } from 'vue'
import {
  NButton,
  NCard,
  NForm,
  NFormItem,
  NInput,
  NInputNumber,
  NRadioButton,
  NRadioGroup,
  NSpace,
  useMessage,
} from 'naive-ui'
import { api } from '@/api/client'
import { useWorkbenchStore } from '@/stores/workbench'

const store = useWorkbenchStore()
const message = useMessage()

const proxyUrl = ref('')
const githubMirror = ref('')
const appDataDir = ref('')
const maxLines = ref(store.logcatMaxLines)

onMounted(async () => {
  proxyUrl.value = (await api.getSetting<string>('network.proxy')) ?? ''
  githubMirror.value = (await api.getSetting<string>('network.github_mirror')) ?? ''
  appDataDir.value = await api.getAppDataDir()
})

async function saveGlobalSettings() {
  await api.setSetting('network.proxy', proxyUrl.value.trim())
  await api.setSetting('network.github_mirror', githubMirror.value.trim())
  await api.setSetting('logcat.maxLines', maxLines.value)
  store.logcatMaxLines = maxLines.value
  message.success('全局设置已保存')
}
</script>

<template>
  <NSpace vertical :size="16">
    <NCard title="外观与界面" size="small">
      <NForm label-placement="left" label-width="140">
        <NFormItem label="应用主题">
          <NRadioGroup
            :value="store.themeMode"
            @update:value="(v) => store.setThemeMode(v)"
          >
            <NRadioButton value="auto">跟随系统</NRadioButton>
            <NRadioButton value="light">浅色</NRadioButton>
            <NRadioButton value="dark">深色</NRadioButton>
          </NRadioGroup>
        </NFormItem>
      </NForm>
    </NCard>

    <NCard title="工具链与网络" size="small">
      <NForm label-placement="left" label-width="140">
        <NFormItem label="工具链状态">
          <NSpace align="center">
            <NButton size="small" @click="store.toolchainModalOpen = true">
              打开工具链探测与下载向导
            </NButton>
          </NSpace>
        </NFormItem>
        <NFormItem label="HTTP/SOCKS5 代理">
          <NInput v-model:value="proxyUrl" placeholder="如 http://127.0.0.1:7890（留空直连）" />
        </NFormItem>
        <NFormItem label="GitHub 下载镜像">
          <NInput v-model:value="githubMirror" placeholder="如 https://ghproxy.net/" />
        </NFormItem>
        <NFormItem label="Logcat 内存上限">
          <NInputNumber v-model:value="maxLines" :min="1000" :max="200000" :step="5000" />
        </NFormItem>
        <NFormItem label="数据存储目录">
          <code>{{ appDataDir }}</code>
        </NFormItem>
      </NForm>
      <NSpace justify="end">
        <NButton type="primary" @click="saveGlobalSettings">保存设置</NButton>
      </NSpace>
    </NCard>
  </NSpace>
</template>
