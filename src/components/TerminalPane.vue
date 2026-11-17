<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { Terminal, type ITheme } from '@xterm/xterm'
import { FitAddon } from '@xterm/addon-fit'
import { SearchAddon } from '@xterm/addon-search'
import { WebLinksAddon } from '@xterm/addon-web-links'
import '@xterm/xterm/css/xterm.css'
import { NButton, NCheckbox, NInput, NSpace, useMessage } from 'naive-ui'
import { api } from '@/api/client'
import type { TerminalAppearance } from '@/api/types'
import { useWorkbenchStore } from '@/stores/workbench'

const props = defineProps<{
  serial: string
  appearance: TerminalAppearance
}>()

const store = useWorkbenchStore()
const message = useMessage()

const containerRef = ref<HTMLElement | null>(null)
const searchVisible = ref(false)
const searchKeyword = ref('')
const searchCaseSensitive = ref(false)
const searchRegex = ref(false)
const sessionId = ref<string | null>(null)
const connected = ref(false)

let term: Terminal | null = null
let fitAddon: FitAddon | null = null
let searchAddon: SearchAddon | null = null
let resizeObserver: ResizeObserver | null = null

const THEME_MAP: Record<string, ITheme> = {
  'light-clean': {
    background: '#fafafa',
    foreground: '#1f2328',
    cursor: '#0969da',
    selectionBackground: 'rgba(9, 105, 218, 0.22)',
  },
  'one-dark': {
    background: '#1e2127',
    foreground: '#abb2bf',
    cursor: '#528bff',
    selectionBackground: 'rgba(82, 139, 255, 0.28)',
  },
  'tokyo-night': {
    background: '#1a1b26',
    foreground: '#c0caf5',
    cursor: '#7aa2f7',
    selectionBackground: 'rgba(122, 162, 247, 0.26)',
  },
  'catppuccin-mocha': {
    background: '#1e1e2e',
    foreground: '#cdd6f4',
    cursor: '#f5e0dc',
    selectionBackground: 'rgba(203, 166, 247, 0.26)',
  },
}

const resolvedTheme = computed<ITheme>(() => {
  if (props.appearance.theme === 'auto') {
    return store.isDark ? THEME_MAP['one-dark'] : THEME_MAP['light-clean']
  }
  return THEME_MAP[props.appearance.theme] ?? THEME_MAP['one-dark']
})

async function openPtySession() {
  if (!term || !fitAddon) return
  fitAddon.fit()
  try {
    const sid = await api.shellOpen(props.serial, term.cols, term.rows, (chunk) => {
      term?.write(chunk)
    })
    sessionId.value = sid
    connected.value = true
  } catch (e) {
    term.writeln(`\r\n\x1b[31m[连接失败] ${e}\x1b[0m`)
    connected.value = false
  }
}

async function reconnect() {
  if (sessionId.value) {
    await api.shellClose(sessionId.value).catch(() => {})
    sessionId.value = null
  }
  term?.clear()
  await openPtySession()
}

function sendCommand(cmd: string, autoEnter = true) {
  if (!sessionId.value) return
  const payload = autoEnter ? `${cmd}\r` : cmd
  void api.shellWrite(sessionId.value, new TextEncoder().encode(payload))
  term?.focus()
}

function clear() {
  term?.clear()
}

function findNext() {
  if (!searchAddon || !searchKeyword.value) return
  searchAddon.findNext(searchKeyword.value, {
    caseSensitive: searchCaseSensitive.value,
    regex: searchRegex.value,
  })
}

function findPrev() {
  if (!searchAddon || !searchKeyword.value) return
  searchAddon.findPrevious(searchKeyword.value, {
    caseSensitive: searchCaseSensitive.value,
    regex: searchRegex.value,
  })
}

onMounted(async () => {
  if (!containerRef.value) return

  term = new Terminal({
    theme: resolvedTheme.value,
    fontFamily: props.appearance.fontFamily,
    fontSize: props.appearance.fontSize,
    lineHeight: props.appearance.lineHeight,
    cursorStyle: props.appearance.cursorStyle,
    cursorBlink: props.appearance.cursorBlink,
    scrollback: props.appearance.scrollback,
    allowProposedApi: true,
  })
  fitAddon = new FitAddon()
  searchAddon = new SearchAddon()
  term.loadAddon(fitAddon)
  term.loadAddon(searchAddon)
  term.loadAddon(new WebLinksAddon())

  term.open(containerRef.value)
  fitAddon.fit()

  term.onData((data) => {
    if (sessionId.value) {
      void api.shellWrite(sessionId.value, new TextEncoder().encode(data))
    }
  })

  term.onResize(({ cols, rows }) => {
    if (sessionId.value) {
      void api.shellResize(sessionId.value, cols, rows)
    }
  })

  term.onSelectionChange(() => {
    if (props.appearance.copyOnSelect && term?.hasSelection()) {
      const sel = term.getSelection()
      if (sel) {
        void navigator.clipboard.writeText(sel)
      }
    }
  })

  term.attachCustomKeyEventHandler((ev) => {
    if ((ev.ctrlKey || ev.metaKey) && ev.key.toLowerCase() === 'f' && ev.type === 'keydown') {
      searchVisible.value = !searchVisible.value
      return false
    }
    if (ev.ctrlKey && ev.shiftKey && ev.key.toLowerCase() === 'c' && ev.type === 'keydown') {
      const sel = term?.getSelection()
      if (sel) {
        void navigator.clipboard.writeText(sel)
        message.info('已复制选中内容')
      }
      return false
    }
    return true
  })

  resizeObserver = new ResizeObserver(() => {
    fitAddon?.fit()
  })
  resizeObserver.observe(containerRef.value)

  await openPtySession()
})

watch(
  () => [props.appearance, store.isDark],
  () => {
    if (!term) return
    term.options.theme = resolvedTheme.value
    term.options.fontFamily = props.appearance.fontFamily
    term.options.fontSize = props.appearance.fontSize
    term.options.lineHeight = props.appearance.lineHeight
    term.options.cursorStyle = props.appearance.cursorStyle
    term.options.cursorBlink = props.appearance.cursorBlink
    term.options.scrollback = props.appearance.scrollback
    fitAddon?.fit()
  },
  { deep: true }
)

onBeforeUnmount(() => {
  resizeObserver?.disconnect()
  if (sessionId.value) {
    void api.shellClose(sessionId.value)
  }
  term?.dispose()
})

defineExpose({ sendCommand, clear, reconnect })
</script>

<template>
  <div class="terminal-pane-wrap">
    <div v-if="searchVisible" class="search-bar">
      <NSpace align="center" :size="8">
        <NInput
          v-model:value="searchKeyword"
          size="tiny"
          placeholder="搜索终端输出..."
          style="width: 200px"
          @keydown.enter="findNext"
        />
        <NCheckbox v-model:checked="searchCaseSensitive" size="small">区分大小写</NCheckbox>
        <NCheckbox v-model:checked="searchRegex" size="small">正则</NCheckbox>
        <NButton size="tiny" @click="findPrev">上一个</NButton>
        <NButton size="tiny" @click="findNext">下一个</NButton>
        <NButton size="tiny" quaternary @click="searchVisible = false">关闭</NButton>
      </NSpace>
    </div>
    <div ref="containerRef" class="xterm-host" />
  </div>
</template>

<style scoped>
.terminal-pane-wrap {
  position: relative;
  display: flex;
  flex-direction: column;
  height: 100%;
  width: 100%;
  border-radius: 6px;
  overflow: hidden;
}
.search-bar {
  padding: 6px 10px;
  border-bottom: 1px solid rgba(128, 128, 128, 0.2);
  background: rgba(128, 128, 128, 0.08);
}
.xterm-host {
  flex: 1;
  min-height: 0;
  padding: 6px;
}
</style>
